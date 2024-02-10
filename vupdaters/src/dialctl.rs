use camino::Utf8PathBuf;
use miette::{Context, IntoDiagnostic};
use std::fmt;
use vu_api::{api::DialInfo, dial, Dial};

#[derive(Debug, clap::Parser)]
#[command(author, version, about)]
#[command(propagate_version = true)]
pub struct Args {
    #[clap(flatten)]
    client_args: crate::cli::ClientArgs,

    #[clap(flatten)]
    output_args: crate::cli::OutputArgs,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// List all dials.
    List {
        /// If set, show verbose dial details.
        #[clap(long, short = 'd')]
        details: bool,
    },

    /// Get detailed status information about a dial.
    Status {
        #[clap(flatten)]
        dial: DialSelection,
    },

    /// Set a dial's value, image file, backlight, or easing config.
    ///
    /// At least one of `--value`, `--image`, `--red`, `--green`, or `--blue`
    /// must be provided.
    Set {
        #[clap(flatten)]
        dial: DialSelection,

        #[clap(flatten)]
        values: SetValues,
    },
}

#[derive(Debug, clap::Parser)]
#[command(next_help_heading = "Setting Values")]
#[group(id = "set", required = true, multiple = true)]
pub struct SetValues {
    /// Set the dial's needle to the provided value.
    ///
    /// Values must be between 0 and 100.
    #[clap(long, short = 'v')]
    value: Option<dial::Value>,

    /// Set the dial's background image to the provided image file.
    #[clap(long, value_hint = clap::ValueHint::FilePath)]
    image: Option<Utf8PathBuf>,

    /// Set the red value of the dial's backlight to the provided value.
    ///
    /// Values must be between 0 and 100.
    #[clap(long, short = 'r')]
    red: Option<dial::Value>,

    /// Set the green value of the dial's backlight to the provided value.
    ///
    /// Values must be between 0 and 100.
    #[clap(long, short = 'g')]
    green: Option<dial::Value>,

    /// Set the blue value of the dial's backlight to the provided value.
    ///
    /// Values must be between 0 and 100.
    #[clap(long, short = 'b')]
    blue: Option<dial::Value>,
}

#[derive(Debug, clap::Parser)]
#[command(next_help_heading = "Dial Selection")]
#[group(id = "selection", required = true, multiple = false)]
pub struct DialSelection {
    /// The dial's UID.
    #[clap(long = "dial", short = 'd')]
    uid: Option<dial::Id>,

    /// The dial's index.
    #[clap(long, short = 'i')]
    index: Option<usize>,
}

impl Args {
    pub async fn run(self) -> miette::Result<()> {
        let Self {
            command,
            client_args,
            output_args,
        } = self;
        output_args.init_tracing()?;
        let client = client_args
            .into_client()
            .context("failed to build client")?;
        command.run(&client).await
    }
}

impl Command {
    pub async fn run(self, client: &vu_api::Client) -> miette::Result<()> {
        match self {
            Command::List { details } => {
                list_dials(client, details).await?;
            }

            Command::Status { dial } => {
                let status = match dial.select_dial(client).await? {
                    (_, Some(status)) => status,
                    (d, None) => d
                        .status()
                        .await
                        .with_context(|| format!("failed to get status for dial {dial}"))?,
                };
                print_status(status);
            }

            Command::Set { dial, values } => values.run(client, &dial).await?,
        };
        Ok(())
    }
}

impl DialSelection {
    async fn select_dial(
        &self,
        client: &vu_api::Client,
    ) -> miette::Result<(Dial, Option<dial::Status>)> {
        match self.uid {
            Some(ref uid) => client
                .dial(uid.clone())
                .into_diagnostic()
                .map(|dial| (dial, None)),
            None => {
                let index = self
                    .index
                    .expect("if no UID is provided, an index must be provided");
                let dials = client.list_dials().await?;
                let mut found_dial = None;
                for (dial, _) in dials {
                    let status = dial
                        .status()
                        .await
                        .with_context(|| format!("failed to get status for dial {}", dial.id()))?;
                    if status.index == index {
                        found_dial = Some((dial, Some(status)));
                        break;
                    }
                }
                found_dial.ok_or_else(|| miette::miette!("no dial found with index {index}"))
            }
        }
    }
}

