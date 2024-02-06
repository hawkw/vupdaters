use clap::Parser;

#[tokio::main(flavor = "current_thread")]
async fn main() -> miette::Result<()> {
    vupdaters::dialctl::Args::parse().run().await
}
