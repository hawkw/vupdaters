use miette::{Context, IntoDiagnostic};

#[derive(Clone, Debug, clap::Args)]
#[command(next_help_heading = "VU-Server Client Options")]
pub struct ClientArgs {
    /// The server API key.
    #[clap(long, short = 'k', env = "VU_DIALS_API_KEY")]
    key: String,

    /// The hostname of the VU-Server instance to connect to.
    #[clap(
        long,
        short = 's',
        env = "VU_DIALS_SERVER_ADDR",
        default_value = "http://localhost:5340",
        value_hint = clap::ValueHint::Url,
        global = true
    )]
    server: reqwest::Url,
}

#[derive(Clone, Debug, clap::Args)]
#[command(next_help_heading = "Output Options")]
pub struct OutputArgs {
    /// A list of log-level filters for `tracing-subscriber`.
    #[clap(
        long = "trace",
        env = "RUST_LOG",
        global = true,
        default_value = "info"
    )]
    filter: tracing_subscriber::filter::Targets,

    /// If set, log to the system journal, instead of stderr.
    #[clap(long, global = true)]
    journald: bool,

    /// If set, forcibly disable ANSI colors in stderr output.
    ///
    /// See https://no-color.org/
    #[clap(
        long,
        env = "NO_COLOR",
        value_parser = clap::builder::BoolishValueParser::new(),
        global = true,
    )]
    no_color: bool,
}

impl ClientArgs {
    pub fn into_client(self) -> Result<vu_api::client::Client, vu_api::client::NewClientError> {
        vu_api::client::Client::new(self.key, self.server)
    }
}

impl OutputArgs {
    pub fn init_tracing(self) -> miette::Result<()> {
        use tracing_subscriber::{fmt, prelude::*};
        let subcriber = tracing_subscriber::registry().with(self.filter);
        if self.journald {
            let layer = tracing_journald::layer()
                .into_diagnostic()
                .context("could not connect to journald!")?;
            subcriber.with(layer).init();
        } else {
            let has_ansi_colors = supports_color::on(supports_color::Stream::Stderr)
                .map(|colors| colors.has_basic)
                .unwrap_or(false);
            let fmt = fmt::layer()
                .with_span_events(fmt::format::FmtSpan::CLOSE)
                .with_timer(fmt::time::uptime())
                .with_writer(std::io::stderr)
                .with_ansi(has_ansi_colors);
            subcriber.with(fmt).init();
        }

        Ok(())
    }
}
