use tonic::transport::Server;
use echo::service::{EchoService, echo::service_server::ServiceServer};


const DEFAULT_PORT: u16 = 7878;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:7878".parse()?;
    let echo_service = EchoService::default();


    Server::builder()
        .add_service(ServiceServer::new(echo_service))
        .serve(addr)
        .await?;

    Ok(())
}