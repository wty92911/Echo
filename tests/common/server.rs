use echo::config::Config;
use std::time::Duration;
use tonic::{service::Interceptor, Request, Status};

use echo::servers::manager_server::start_manager_server;
use sqlx_db_tester::TestPg;
pub async fn init(server_port: u16) -> (Config, tokio::task::JoinHandle<()>, TestPg) {
    let tdb = TestPg::new(
        "postgres://postgres:postgres@localhost:5432".to_string(),
        std::path::Path::new("./migrations"),
    );
    println!("db name: {}", tdb.dbname);
    let pool = tdb.get_pool().await;

    let mut config = Config::load("./config/manager.yaml").unwrap();
    config.server.port = server_port; //change port to support multiple tests in different threads.

    let join_handle = start_manager_server(pool.into(), &config.server)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    (config, join_handle, tdb)
}
#[allow(dead_code)]
pub fn create_interceptor(token: String) -> impl Interceptor {
    move |mut req: Request<()>| -> Result<Request<()>, Status> {
        req.metadata_mut().insert(
            "authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );
        Ok(req)
    }
}
