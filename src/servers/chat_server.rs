use crate::chat_service_server::ChatServiceServer;
use crate::client::ChannelClient;
use crate::error::Error;
use crate::{config::ServerConfig, db::SqlHelper, Channel, Message};
use crate::{get_claims_from, ReportRequest};
use chrono::Utc;
use dashmap::DashMap;
use log::{error, info};
use std::pin::Pin;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tonic::{Request, Response, Status, Streaming};
#[derive(Debug)]
#[allow(unused)]
pub struct ChatService {
    manager_addr: String,
    config: ServerConfig,
    sql_helper: SqlHelper,
    tx: tokio::sync::mpsc::Sender<ReportRequest>,

    // for chat
    core: DashMap<i32, ChannelCore>,
}

/// !Concurrent Safe Channel Core Logic
///
/// Holds all channels
#[derive(Debug)]
struct ChannelCore {
    broadcast: broadcast::Sender<Message>,
}

impl ChannelCore {
    fn new() -> Self {
        Self {
            broadcast: broadcast::channel(32).0,
        }
    }

    pub fn broadcast(&self) -> &broadcast::Sender<Message> {
        &self.broadcast
    }
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
            core: DashMap::new(),
        }
    }
}

#[tonic::async_trait]
impl crate::chat_service_server::ChatService for ChatService {
    type ConnStream = Pin<Box<dyn Stream<Item = std::result::Result<Message, Status>> + Send>>;

    async fn conn(
        &self,
        request: Request<Streaming<Message>>,
    ) -> Result<Response<Self::ConnStream>, Status> {
        info!("conn request: {:?}", request);
        let claims = get_claims_from!(request, &self.config.secret);
        let user_id = claims.user_id.clone();
        let channel_id = claims.channel_id;
        // todo: limiter by user_id, channel_id

        let mut inbound = request.into_inner();
        let channel_core = self.core.entry(channel_id).or_insert_with(ChannelCore::new);
        let broadcast = channel_core.broadcast().clone();
        tokio::spawn(async move {
            while let Ok(msg) = inbound.message().await {
                if let Some(mut msg) = msg {
                    msg.user_id = user_id.clone();
                    msg.timestamp = Utc::now().timestamp_millis();
                    info!("receive msg: {:?} from {}-{}", msg, user_id, channel_id);
                    broadcast.send(msg).unwrap(); // todo: handle err
                }
            }
            info!("{}-{} connection closed", user_id, channel_id);
        });

        let user_id = claims.user_id.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let mut outbound = channel_core.broadcast().subscribe();
        tokio::spawn(async move {
            while let Ok(msg) = outbound.recv().await {
                info!("send msg: {:?} to {}-{}", msg, user_id, channel_id);
                if let Err(err) = tx.send(Ok(msg)).await {
                    error!("send msg to {}-{} failed: {}", user_id, channel_id, err);
                }
            }
            let _ = tx.send(Err(Error::ChannelBroadcastStopped.into())).await; // do not consider
            info!("{}-{} stop recv channel's broadcast", user_id, channel_id);
        });
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        let stream = Box::pin(stream);
        Ok(Response::new(stream))
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
