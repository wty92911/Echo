use abi::pb::{
    channel_service_client::ChannelServiceClient, user_service_client::UserServiceClient, Channel,
    LoginRequest, RegisterRequest,
};
use std::str::FromStr;
use tonic::transport::Endpoint;
use tonic::Request;
mod common;
use common::server::init_manager_server;

#[tokio::test]
async fn test_register_and_login() {
    let (config, join_handle, tdb) = init_manager_server(50051).await;

    let mut client = UserServiceClient::connect(config.server.url_with(false))
        .await
        .unwrap();

    client
        .register(RegisterRequest {
            user_id: "test".to_string(),
            password: "test_password".to_string(),
            name: "test_name".to_string(),
        })
        .await
        .unwrap();

    let _ = client
        .login(LoginRequest {
            user_id: "test".to_string(),
            password: "test_password".to_string(),
        })
        .await
        .unwrap();

    join_handle.abort();
    drop(tdb);
}

#[tokio::test]
async fn test_interceptor() {
    // start server
    let (config, join_handle, tdb) = init_manager_server(50052).await;

    let addr = config.server.url_with(false);
    let conn = Endpoint::from_str(&addr).unwrap().connect().await.unwrap();

    let mut client = UserServiceClient::new(conn.clone());
    client
        .register(RegisterRequest {
            user_id: "test".to_string(),
            password: "test_password".to_string(),
            name: "test_name".to_string(),
        })
        .await
        .unwrap();

    let mut chan_client = ChannelServiceClient::new(conn.clone());
    // check invalid or no token
    let rsp = chan_client.list(Request::new(Channel::default())).await;
    assert!(rsp.is_err());

    let token = client
        .login(LoginRequest {
            user_id: "test".to_string(),
            password: "test_password".to_string(),
        })
        .await
        .unwrap()
        .into_inner()
        .token;

    let mut req = Request::new(Channel::default());
    req.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", token).parse().unwrap(),
    );

    // check valid token
    let rsp = chan_client.list(req).await;
    assert!(rsp.is_ok());

    // stop
    join_handle.abort();
    drop(tdb);
}
