use camino::{Utf8Path, Utf8PathBuf};
use futures::TryFutureExt;
use miette::{Context, IntoDiagnostic};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use vu_api::{
    client::{Client, Dial},
    dial::{Backlight, Value},
};

#[cfg(all(target_os = "linux", feature = "hotplug"))]
mod hotplug;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    dials: HashMap<String, DialConfig>,

    #[serde(default)]
    retries: RetryConfig,
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
    index: usize,
    metric: Metric,
    update_interval: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum Metric {
    /// Display CPU load as a percentage.
    CpuLoad,
    /// Display memory usage, as a percentage of total memory.
    Mem,
    /// Display disk usage as a percentage of total disk space.
    Disk,
    /// Display CPU temperature.
    CpuTemp,
    /// Display swap usage, as a percentage of total swap space.
    Swap,
    /// Display the current remaining battery percentage.
    Battery,
}

#[derive(Debug, clap::Parser)]
#[command(
    name = "vupdated",
    author,
    version,
    about = "Daemon for updating VU-1 dials"
)]
pub struct Args {
    /// Path to the config file.
    #[clap(
        long = "config",
        short = 'c',
        default_value_t = default_config_path(),
        value_hint = clap::ValueHint::FilePath,
        global = true,
    )]
    config_path: Utf8PathBuf,

    #[clap(flatten)]
    client_args: crate::cli::ClientArgs,

    #[clap(flatten)]
    output_args: crate::cli::OutputArgs,

    #[clap(flatten)]
    hotplug: HotplugSettings,

    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, clap::Parser)]
#[command(next_help_heading = "USB Hotplug Settings")]
#[group(id = "hotplug", multiple = true)]
pub struct HotplugSettings {
    /// Enable USB hotplug management.
    ///
    /// If this is set, then `vupdated` will listen for USB hotplug events for
    /// USB-serial TTYs, and, when one occurs, attempt to restart the VU-Server
    /// systemd service.
    ///
    /// This feature is currently only supported on Linux.
    #[clap(long = "hotplug")]
    enabled: bool,

    /// The systemd unit name for the VU-Server service.
    ///
    /// When a hotplug event for a USB-serial device occurs, `vupdated` will
    /// attempt to restart this systemed service.
    #[clap(long, default_value = "VU-Server.service")]
    hotplug_service: String,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Generate a new config file with the given metrics.
    GenConfig {
        /// The list of requested metrics to include in the config file.
        ///
        /// If more metrics are requested than the number of dials connected to
        /// the system, only the first N metrics will be included in the config,
        /// where N is the number of dials discovered.
        #[arg(
            num_args = 1..,
            value_enum,
            default_values_t = [
                Metric::CpuLoad,
                Metric::Mem,
                Metric::CpuTemp,
                Metric::Swap,
            ],
        )]
        metrics: Vec<Metric>,
    },
}

struct DialManager {
    name: String,
    metric: Metric,
    update_interval: std::time::Duration,
    dial: Dial,
    index: usize,
    backoff: backoff::ExponentialBackoffBuilder,
}

fn default_config_path() -> Utf8PathBuf {
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

impl Args {
    pub async fn run(self) -> miette::Result<()> {
        let Self {
            subcommand,
            client_args,
            output_args,
            config_path,
            hotplug,
        } = self;
        output_args.init_tracing()?;
        let client = client_args
            .into_client()
            .context("failed to build client")?;
        match subcommand {
            Some(Subcommand::GenConfig { metrics }) => {
                gen_config(&client, config_path, metrics).await?
            }
            None => {
                tracing::info!("starting daemon...");
                let config = {
                    let file = std::fs::read_to_string(&config_path)
                        .into_diagnostic()
                        .with_context(|| format!("failed to read config file {config_path}"))?;
                    toml::from_str(&file)
                        .into_diagnostic()
                        .with_context(|| format!("failed to parse config file {config_path}"))?
                };
                run_daemon(client, config, hotplug).await?;
            }
        }

        Ok(())
    }
}

async fn gen_config(
    client: &vu_api::client::Client,
    config_path: impl AsRef<Utf8Path>,
    metrics: Vec<Metric>,
) -> miette::Result<()> {
    use std::io::Write;

    tracing::info!("generating config with metrics: {metrics:?}");
    let mut config = Config::default();
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
            },
        );
    }

    let toml = toml::to_string_pretty(&config).into_diagnostic()?;
    tracing::info!(config = %toml);

    let path = config_path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .into_diagnostic()
            .with_context(|| format!("failed to create {parent}"))?;
    }
    std::fs::File::create(path)
        .into_diagnostic()
        .with_context(|| format!("failed to create {path}"))?
        .write_all(toml.as_bytes())
        .into_diagnostic()
        .with_context(|| format!("failed to write to {path}"))?;

    Ok(())
}

