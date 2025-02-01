mod user;
use log::info;
use user::*;
mod channel;
use channel::*;
mod server;
use crate::{
    channel_service_server::ChannelServiceServer, config::ServerConfig, db::SqlHelper,
    user_service_server::UserServiceServer,
};

#[macro_export]
macro_rules! get_claims_from {
    ($request:expr, $secret:expr) => {{
        let authorization = $request
            .metadata()
            .get("authorization")
            .ok_or_else(|| tonic::Status::unauthenticated("No auth token provided"))?
            .to_str()
            .map_err(|e| tonic::Status::unauthenticated(e.to_string()))?;
        let token = &authorization["Bearer ".len()..];

        let claims: $crate::auth::interceptor::Claims =
            $crate::auth::interceptor::extract($secret, token)?;
        claims
    }};
}

// user and manager services are on manager_server
pub async fn start_manager_server(
    sql_helper: SqlHelper,
    config: &ServerConfig,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
    let user_svc = UserService::new(config.secret.clone(), sql_helper.clone());
    let channel_svc = ChannelService::new(config, sql_helper);

    let addr: std::net::SocketAddr = config.url().parse()?;
    info!("start manager server at {}", addr);

    let server = tonic::transport::Server::builder()
        .add_service(UserServiceServer::new(user_svc))
        .add_service(ChannelServiceServer::new(channel_svc))
        .serve(addr);

    Ok(tokio::spawn(async move {
        if let Err(e) = server.await {
            eprintln!("Server error: {}", e);
        }
    }))
}
