use clap::Parser;

#[tokio::main(flavor = "current_thread")]
async fn main() -> miette::Result<()> {
    vupdaters::daemon::Args::parse().run().await
}
