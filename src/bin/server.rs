use echo::auth::Interceptor;
use echo::core::EchoCore;
use echo::{
    service::{EchoService, UserService},
    service_server::ServiceServer,
    user_service_server::UserServiceServer,
};
use std::sync::Arc;
use tonic::transport::Server;

const DEFAULT_PORT: u16 = 7878;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("[::1]:{}", DEFAULT_PORT).parse()?;
    let core = Arc::new(EchoCore::new());
    let echo_service = EchoService::new(core.clone());
    let echo_service =
        ServiceServer::with_interceptor(echo_service, Interceptor::new(core.clone()));

    let user_service = UserServiceServer::new(UserService::new(core.clone()));

    Server::builder()
        .add_service(user_service)
        .add_service(echo_service)
        .serve(addr)
        .await?;

    Ok(())
}
