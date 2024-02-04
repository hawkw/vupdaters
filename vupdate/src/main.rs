#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = vu_api::client::Client::new(
        "cTpAWYuRpA2zx75Yh961Cg".to_string(),
        "http://localhost:5340".to_string(),
    );
    let dials = client.list_dials().await?;
    println!("{dials:#?}");
    Ok(())
}