impl Metric {
    fn dial_name(&self) -> &'static str {
        match self {
            Metric::Battery => "Battery Remaining",
            Metric::Disk => "Disk Usage",
            Metric::CpuLoad => "CPU Load",
            Metric::CpuTemp => "CPU Temperature",
            Metric::Swap => "Swap Usage",
            Metric::Mem => "Memory Usage",
        }
    }

    fn img_file(&self) -> Option<&'static ImgFile> {
        macro_rules! imgfile {
            ($name: literal) => {
                ImgFile {
                    name: $name,
                    image: include_bytes!(concat!("../assets/", $name)),
                }
            };
        }
        static MEM_IMG: ImgFile = imgfile!("mem.png");
        static CPU_LOAD_IMG: ImgFile = imgfile!("cpu_load.png");
        static CPU_TEMP_IMG: ImgFile = imgfile!("cpu_temp.png");
        static SWAP_IMG: ImgFile = imgfile!("swap.png");

        match self {
            Metric::Swap => Some(&SWAP_IMG),
            Metric::CpuLoad => Some(&CPU_LOAD_IMG),
            Metric::CpuTemp => Some(&CPU_TEMP_IMG),
            Metric::Mem => Some(&MEM_IMG),
            ref d => {
                tracing::warn!("skipping image upload for unsupported Metric {d:?}");
                None
            }
        }
    }
}

struct ImgFile {
    name: &'static str,
    image: &'static [u8],
}

pub async fn run_daemon(
    client: Client,
    config: Config,
    hotplug: HotplugSettings,
) -> miette::Result<()> {
    // TODO(eliza): handle sighup...
    let mut tasks = tokio::task::JoinSet::new();

    if hotplug.enabled {
        #[cfg(all(target_os = "linux", feature = "hotplug"))]
        tasks.spawn_local(hotplug::run(hotplug));
        #[cfg(all(target_os = "linux", not(feature = "hotplug")))]
        miette::bail!("hotplug support requires `vupdated` to be built with `--features hotplug`!");
        #[cfg(not(target_os = "linux"))]
        miette::bail!("hotplug support is currently only available on Linux!");
    }

    let mut dials_by_index = HashMap::new();

    for (dial, _) in client.list_dials().await? {
        let index = dial
            .status()
            .await
            .with_context(|| format!("failed to get status for {}", dial.id()))?
            .index;
        dials_by_index.insert(index, dial);
    }
    if dials_by_index.len() < config.dials.len() {
        tracing::warn!("not enough dials for all dials in config file!");
    }

    for (
        name,
        DialConfig {
            metric,
            update_interval,
            index,
        },
    ) in config.dials
    {
        let dial = dials_by_index
            .remove(&index)
            .ok_or_else(|| miette::miette!("no dial for index {index}"))?;
        let dial_manager = DialManager {
            name,
            metric,
            update_interval,
            dial,
            index,
            backoff: config.retries.backoff_builder(),
        };
        tasks.spawn(dial_manager.run());
    }

    while let Some(next) = tasks.join_next().await {
        next.into_diagnostic()?.context("a task failed!")?;
    }
    Ok(())
}

