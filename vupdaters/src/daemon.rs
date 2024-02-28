use self::config::{Config, DialConfig};
use crate::MultiError;
use camino::{Utf8Path, Utf8PathBuf};
use futures::TryFutureExt;
use miette::{Context, IntoDiagnostic};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::{sync::watch, task};
use vu_api::{
    client::{Client, Dial},
    dial::{Backlight, Percent},
};

pub mod config;
#[cfg(all(target_os = "linux", feature = "hotplug"))]
mod hotplug;
mod signal;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum Metric {
    /// Display CPU load as a percentage.
    CpuLoad,
    /// Display memory usage, as a percentage of total memory.
    Mem,
    /// Display disk usage as a percentage of total disk space.
    DiskUsage,
    // /// Display disk usage for a specific filesystem.
    // #[clap(skip)]
    // FsUsage { filesystem: String },
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
        default_value_t = Config::default_path(),
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
    config: DialConfig,
    dial: Dial,
    name: String,
    backoff: backoff::ExponentialBackoffBuilder,
    running: watch::Receiver<bool>,
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
                Config::generate(&client, metrics)
                    .await?
                    .write(&config_path)?;
            }
            None => {
                tracing::info!("starting daemon...");
                run_daemon(client, config_path, hotplug).await?;
            }
        }

        Ok(())
    }
}

