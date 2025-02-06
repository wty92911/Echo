use clap::{Arg, Command};
use echo_server::config::Config;
use echo_server::db::SqlHelper;
use echo_server::servers::chat_server::start_chat_server;
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
        .arg(
            Arg::new("mgr_addr")
                .short('m')
                .long("mgr_addr")
                .help("manager address"),
        )
        .get_matches();

    let path = matches
        .get_one::<String>("config")
        .map_or("./config/manager.yaml".to_string(), |s| s.clone());

    let mgr_addr = matches
        .get_one::<String>("mgr_addr")
        .map_or("127.0.0.1:8080".to_string(), |s| s.clone());

    let config = Config::load(path)?;
    let sql_helper = SqlHelper::new(&config.db).await?;

    start_chat_server(sql_helper, &config.server, &mgr_addr).await?;
    Ok(())
}