impl DialManager {
    #[tracing::instrument(
        level = tracing::Level::INFO,
        name = "dial",
        fields(message = %self.name, index = self.index),
        skip_all
        err(Display),
    )]
    async fn run(self) -> miette::Result<()> {
        use systemstat::Platform;
        let DialManager {
            dial,
            name,
            metric,
            update_interval,
            backoff,
            ..
        } = self;

        tracing::info!("configuring dial...");

        async fn retry<F>(
            backoff: &backoff::ExponentialBackoffBuilder,
            name: &'static str,
            f: impl Fn() -> F,
        ) -> Result<(), vu_api::api::Error>
        where
            F: std::future::Future<Output = Result<(), vu_api::api::Error>>,
        {
            backoff::future::retry_notify(
                backoff.build(),
                || f().map_err(backoff_error),
                |error, retry_after| {
                    tracing::warn!(%error, ?retry_after, "failed to {name}, retrying...");
                },
            )
            .await
        }

        retry(&backoff, "set dial name", || dial.set_name(&name)).await?;

        let white = Backlight::new(50, 50, 50)?;
        retry(&backoff, "set dial backlight", || dial.set_backlight(white)).await?;
        if let Some(img) = metric.img_file() {
            retry(&backoff, "set dial image", || {
                use reqwest::multipart::Part;
                let part = Part::bytes(img.image);
                tracing::info!("setting image for {} to {}", dial.id(), img.name);
                dial.set_image(img.name, part, false)
            })
            .await?;
        }

        tracing::info!("updating dial with {metric:?} every {update_interval:?}");
        let mut interval = tokio::time::interval(update_interval);
        let systemstat = systemstat::System::new();
        loop {
            let value = match metric {
                Metric::CpuLoad => {
                    let load = match systemstat.cpu_load_aggregate() {
                        Ok(load) => load,
                        Err(error) => {
                            tracing::warn!(%error, "failed to start load aggregate measurement");
                            continue;
                        }
                    };
                    interval.tick().await;

                    match load.done() {
                        Ok(load) => {
                            let percent =
                                (load.user + load.system + load.interrupt + load.nice) * 100.0;
                            tracing::debug!("CPU Load: {percent}%");
                            Value::new(percent as u8)?
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to read load aggregate");
                            continue;
                        }
                    }
                }
                Metric::Mem => {
                    let mem = systemstat.memory();
                    // tracing::info!("Memory: {mem:?}");
                    match mem {
                        Ok(systemstat::Memory { total, free, .. }) => {
                            let percent_free = free.0 / (total.0 / 100);
                            let percent_used = 100 - percent_free;
                            tracing::debug!("Memory: {percent_used}% used");
                            Value::new(percent_used as u8)?
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to read memory usage");
                            continue;
                        }
                    }
                }
                Metric::Swap => {
                    let swap = systemstat.swap();
                    // tracing::info!("Swap: {mem:?}");
                    match swap {
                        Ok(systemstat::Swap { total, free, .. }) => {
                            let percent_free = free.0 / (total.0 / 100);
                            let percent_used = 100 - percent_free;
                            tracing::debug!("Swap: {percent_used}% used");
                            Value::new(percent_used as u8)?
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to read swap usage");
                            continue;
                        }
                    }
                }
                Metric::CpuTemp => {
                    let temp = systemstat.cpu_temp();
                    match temp {
                        Ok(temp) => {
                            tracing::debug!("CPU temp: {temp}°C");
                            Value::new(temp as u8)?
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to read CPU temp");
                            continue;
                        }
                    }
                }
                _ => miette::bail!("unsupported Metric type {metric:?}"),
            };
            retry(&backoff, "set value", || dial.set(value))
                .await
                .with_context(|| format!("failed to set value for {name} to {value}"))?;
            if metric != Metric::CpuLoad {
                interval.tick().await;
            }
        }
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

    fn backoff_builder(&self) -> backoff::ExponentialBackoffBuilder {
        let mut builder = backoff::ExponentialBackoffBuilder::new();
        builder
            .with_initial_interval(self.initial_backoff)
            .with_randomization_factor(self.jitter)
            .with_max_interval(self.max_backoff)
            .with_max_elapsed_time(self.max_elapsed_time);
        builder
    }
}

fn backoff_error(error: vu_api::api::Error) -> backoff::Error<vu_api::api::Error> {
    use vu_api::api::Error;
    match error {
        error @ Error::BuildUrl(_)
        | error @ Error::BuildRequest(_)
        | error @ Error::InvalidBacklight(_)
        | error @ Error::InvalidValue(_) => backoff::Error::permanent(error),
        error => backoff::Error::transient(error),
    }
}
