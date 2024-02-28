use super::Metric;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::{collections::HashMap, time::Duration};
use vu_api::dial::Percent;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub(super) dials: HashMap<String, DialConfig>,

    #[serde(default)]
    pub(super) retries: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "RetryConfig::default_initial_backoff")]
    initial_backoff: Duration,

    #[serde(default = "RetryConfig::default_jitter")]
    jitter: f64,

    #[serde(default = "RetryConfig::default_multiplier")]
    multiplier: f64,

    #[serde(default = "RetryConfig::default_max_backoff")]
    max_backoff: Duration,

    #[serde(default)]
    max_elapsed_time: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialConfig {
    pub(super) index: usize,
    pub(super) metric: Metric,
    pub(super) update_interval: Duration,
    pub(super) dial_easing: Option<Easing>,
    pub(super) backlight_easing: Option<Easing>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Easing {
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub(super) period_ms: Duration,
    pub(super) step: Percent,
}

impl Config {
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
