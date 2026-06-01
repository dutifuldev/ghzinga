#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ghzinga::runner::run_from_cli().await
}
