use super::Metric;
use camino::{Utf8Path, Utf8PathBuf};
use miette::{Context, IntoDiagnostic};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, time::Duration};
use vu_api::dial::{Backlight, Percent};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub(super) dials: HashMap<String, DialConfig>,

    #[serde(default)]
    pub(super) retries: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RetryConfig {
    #[serde(
        with = "humantime_serde",
        default = "RetryConfig::default_initial_backoff"
    )]
    initial_backoff: Duration,

    #[serde(default = "RetryConfig::default_jitter")]
    jitter: f64,

    #[serde(default = "RetryConfig::default_multiplier")]
    multiplier: f64,

    #[serde(with = "humantime_serde", default = "RetryConfig::default_max_backoff")]
    max_backoff: Duration,

    #[serde(with = "humantime_serde", default)]
    max_elapsed_time: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DialConfig {
    pub(super) index: usize,
    pub(super) metric: Metric,
    #[serde(with = "humantime_serde")]
    pub(super) update_interval: Duration,
    #[serde(flatten, with = "prefix_easing")]
    pub(super) easing: Option<Easing>,

    pub(super) backlight: BacklightSettings,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Easing {
    #[serde(with = "humantime_serde")]
    pub(super) period: Duration,
    pub(super) step: Percent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct BacklightSettings {
    #[serde(default)]
    pub(super) mode: BacklightMode,
    #[serde(default, flatten, with = "prefix_easing")]
    pub(super) easing: Option<Easing>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BacklightMode {
    /// A single, static color.
    Static(Backlight),
    /// Backlight off
    Off,
}

serde_with::with_prefix!(prefix_easing "easing-");
// === impl Config ===

impl Config {
    pub(super) async fn generate(
        client: &vu_api::client::Client,
        metrics: Vec<Metric>,
    ) -> miette::Result<Self> {
        tracing::info!("generating config with metrics: {metrics:?}");
        let mut config = Self::default();
        let dials = client.list_dials().await?;
        if dials.len() < metrics.len() {
            tracing::warn!("not enough dials available to display all requested metrics!");
            tracing::warn!(
                "the generated config will only include the following metrics: {:?}",
                &metrics[..dials.len()]
            );
        }

        for (metric, (dial, info)) in metrics.into_iter().zip(dials) {
            let dial = dial
                .status()
                .await
                .with_context(|| format!("failed to get status for {}", info.uid))?;
            let index = dial.index;
            tracing::info!("Assigning dial {index} to {metric:?}");
            config.dials.insert(
                metric.dial_name().to_string(),
                DialConfig {
                    index,
                    metric,
                    update_interval: Duration::from_secs(1),
                    easing: Some(Easing {
                        period: dial.easing.dial_period,
                        step: dial.easing.dial_step,
                    }),
                    backlight: BacklightSettings {
                        mode: BacklightMode::Static(dial.backlight),
                        easing: Some(Easing {
                            period: dial.easing.backlight_period,
                            step: dial.easing.backlight_step,
                        }),
                    },
                },
            );
        }

        Ok(config)
    }

    pub(super) fn write(&self, path: impl AsRef<Utf8Path>) -> miette::Result<()> {
        use std::io::Write;

        let toml = toml::to_string_pretty(self).into_diagnostic()?;
        tracing::info!(config = %toml);

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .into_diagnostic()
                .with_context(|| format!("failed to create {parent}"))?;
        }
        fs::File::create(path)
            .into_diagnostic()
            .with_context(|| format!("failed to create {path}"))?
            .write_all(toml.as_bytes())
            .into_diagnostic()
            .with_context(|| format!("failed to write to {path}"))?;

        Ok(())
    }

    pub(super) fn load(path: impl AsRef<Utf8Path>) -> miette::Result<Self> {
        let path = path.as_ref();
        tracing::info!("loading config from {path}...");

        let file = fs::read_to_string(path)
            .into_diagnostic()
            .with_context(|| format!("failed to read config file '{path}'"))?;
        toml::from_str(&file)
            .into_diagnostic()
            .with_context(|| format!("failed to parse config file '{path}'"))
    }

    pub(super) fn default_path() -> Utf8PathBuf {
        directories::BaseDirs::new()
            .and_then(|dirs| {
                let path = Utf8Path::from_path(dirs.config_dir())?.join("vupdate/config.toml");
                Some(path)
            })
            .unwrap_or_else(|| {
                ["$HOME", ".config", "vupdate", "config.toml"]
                    .iter()
                    .collect()
            })
    }
}

// === impl RetryConfig ===

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            initial_backoff: Self::default_initial_backoff(),
            jitter: Self::default_jitter(),
            max_backoff: Self::default_max_backoff(),
            max_elapsed_time: Some(Duration::from_millis(
                backoff::default::MAX_ELAPSED_TIME_MILLIS,
            )),
            multiplier: Self::default_multiplier(),
        }
    }
}

impl RetryConfig {
    const fn default_initial_backoff() -> Duration {
        Duration::from_millis(backoff::default::INITIAL_INTERVAL_MILLIS)
    }
    const fn default_jitter() -> f64 {
        backoff::default::RANDOMIZATION_FACTOR
    }

    const fn default_max_backoff() -> Duration {
        Duration::from_millis(backoff::default::MAX_INTERVAL_MILLIS)
    }

    const fn default_multiplier() -> f64 {
        backoff::default::MULTIPLIER
    }

    pub(super) fn backoff_builder(&self) -> backoff::ExponentialBackoffBuilder {
        let mut builder = backoff::ExponentialBackoffBuilder::new();
        builder
            .with_initial_interval(self.initial_backoff)
            .with_randomization_factor(self.jitter)
            .with_max_interval(self.max_backoff)
            .with_max_elapsed_time(self.max_elapsed_time);
        builder
    }
}

// === impl BacklightMode ===

impl Default for BacklightMode {
    fn default() -> Self {
        let color = Backlight::new(50, 50, 50).expect("50 is a valid percent");
        Self::Static(color)
    }
}
