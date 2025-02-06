use clap::{Arg, Command};
use echo_server::config::Config;
use echo_server::db::SqlHelper;
use echo_server::servers::manager::start_manager_server;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let matches = Command::new("echo-chat-server")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("config file path"),
        )
        .get_matches();

    let path = matches
        .get_one::<String>("config")
        .map_or("./config/manager.yaml".to_string(), |s| s.clone());
    let config = Config::load(path)?;
    let sql_helper = SqlHelper::new(&config.db).await?;

    start_manager_server(sql_helper, &config.server).await?;
    Ok(())
}
