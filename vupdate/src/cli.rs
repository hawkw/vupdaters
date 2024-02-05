use camino::{Utf8Path, Utf8PathBuf};

#[derive(Debug, clap::Parser)]
#[command(author, version, about)]
#[command(propagate_version = true)]
pub struct Args {
    /// The server API key.
    #[clap(long, short = 'k', env = "VU_DIALS_API_KEY")]
    pub key: String,

    #[clap(
        long,
        short = 's',
        env = "VU_DIALS_SERVER_ADDR",
        default_value = "http://localhost:5340",
        value_hint = clap::ValueHint::Url,
        global = true
    )]
    pub server: reqwest::Url,

    #[clap(subcommand)]
    pub command: Command,
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
        uid: vu_api::api::DialId,
        #[clap(subcommand)]
        command: DialCommand,
    },

    /// Run a daemon process updating the dials with system status information.
    Daemon(DaemonCommand),
}

#[derive(Debug, clap::Subcommand)]
pub enum DialCommand {
    /// Get detailed status information about this dial.
    Status,
    /// Set a dial's value.
    Set {
        /// The new value to set the dial to.
        value: vu_api::api::Value,
    },
    /// Set the dial's background image.
    SetImage {
        /// Path to the new image file.
        #[clap(value_hint = clap::ValueHint::FilePath)]
        path: Utf8PathBuf,
    },
}

#[derive(Debug, clap::Parser)]
pub struct DaemonCommand {
    #[clap(long)]
    pub gen_config: bool,

    #[clap(
        long,
        short = 'c',
        default_value_t = default_config_path(),
        value_hint = clap::ValueHint::FilePath,
    )]
    pub config: Utf8PathBuf,
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
