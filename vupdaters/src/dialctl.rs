use camino::Utf8PathBuf;
use miette::{Context, IntoDiagnostic};
use std::fmt;
use vu_api::{api::DialInfo, dial, Dial};

/// A command-line tool for controlling Streacom VU-1 dials.
///
/// Use `dialctl list` to list all dials connected to the system, `dialctl
/// status` to get detailed status information about a dial, or `dialctl set` to
/// set a dial's value, backlight configuration, and background image.
#[derive(Debug, clap::Parser)]
#[command(name = "dialctl", author, version, propagate_version = true)]
pub struct Args {
    #[clap(flatten)]
    client_args: crate::cli::ClientArgs,

    #[clap(flatten)]
    output_args: crate::cli::OutputArgs,

    #[clap(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// List all dials.
    List {
        /// If set, show verbose dial details.
        #[clap(long, short = 'd')]
        details: bool,

        /// Configures how the dials are displayed.
        #[clap(long, short = 'o', default_value_t = OutputMode::Text, value_enum)]
        output: OutputMode,
    },

    /// Get detailed status information about a dial.
    ///
    /// The dial to look up can be selected either by its index (using `--index
    /// <index>`) or by its UID (using `--dial <uid>`).
    Status {
        #[clap(flatten)]
        dial: DialSelection,

        /// Configures how the dial's status is displayed.
        #[clap(long, short = 'o', default_value_t = OutputMode::Text, value_enum)]
        output: OutputMode,
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

    /// Forcibly reload a dial's hardware info.
    Reload {
        /// The UID of the dial to reload.
        #[clap(long = "dial", short = 'd')]
        dial: dial::Id,

        /// Configures how the dial's status is displayed.
        #[clap(long, short = 'o', default_value_t = OutputMode::Text, value_enum)]
        output: OutputMode,
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
    value: Option<dial::Percent>,

    /// Set the dial's background image to the provided image file.
    #[clap(long, value_hint = clap::ValueHint::FilePath)]
    image: Option<Utf8PathBuf>,

    /// Set the red value of the dial's backlight to the provided value.
    ///
    /// Values must be between 0 and 100.
    #[clap(long, short = 'r')]
    red: Option<dial::Percent>,

    /// Set the green value of the dial's backlight to the provided value.
    ///
    /// Values must be between 0 and 100.
    #[clap(long, short = 'g')]
    green: Option<dial::Percent>,

    /// Set the blue value of the dial's backlight to the provided value.
    ///
    /// Values must be between 0 and 100.
    #[clap(long, short = 'b')]
    blue: Option<dial::Percent>,
}

#[derive(Debug, clap::Parser)]
#[command(next_help_heading = "Dial Selection")]
#[group(id = "selection", required = true, multiple = false)]
pub struct DialSelection {
    /// Select a dial by its UID.
    #[clap(long = "dial", short = 'd')]
    uid: Option<dial::Id>,

    /// Select a dial by its numeric index.
    #[clap(long, short = 'i')]
    index: Option<usize>,

    /// Select a dial by its user-assigned name.
    #[clap(long, short = 'n')]
    name: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputMode {
    Text,
    Json,
    Ascii,
}

#[derive(miette::Diagnostic, Debug, thiserror::Error)]
#[error("{}", .msg)]
#[diagnostic()]
struct MultiError {
    msg: &'static str,
    #[related]
    errors: Vec<miette::Report>,
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
        match command {
            Some(command) => command.run(&client).await,
            None => list_dials(&client, false, OutputMode::Text).await,
        }
    }
}

impl Command {
    pub async fn run(self, client: &vu_api::Client) -> miette::Result<()> {
        match self {
            Command::List { details, output } => {
                list_dials(client, details, output).await?;
            }

            Command::Status { dial, output } => {
                let status = match dial.select_dial(client).await? {
                    (_, Some(status)) => status,
                    (d, None) => d
                        .status()
                        .await
                        .with_context(|| format!("failed to get status for dial {dial}"))?,
                };
                output.print_status(&status)?;
            }

            Command::Set { dial, values } => values.run(client, &dial).await?,
            Command::Reload { dial, output } => {
                let status = client
                    .dial(dial)
                    .into_diagnostic()?
                    .reload_hw_info()
                    .await?;
                output.print_status(&status)?;
            }
        };
        Ok(())
    }
}

impl DialSelection {
    #[tracing::instrument(
        level = tracing::Level::DEBUG,
        skip(self, client),
        fields(message = %self)
    )]
    async fn select_dial(
        &self,
        client: &vu_api::Client,
    ) -> miette::Result<(Dial, Option<dial::Status>)> {
        if let Some(ref uid) = self.uid {
            return client
                .dial(uid.clone())
                .into_diagnostic()
                .map(|dial| (dial, None));
        }

        let dials = client.list_dials().await?;
        for (dial, info) in dials {
            match (self.index, self.name.as_deref()) {
                (Some(index), None) => {
                    let status = dial
                        .status()
                        .await
                        .with_context(|| format!("failed to get status for dial {}", dial.id()))?;
                    if status.index == index {
                        tracing::debug!(
                            dial.index = index,
                            dial.name = %status.dial_name,
                            dial.uid = %status.uid,
                            "found dial by index",
                        );
                        return Ok((dial, Some(status)));
                    } else {
                        tracing::debug!(
                            dial.index = status.index,
                            dial.name = %status.dial_name,
                            dial.uid = %status.uid,
                            "dial does not match index {index}",
                        );
                    }
                }
                (None, Some(name)) if info.dial_name == name => {
                    tracing::debug!(
                        dial.name = %info.dial_name,
                        dial.uid = %info.uid,
                        "found dial by name",
                    );
                    return Ok((dial, None));
                }
                (None, Some(name)) => {
                    tracing::debug!(
                        dial.name = %info.dial_name,
                        dial.uid = %info.uid,
                        "dial does not match name {name:?}",
                    );
                }
                _ => unreachable!("selection must be validated to include either an index or name"),
            };
        }

        Err(miette::miette!("no dial found for {self}"))
    }
}