impl fmt::Display for DialSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.uid {
            Some(ref uid) => write!(f, "ID {}", uid),
            None => write!(f, "index {}", self.index.expect("index must be provided")),
        }
    }
}

impl SetValues {
    #[tracing::instrument(name = "set", level = tracing::Level::INFO, skip(self, client))]
    async fn run(self, client: &vu_api::Client, selection: &DialSelection) -> miette::Result<()> {
        let (dial, status) = selection.select_dial(client).await?;
        tracing::debug!(%dial, "Found dial for selection");

        if let Some(value) = self.value {
            tracing::info!(%dial, %value, "Setting value...");
            dial.set(value)
                .await
                .with_context(|| format!("failed to set value for dial {selection} to {value}"))?;
        }

        if self.red.is_some() || self.green.is_some() || self.blue.is_some() {
            let mut backlight = match status {
                Some(status) => status,
                None => dial
                    .status()
                    .await
                    .with_context(|| format!("failed to get status for dial {selection}"))?,
            }
            .backlight;
            if let Some(red) = self.red {
                tracing::info!(%red, "Setting backlight...");
                backlight.red = red;
            }

            if let Some(green) = self.green {
                tracing::info!(%green, "Setting backlight...");
                backlight.green = green;
            }

            if let Some(blue) = self.blue {
                tracing::info!(%blue, "Setting backlight...");
                backlight.blue = blue;
            }

            dial.set_backlight(backlight.clone())
                .await
                .with_context(|| {
                    format!("failed to set backlight for dial {selection} to {backlight:?}")
                })?;
        }

        if let Some(image) = self.image {
            tracing::warn!("Not setting image to {image}; not yet implemented.");
        }

        Ok(())
    }
}

async fn list_dials(client: &vu_api::client::Client, details: bool) -> miette::Result<()> {
    let dials = client.list_dials().await?;
    fn print_info(dial: DialInfo) {
        println!("DIAL: {}", dial.uid);
        println!("├─name: {}", dial.dial_name);
        println!("├─value: {}", dial.value);
        print_backlight(dial.backlight);
        println!("└─image: {}\n", dial.image_file);
    }

    if details {
        for (dial, info) in dials {
            match dial.status().await {
                Ok(status) => print_status(status),
                Err(error) => {
                    eprintln!(
                        "failed to get detailed status for dial {}: {error}",
                        info.uid
                    );

                    println!("{dial:#?}\n");
                }
            }
        }
    } else {
        for (_, info) in dials {
            print_info(info)
        }
    }

    Ok(())
}

fn print_status(dial: dial::Status) {
    println!("DIAL: {}", dial.uid);
    println!("├─name: {}", dial.dial_name);
    println!("├─value: {}", dial.value);
    println!("├─index: {}", dial.index);
    println!("├─rgbw: {:?}", dial.rgbw);
    println!("├─image file: {}", dial.image_file);
    let dial::Easing {
        dial_step,
        dial_period,
        backlight_step,
        backlight_period,
    } = dial.easing;
    println!("├─DIAL EASING:");
    println!("│ ├─dial step: {dial_step}");
    println!("│ └─dial period: {dial_period}");
    println!("├─BACKLIGHT EASING:");
    println!("│ ├─backlight step: {backlight_step}");
    println!("│ └─backlight period: {backlight_period}");
    println!("├─VERSION:");
    println!("│ ├─firmware hash: {}", dial.fw_hash);
    println!("│ ├─firmware version: {}", dial.fw_version);
    println!("│ ├─hardware version: {}", dial.hw_version);
    println!("│ └─protocol version: {}", dial.protocol_version);
    print_backlight(dial.backlight);
    println!("├─STATUS:");
    println!("│ ├─value_changed: {}", dial.value_changed);
    println!("│ ├─backlight_changed: {}", dial.backlight_changed);
    println!("│ └─image_changed: {}", dial.image_changed);
    println!("└─update deadline: {}\n", dial.update_deadline);
}

fn print_backlight(dial::Backlight { red, green, blue }: dial::Backlight) {
    println!("├─BACKLIGHT:");
    println!("│ ├─red: {red}");
    println!("│ ├─green: {green}");
    println!("│ └─blue: {blue}");
}
