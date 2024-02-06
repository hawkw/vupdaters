use camino::Utf8PathBuf;
use miette::{Context, IntoDiagnostic};
use vu_api::{api::DialInfo, dial};

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
    Dials {
        /// If set, show verbose dial details.
        #[clap(long, short = 'd')]
        details: bool,
    },

    /// Commands related to a specific dial.
    Dial {
        /// The dial's UID.
        uid: dial::Id,
        #[clap(subcommand)]
        command: DialCommand,
    },
}

#[derive(Debug, clap::Subcommand)]
pub enum DialCommand {
    /// Get detailed status information about this dial.
    Status,
    /// Set a dial's value.
    Set {
        /// The new value to set the dial to.
        value: vu_api::dial::Value,
    },
    /// Set the dial's background image.
    SetImage {
        /// Path to the new image file.
        #[clap(value_hint = clap::ValueHint::FilePath)]
        path: Utf8PathBuf,
    },
    /// Sets the dial's backlight to the provided RGB values.
    SetBacklight {
        /// A red value in the range 0-100.
        #[clap(long, short = 'r')]
        red: dial::Value,

        /// A red value in the range 0-100.
        #[clap(long, short = 'g')]
        green: dial::Value,

        /// A red value in the range 0-100.
        #[clap(long, short = 'b')]
        blue: dial::Value,
    },
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
            Command::Dials { details } => {
                list_dials(client, details).await?;
            }

            Command::Dial {
                uid,
                command: DialCommand::Status,
            } => {
                let status = client
                    .dial(uid.clone())
                    .into_diagnostic()?
                    .status()
                    .await
                    .with_context(|| format!("failed to get status for dial {uid}"))?;
                print_status(status);
            }

            Command::Dial { .. } => todo!(),
        };
        Ok(())
    }
}

impl DialCommand {
    pub async fn run(self, client: &vu_api::Client, uid: dial::Id) -> miette::Result<()> {
        let dial = client
            .dial(uid.clone())
            .into_diagnostic()
            .with_context(|| format!("invalid dial UID {uid}"))?;
        match self {
            DialCommand::Status => {
                let status = dial
                    .status()
                    .await
                    .with_context(|| format!("failed to get status for dial {uid}"))?;
                print_status(status);
            }

            DialCommand::Set { value } => {
                client
                    .dial(uid.clone())
                    .into_diagnostic()?
                    .set(value)
                    .await
                    .with_context(|| format!("failed to set value for dial {uid}"))?;
            }

            DialCommand::SetImage { path } => {
                // client
                //     .dial(uid.clone())
                //     .into_diagnostic()?
                //     .set_image(path,)
                //     .await
                //     .with_context(|| format!("failed to set image for dial
                //     {uid}"))?;
                todo!("eliza: {path:?}")
            }

            DialCommand::SetBacklight { red, green, blue } => {
                client
                    .dial(uid.clone())
                    .into_diagnostic()?
                    .set_backlight(dial::Backlight { red, green, blue })
                    .await
                    .with_context(|| format!("failed to set backlight for dial {uid}"))?;
            }
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