impl fmt::Display for DialSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.uid.as_ref(), self.index, self.name.as_deref()) {
            (Some(uid), _, _) => write!(f, "ID {uid}"),
            (None, Some(index), _) => write!(f, "index {index}"),
            (None, None, Some(name)) => write!(f, "name {name:?}"),
            _ => f.write_str("<invalid dial selection>"),
        }
    }
}

impl SetValues {
    #[tracing::instrument(
        name = "set",
        level = tracing::Level::INFO,
        skip_all,
        fields(dial = %selection),
    )]
    async fn run(self, client: &vu_api::Client, selection: &DialSelection) -> miette::Result<()> {
        let (dial, status) = selection.select_dial(client).await?;
        tracing::debug!(%dial, "Found dial for selection");
        let mut errors = Vec::new();
        if let Some(value) = self.value {
            tracing::info!(%dial, %value, "Setting value...");
            if let Err(e) = dial
                .set(value)
                .await
                .with_context(|| format!("failed to set value to {value}"))
            {
                errors.push(e);
            }
        }

        if self.red.is_some() || self.green.is_some() || self.blue.is_some() {
            let backlight = match status {
                Some(status) => Ok(status.backlight),
                None => dial
                    .status()
                    .await
                    .with_context(|| format!("failed to get status for dial {selection}"))
                    .map(|status| status.backlight),
            };
            match backlight {
                Ok(mut backlight) => {
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

                    if let Err(e) = dial.set_backlight(backlight).await.with_context(|| {
                        format!("failed to set backlight for dial {selection} to {backlight:?}")
                    }) {
                        errors.push(e);
                    }
                }
                Err(e) => errors.push(e.context("failed to set backlight")),
            }
        }

        if let Some(image) = self.image {
            tracing::warn!("Not setting image to {image}; not yet implemented.");
            errors.push(miette::miette!("setting images is not yet implemented"));
        }

        MultiError::from_vec(errors, "failed to set some dial configurations")
    }
}

struct TextTheme {
    branch: &'static str,
    trunk: &'static str,
    leaf: &'static str,
}

const UNICODE_THEME: TextTheme = TextTheme {
    branch: "├── ",
    trunk: "│  ",
    leaf: "└── ",
};

const ASCII_THEME: TextTheme = TextTheme {
    branch: "+- ",
    trunk: "| ",
    leaf: "+- ",
};

impl OutputMode {
    pub fn print_dial(&self, info: &DialInfo) -> miette::Result<()> {
        fn print_info(dial: &DialInfo, theme: &TextTheme, style: owo_colors::Style) {
            let TextTheme { branch, leaf, .. } = theme;
            println!("DIAL: {}", style.style(&dial.uid));
            println!("{branch}name: {}", style.style(&dial.dial_name));
            println!("{branch}value: {}", style.style(dial.value));
            print_backlight(&dial.backlight, theme, style);
            println!("{leaf}image: {}\n", style.style(&dial.image_file));
        }

        let has_color = supports_color::on(supports_color::Stream::Stdout)
            .map(|s| s.has_basic)
            .unwrap_or(false);
        let style = if has_color {
            owo_colors::Style::new().bold()
        } else {
            owo_colors::Style::new()
        };
        match self {
            OutputMode::Ascii => print_info(info, &ASCII_THEME, style),
            OutputMode::Text => print_info(info, &UNICODE_THEME, style),
            OutputMode::Json => {
                let json = serde_json::to_string_pretty(info).into_diagnostic()?;
                println!("{json}");
            }
        }

        Ok(())
    }

