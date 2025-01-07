use tokio::net::TcpListener;
use tokio::signal;
use Echo::server;

const DEFAULT_PORT: u16 = 7878;
#[tokio::main]
pub async fn main() -> Echo::Result<()> {


    // Bind a TCP listener
    let listener = TcpListener::bind(&format!("127.0.0.1:{}", DEFAULT_PORT)).await?;

    server::run(listener, signal::ctrl_c()).await;

    Ok(())
}