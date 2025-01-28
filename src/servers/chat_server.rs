use crate::chat_service_server::ChatServiceServer;
use crate::client::ChannelClient;
use crate::ReportRequest;
use crate::{config::ServerConfig, db::SqlHelper, Channel, Message};
use log::{error, info};
use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status, Streaming};
#[derive(Debug)]
#[allow(unused)]
pub struct ChatService {
    manager_addr: String,
    config: ServerConfig,
    sql_helper: SqlHelper,
    tx: tokio::sync::mpsc::Sender<ReportRequest>,
}

impl ChatService {
    pub async fn new(manager_addr: String, config: &ServerConfig, sql_helper: SqlHelper) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let mut client = ChannelClient::new(&manager_addr, &config.secret).await;
        client.report(config.url_with(false), rx).await.unwrap(); // todo handle err, and deal with retrying.
        Self {
            manager_addr,
            config: config.clone(),
            sql_helper,
            tx,
        }
    }
}

#[tonic::async_trait]
impl crate::chat_service_server::ChatService for ChatService {
    type ConnStream = Pin<Box<dyn Stream<Item = std::result::Result<Message, Status>> + Send>>;

    async fn conn(
        &self,
        _request: Request<Streaming<Message>>,
    ) -> Result<Response<Self::ConnStream>, Status> {
        // let user_id = request.metadata().get("user_id").unwrap().to_str().unwrap();

        todo!()
    }

    async fn add(&self, _request: Request<Streaming<Channel>>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn remove(&self, _request: Request<Streaming<Channel>>) -> Result<Response<()>, Status> {
        todo!()
    }
}

pub async fn start_chat_server(
    sql_helper: SqlHelper,
    config: &ServerConfig,
    manager_addr: &str,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
    let addr: std::net::SocketAddr = config.url().parse()?;
    info!("start chat server at {}", addr);

    let server = tonic::transport::Server::builder()
        .add_service(ChatServiceServer::new(
            ChatService::new(manager_addr.to_string(), config, sql_helper).await,
        ))
        .serve(addr);

    Ok(tokio::spawn(async move {
        if let Err(e) = server.await {
            error!("failed to run chat server: {}", e);
        }
    }))
}
