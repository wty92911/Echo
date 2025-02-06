use abi::pb::user_service_client::UserServiceClient;
use abi::pb::{LoginRequest, RegisterRequest};
use echo_server::config::Config;
use echo_server::servers::chat_server::start_chat_server;
use echo_server::servers::manager::start_manager_server;
use sqlx_db_tester::TestPg;
use std::time::Duration;
use tonic::Request;
pub async fn init_manager_server(
    server_port: u16,
) -> (Config, tokio::task::JoinHandle<()>, TestPg) {
    let tdb = TestPg::new(
        "postgres://postgres:postgres@localhost:5432".to_string(),
        std::path::Path::new("../migrations"),
    );
    println!("db name: {}", tdb.dbname);
    let pool = tdb.get_pool().await;
    let mut config = Config::load("../config/manager_test.yaml").unwrap();
    config.server.port = server_port; //change port to support multiple tests in different threads.

    let join_handle = start_manager_server(pool.into(), &config.server)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    (config, join_handle, tdb)
}

#[allow(dead_code)]
pub async fn init_chat_server(
    server_port: u16,
    tdb: &TestPg,
    manager_addr: &str,
) -> (Config, tokio::task::JoinHandle<()>) {
    let pool = tdb.get_pool().await;
    let mut config = Config::load("../config/manager.yaml").unwrap();
    config.server.port = server_port; //change port to support multiple tests in different threads.

    let join_handle = start_chat_server(pool.into(), &config.server, manager_addr)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    (config, join_handle)
}

#[allow(dead_code)]
pub fn intercept_token<T>(mut req: Request<T>, token: &str) -> Request<T> {
    req.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", token).parse().unwrap(),
    );
    req
}

#[allow(dead_code)]
pub async fn register_login(id: &str, conn: tonic::transport::Channel) -> String {
    let mut client = UserServiceClient::new(conn.clone());

    client
        .register(RegisterRequest {
            user_id: id.to_string(),
            password: format!("{}_password", id).to_string(),
            name: format!("{}_name", id).to_string(),
        })
        .await
        .unwrap();

    client
        .login(LoginRequest {
            user_id: id.to_string(),
            password: format!("{}_password", id).to_string(),
        })
        .await
        .unwrap()
        .into_inner()
        .token
}
