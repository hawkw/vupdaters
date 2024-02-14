use clap::Parser;
use miette::{Context, IntoDiagnostic};
use tokio::{runtime, task::LocalSet};

fn main() -> miette::Result<()> {
    let app = vupdaters::daemon::Args::parse();
    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .into_diagnostic()
        .context("failed to build Tokio runtime! something is very messed up")?;

    let local = LocalSet::new();
    local.block_on(&rt, app.run())
}
