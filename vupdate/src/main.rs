use clap::Parser;
use miette::{Context, IntoDiagnostic};
use vupdate::cli::{Args, Command, DaemonCommand, DialCommand};

#[tokio::main(flavor = "current_thread")]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let client =
        vu_api::client::Client::new(args.key, args.server).context("failed to build client")?;

    match args.command {
        Command::Dials { details } => {
            list_dials(&client, details).await?;
        }

        Command::Dial {
            uid,
            command: DialCommand::Status,
        } => {
            let status = client
                .dial_status(&uid)
                .await
                .with_context(|| format!("failed to get status for dial {uid}"))?;
            println!("{status:#?}");
        }

        Command::Daemon(DaemonCommand { config, gen_config }) => {
            if gen_config {
                vupdate::daemon::gen_config(&client, config).await?;
            } else {
                let config = {
                    let file = std::fs::read_to_string(&config)
                        .into_diagnostic()
                        .with_context(|| format!("failed to read config file {config}"))?;
                    toml::from_str(&file)
                        .into_diagnostic()
                        .with_context(|| format!("failed to parse config file {config}"))?
                };
                tokio::spawn(vupdate::daemon::run(client, config))
                    .await
                    .into_diagnostic()
                    .context("daemon main task panicked")??;
            }
        }
    }

    Ok(())
}

async fn list_dials(client: &vu_api::client::Client, details: bool) -> miette::Result<()> {
    let dials = client.list_dials().await?;

    if details {
        for dial in &dials {
            match client.dial_status(&dial.uid).await {
                Ok(status) => println!("{status:#?}\n"),
                Err(error) => {
                    eprintln!(
                        "failed to get detailed status for dial {}: {error}",
                        dial.uid,
                    );

                    println!("{dial:#?}\n");
                }
            }
        }
        return Ok(());
    }

    for dial in dials {
        println!("DIAL: {}", dial.uid);
        println!("├─name: {}", dial.dial_name);
        println!("├─value: {}", dial.value);
        println!("├─backlight:");
        println!("│ ├─red: {}", dial.backlight.red);
        println!("│ ├─green: {}", dial.backlight.green);
        println!("│ └─blue: {}", dial.backlight.blue);
        println!("└─image: {}\n", dial.image_file);
    }
    Ok(())
}
