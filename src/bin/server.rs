use echo::config::Config;
use echo::servers::manager_server::start_manager_server;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let path = std::env::var("CONFIG_PATH")?;
    let config = Config::load(path)?;
    start_manager_server(&config).await?;
    Ok(())
}