impl Metric {
    fn dial_name(&self) -> String {
        match self {
            Metric::Battery => "Battery Remaining".to_owned(),
            Metric::DiskUsage => "Disk Usage".to_owned(),
            // Metric::FsUsage { filesystem } => format!("{} Usage", filesystem),
            Metric::CpuLoad => "CPU Load".to_owned(),
            Metric::CpuTemp => "CPU Temperature".to_owned(),
            Metric::Swap => "Swap Usage".to_owned(),
            Metric::Mem => "Memory Usage".to_owned(),
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
        static DISK_IMG: ImgFile = imgfile!("disk.png");
        static BATT_IMG: ImgFile = imgfile!("battery.png");

        match self {
            Metric::Swap => Some(&SWAP_IMG),
            Metric::CpuLoad => Some(&CPU_LOAD_IMG),
            Metric::CpuTemp => Some(&CPU_TEMP_IMG),
            Metric::Mem => Some(&MEM_IMG),
            Metric::DiskUsage => Some(&DISK_IMG),
            Metric::Battery => Some(&BATT_IMG),
        }
    }
}

struct ImgFile {
    name: &'static str,
    image: &'static [u8],
}

pub async fn run_daemon(
    client: Client,
    config_path: impl AsRef<Utf8Path>,
    hotplug: HotplugSettings,
) -> miette::Result<()> {
    use signal::{SignalAction, SignalListener};

    let mut tasks = task::JoinSet::new();
    let mut signals = SignalListener::new()?;

    let (_running_tx, running) = watch::channel(true);

    if hotplug.enabled {
        #[cfg(all(target_os = "linux", feature = "hotplug"))]
        task::spawn_local(hotplug::run(hotplug, _running_tx));
        #[cfg(all(target_os = "linux", not(feature = "hotplug")))]
        miette::bail!("hotplug support requires `vupdated` to be built with `--features hotplug`!");
        #[cfg(not(target_os = "linux"))]
        miette::bail!("hotplug support is currently only available on Linux!");
    };

    let config = Config::load(&config_path)?;
    config
        .spawn_dial_managers(&client, &running, &mut tasks)
        .await
        .context("failed to spawn dial managers")?;

    loop {
        tokio::select! {
            signal = signals.next_signal() => {
                match signal {
                    SignalAction::Reload => {
                        tracing::info!("Received SIGHUP, reloading config...");
                        tasks.shutdown().await;

                        let config = Config::load(&config_path)?;
                        config
                            .spawn_dial_managers(&client, &running, &mut tasks)
                            .await
                            .context("failed to spawn dial managers")?;
                    }
                    SignalAction::Shutdown => {
                        tracing::info!("Received SIGINT, shutting down");
                        break;
                    }
                }
            }
            join = tasks.join_next() => {
                match join {
                    Some(error) => {
                        error.into_diagnostic()
                            .context("a dial manager task panicked")?
                            .context("a dial manager task failed")?;
                        break;
                    },
                    None => break,
                }
            }
        }
    }

    Ok(())
}

impl Config {
    async fn spawn_dial_managers(
        &self,
        client: &Client,
        running: &watch::Receiver<bool>,
        tasks: &mut task::JoinSet<miette::Result<()>>,
    ) -> miette::Result<()> {
        let mut dials_by_index = HashMap::new();
        let backoff = self.retries.backoff_builder();
        let dials = retry(&backoff, "list dials", || client.list_dials()).await?;
        for (dial, _) in dials {
            let index = dial
                .status()
                .await
                .with_context(|| format!("failed to get status for {}", dial.id()))?
                .index;
            dials_by_index.insert(index, dial);
        }
        if dials_by_index.len() < self.dials.len() {
            tracing::warn!("not enough dials for all dials in config file!");
        }

        let mut dials_spawned = 0;
        for (name, config) in &self.dials {
            if let Some(dial) = dials_by_index.remove(&config.index) {
                let dial_manager = DialManager {
                    name: name.clone(),
                    config: config.clone(),
                    dial,
                    backoff: self.retries.backoff_builder(),
                    running: running.clone(),
                };
                tasks.spawn(dial_manager.run());
                dials_spawned += 1;
            } else {
                tracing::warn!(
                    "no dial found for index {}, skipping {name}...",
                    config.index
                );
            }
        }

        miette::ensure!(dials_spawned > 0, "no dials are connected!");
        Ok(())
    }
}

impl DialManager {
    #[tracing::instrument(
        level = tracing::Level::INFO,
        name = "dial",
        fields(message = %self.name, index = self.config.index),
        skip_all
        err(Display),
    )]
    async fn run(self) -> miette::Result<()> {
        const MAX_ERRORS: usize = 4;

        use systemstat::Platform;
        let DialManager {
            dial,
            name,
            config:
                DialConfig {
                    metric,
                    update_interval,
                    dial_easing,
                    backlight_easing,
                    ..
                },
            backoff,
            mut running,
            ..
        } = self;

        tracing::info!("configuring dial...");

        tracing::info!("setting dial name...");
        retry(&backoff, "set dial name", || dial.set_name(&name)).await?;

        if let Some(config::Easing { period_ms, step }) = dial_easing {
            tracing::info!(?period_ms, %step, "setting dial easing...");

            retry(&backoff, "set dial easing", || {
                dial.set_dial_easing(period_ms, step)
            })
            .await?;
        }

        if let Some(config::Easing { period_ms, step }) = backlight_easing {
            tracing::info!(?period_ms, %step, "setting backlight easing...");
            retry(&backoff, "set backlight easing", || {
                dial.set_backlight_easing(period_ms, step)
            })
            .await?;
        }

        let backlight = Backlight::new(50, 50, 50)?;
        tracing::info!(?backlight, "setting dial backlight...");
        retry(&backoff, "set dial backlight", || {
            dial.set_backlight(backlight)
        })
        .await?;

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
        let mut systemstat_errs =
            MultiError::with_max_errors("reading metric data failed 4 times in a row", MAX_ERRORS);
        let systemstat = systemstat::System::new();

        loop {
            if !(*running.borrow()) {
                tracing::info!("dial updates paused, waiting to restart...");
                while !(*running.borrow_and_update()) {
                    tracing::debug!("updates still paused...");
                    running
                        .changed()
                        .await
                        .into_diagnostic()
                        .context("watch channel closed")?;
                }

                // N.B. that we apparently need to reset the backlight every
                // time we reconnect to the VU-Server, because it apparently
                // doesn't persist backlight state when restarted. IDK why.
                let backlight = Backlight::new(50, 50, 50)?;
                tracing::info!(?backlight, "setting dial backlight...");
                retry(&backoff, "set dial backlight", || {
                    dial.set_backlight(backlight)
                })
                .await?;
            }

            let value = match metric {
                Metric::CpuLoad => {
                    let load = match systemstat.cpu_load_aggregate().into_diagnostic() {
                        Ok(load) => load,
                        Err(error) => {
                            tracing::warn!(%error, "failed to start load aggregate measurement");
                            systemstat_errs.push_error(error)?;
                            continue;
                        }
                    };
                    interval.tick().await;

                    match load.done().into_diagnostic() {
                        Ok(load) => {
                            let percent =
                                (load.user + load.system + load.interrupt + load.nice) * 100.0;
                            tracing::debug!("CPU Load: {percent}%");
                            Percent::new(percent as u8)?
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to read load aggregate");
                            systemstat_errs.push_error(error)?;
                            continue;
                        }
                    }
                }
                Metric::Mem => {
                    let mem = systemstat.memory().into_diagnostic();
                    // tracing::info!("Memory: {mem:?}");
                    match mem {
                        Ok(systemstat::Memory { total, free, .. }) => {
                            let percent_free = free.0 / (total.0 / 100);
                            let percent_used = 100 - percent_free;
                            tracing::debug!("Memory: {percent_used}% used");
                            Percent::new(percent_used as u8)?
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to read memory usage");
                            systemstat_errs.push_error(error)?;
                            continue;
                        }
                    }
                }
                Metric::Swap => {
                    let swap = systemstat.swap().into_diagnostic();
                    match swap {
                        Ok(systemstat::Swap { total, free, .. }) => {
                            let percent_free = free.0 / (total.0 / 100);
                            let percent_used = 100 - percent_free;
                            tracing::debug!("Swap: {percent_used}% used");
                            Percent::new(percent_used as u8)?
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to read swap usage");
                            systemstat_errs.push_error(error)?;
                            continue;
                        }
                    }
                }
                Metric::CpuTemp => {
                    let temp = systemstat.cpu_temp().into_diagnostic();
                    match temp {
                        Ok(temp) => {
                            tracing::debug!("CPU temp: {temp}°C");
                            Percent::new(temp as u8)?
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to read CPU temp");
                            continue;
                        }
                    }
                }
                Metric::Battery => {
                    let battery = systemstat.battery_life().into_diagnostic();
                    match battery {
                        Ok(battery) => {
                            let remaining = battery.remaining_capacity * 100.0;
                            tracing::debug!("Battery: {remaining}% remaining");
                            Percent::new(remaining as u8)?
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to read battery status");
                            systemstat_errs.push_error(error)?;
                            continue;
                        }
                    }
                }
                Metric::DiskUsage => {
                    let mounts = systemstat.mounts().into_diagnostic();
                    let filesystems = match mounts {
                        Ok(mounts) => mounts,
                        Err(error) => {
                            tracing::warn!(%error, "failed to read mounts");
                            systemstat_errs.push_error(error)?;
                            continue;
                        }
                    };
                    let (total, free) = filesystems.iter().fold((0, 0), |(total, free), fs| {
                        let total = total + fs.total.as_u64();
                        let free = free + fs.free.as_u64();
                        tracing::trace!(
                            "filesystem {} has {} bytes free, {} bytes total",
                            fs.fs_mounted_on,
                            fs.free,
                            fs.total
                        );
                        (total, free)
                    });

                    let percent_free = free / (total / 100);
                    let percent_used = 100 - percent_free;
                    tracing::debug!("Disk: {percent_used}% used");
                    Percent::new(percent_used as u8)?
                }
            };
            retry(&backoff, "set value", || dial.set(value))
                .await
                .with_context(|| format!("failed to set value for {name} to {value}"))?;
            systemstat_errs.clear();
            if metric != Metric::CpuLoad {
                interval.tick().await;
            }
        }
    }
}

async fn retry<F, T>(
    backoff: &backoff::ExponentialBackoffBuilder,
    name: &'static str,
    f: impl Fn() -> F,
) -> Result<T, vu_api::client::Error>
where
    F: std::future::Future<Output = Result<T, vu_api::client::Error>>,
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

fn backoff_error(error: vu_api::client::Error) -> backoff::Error<vu_api::client::Error> {
    use vu_api::client::Error;
    match error {
        error @ Error::BuildUrl(_) | error @ Error::BuildRequest(_) => {
            backoff::Error::permanent(error)
        }
        error => backoff::Error::transient(error),
    }
}