    pub fn print_status(&self, status: &dial::Status) -> miette::Result<()> {
        fn print_status(dial: &dial::Status, theme: &TextTheme, style: owo_colors::Style) {
            let TextTheme {
                branch,
                trunk,
                leaf,
            } = theme;
            println!("DIAL: {}", style.style(&dial.uid));
            println!("{branch}name: {}", style.style(&dial.dial_name));
            println!("{branch}value: {}", style.style(dial.value));
            println!("{branch}index: {}", style.style(dial.index));
            println!("{branch}rgbw: {:?}", style.style(&dial.rgbw));
            println!("{branch}image file: {}", style.style(&dial.image_file));
            let dial::Easing {
                dial_step,
                dial_period,
                backlight_step,
                backlight_period,
            } = dial.easing;
            println!("{branch}DIAL EASING:");
            println!("{trunk} {branch}dial step: {}", style.style(dial_step));
            println!("{trunk} {leaf}dial period: {:?}", style.style(dial_period));
            println!("{branch}BACKLIGHT EASING:");
            println!(
                "{trunk} {branch}backlight step: {}",
                style.style(backlight_step)
            );
            println!(
                "{trunk} {leaf}backlight period: {:?}",
                style.style(backlight_period)
            );
            println!("{branch}VERSION:");
            println!(
                "{trunk} {branch}firmware hash: {}",
                style.style(&dial.fw_hash)
            );
            println!(
                "{trunk} {branch}firmware version: {}",
                style.style(&dial.fw_version)
            );
            println!(
                "{trunk} {branch}hardware version: {}",
                style.style(&dial.hw_version)
            );
            println!(
                "{trunk} {leaf}protocol version: {}",
                style.style(&dial.protocol_version)
            );
            print_backlight(&dial.backlight, theme, style);
            println!("{branch}STATUS:");
            println!(
                "{trunk} {branch}value_changed: {}",
                style.style(dial.value_changed)
            );
            println!(
                "{trunk} {branch}backlight_changed: {}",
                style.style(dial.backlight_changed)
            );
            println!(
                "{trunk} {leaf}image_changed: {}",
                style.style(dial.image_changed)
            );
            println!(
                "{leaf}update deadline: {}\n",
                style.style(dial.update_deadline)
            );
        }
        let has_color = supports_color::on(supports_color::Stream::Stdout)
            .map(|s| s.has_basic)
            .unwrap_or(false);
        let style = if has_color {
            owo_colors::Style::new().bold()
        } else {
            owo_colors::Style::new()
        };
        match self {
            OutputMode::Ascii => print_status(status, &ASCII_THEME, style),
            OutputMode::Text => print_status(status, &UNICODE_THEME, style),
            OutputMode::Json => {
                let json = serde_json::to_string_pretty(status).into_diagnostic()?;
                println!("{json}");
            }
        }

        Ok(())
    }
}
async fn list_dials(
    client: &vu_api::client::Client,
    details: bool,
    output: OutputMode,
) -> miette::Result<()> {
    let dials = client.list_dials().await?;
    let mut errors = Vec::new();
    if details {
        for (dial, info) in dials {
            match dial
                .status()
                .await
                .with_context(|| format!("failed to get detailed status for {dial}"))
            {
                Ok(status) => {
                    if let Err(e) = output.print_status(&status) {
                        errors
                            .push(e.context(format!("failed to print detailed status for {dial}")));
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                    );
                    errors.push(error);

                    if let Err(e) = output.print_dial(&info) {
                        errors
                            .push(e.context(format!("failed to print detailed status for {dial}")));
                    }
                }
            }
        }
    } else {
        for (dial, info) in dials {
            if let Err(e) = output.print_dial(&info) {
                errors.push(e.context(format!("failed to print dial {dial}")));
            }
        }
    }

    MultiError::from_vec(errors, "could not get info for all dials")
}

fn print_backlight(
    dial::Backlight { red, green, blue }: &dial::Backlight,
    TextTheme {
        branch,
        trunk,
        leaf,
    }: &TextTheme,
    style: owo_colors::Style,
) {
    println!("{branch}BACKLIGHT:");
    println!("{trunk} {branch}red: {}", style.style(red));
    println!("{trunk} {branch}green: {}", style.style(green));
    println!("{trunk} {leaf}blue: {}", style.style(blue));
}

impl MultiError {
    fn from_vec(errors: Vec<miette::Report>, msg: &'static str) -> miette::Result<()> {
        if errors.is_empty() {
            return Ok(());
        }

        if errors.len() == 1 {
            return Err(errors.into_iter().next().unwrap());
        }

        Err(MultiError { msg, errors }.into())
    }
}
