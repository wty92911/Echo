use echo::config::Config;
use echo::db::SqlHelper;
use echo::servers::manager_server::start_manager_server;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let path = std::env::var("CONFIG_PATH")?;
    let config = Config::load(path)?;
    let sql_helper = SqlHelper::new(&config.db).await?;

    start_manager_server(sql_helper, &config.server).await?;
    Ok(())
}
