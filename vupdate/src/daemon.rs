use camino::Utf8Path;
use miette::{Context, IntoDiagnostic};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use vu_api::{
    api::{Backlight, DialId, Value},
    client::Client,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    dials: HashMap<DialId, DialConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialConfig {
    name: String,
    data: Data,
    update_interval: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Data {
    #[serde(rename = "CPU Load")]
    CpuLoad,
    Mem,
    Disk,
    #[serde(rename = "CPU Temp")]
    CpuTemp,
    Swap,
    Battery,
}

pub async fn gen_config(
    client: &vu_api::client::Client,
    path: impl AsRef<Utf8Path>,
) -> miette::Result<()> {
    use std::io::Write;

    let mut config = Config::default();
    let dials = client.list_dials().await?;
    let priority = [
        ("CPU Load", Data::CpuLoad),
        ("Memory", Data::Mem),
        ("Swap", Data::Swap),
        ("CPU Temp", Data::CpuTemp),
    ];

    for ((name, data), dial) in priority.into_iter().zip(dials) {
        tracing::info!("Assigning dial {} to {name}", dial.uid);
        config.dials.insert(
            dial.uid,
            DialConfig {
                name: name.to_string(),
                data,
                update_interval: Duration::from_secs(1),
            },
        );
    }

    let toml = toml::to_string_pretty(&config).into_diagnostic()?;
    tracing::info!(config = %toml);

    let path = path.as_ref();
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

pub async fn run(client: Client, config: Config) -> miette::Result<()> {
    let mut tasks = tokio::task::JoinSet::new();
    for (uid, dial) in config.dials {
        tracing::info!(?uid, data = ?dial.data, "configuring dial");
        client
            .set_name(&uid, &dial.name)
            .await
            .with_context(|| format!("failed to set name for {uid} to {}", dial.name))?;

        struct ImgFile {
            name: &'static str,
            data: &'static [u8],
        }

        impl ImgFile {
            async fn set_img(&self, client: &Client, dial: &DialId) -> miette::Result<()> {
                use reqwest::multipart::Part;
                let part = Part::bytes(self.data);
                tracing::info!("setting image for {dial} to {}", self.name);
                client
                    .set_image(dial, self.name, part, false)
                    .await
                    .with_context(|| format!("failed to set image for {dial} to {}", self.name))?;
                Ok(())
            }
        }

        macro_rules! imgfile {
            ($name:literal) => {
                ImgFile {
                    name: $name,
                    data: include_bytes!(concat!("../assets/", $name)),
                }
            };
        }

        let white = Backlight::new(50, 50, 50)?;

        static MEM_IMG: ImgFile = imgfile!("mem.png");
        static CPU_LOAD_IMG: ImgFile = imgfile!("cpu_load.png");
        static CPU_TEMP_IMG: ImgFile = imgfile!("cpu_temp.png");
        static SWAP_IMG: ImgFile = imgfile!("swap.png");

        let img = match dial.data {
            Data::Swap => Some(&SWAP_IMG),
            Data::CpuLoad => Some(&CPU_LOAD_IMG),
            Data::CpuTemp => Some(&CPU_TEMP_IMG),
            Data::Mem => Some(&MEM_IMG),
            ref d => {
                tracing::warn!("skipping image upload for unsupported data {d:?}");
                None
            }
        };

        if let Some(img) = img {
            img.set_img(&client, &uid).await?;
        }
        client.set_backlight(&uid, white).await?;

        tasks.spawn(run_dial(uid, dial, client.clone()));
    }

    while let Some(next) = tasks.join_next().await {
        next.into_diagnostic()??;
    }
    Ok(())
}

#[tracing::instrument(
    level = tracing::Level::INFO,
    name = "dial",
    fields(message = %dial),
    skip_all
)]
async fn run_dial(
    dial: DialId,
    DialConfig {
        data,
        update_interval,
        ..
    }: DialConfig,
    client: Client,
) -> miette::Result<()> {
    use systemstat::Platform;
    tracing::info!("updating dial {dial} with {data:?} every {update_interval:?}");
    let mut interval = tokio::time::interval(update_interval);
    let systemstat = systemstat::System::new();
    loop {
        interval.tick().await;
        let value = match data {
            Data::CpuLoad => {
                let load = systemstat.load_average();
                match load {
                    Ok(systemstat::LoadAverage { one, .. }) => {
                        tracing::debug!("Load (1 min): {one}%");
                        Value::new(one as u8)?
                    }
                    Err(error) => {
                        tracing::warn!(%error, "failed to read load average");
                        continue;
                    }
                }
            }
            Data::Mem => {
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
            Data::Swap => {
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
            Data::CpuTemp => {
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
            _ => miette::bail!("unsupported data type {data:?}"),
        };
        client
            .set_value(&dial, value)
            .await
            .with_context(|| format!("failed to set value for {dial} to {value}"))?;
    }
}
