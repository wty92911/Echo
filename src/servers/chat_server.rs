use crate::chat_service_server::ChatServiceServer;
use crate::client::ChannelClient;
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

async fn run_connection_tasks(
    user_id: String,
    channel_id: i32,
    broadcast: broadcast::Sender<Message>,
    inbound: Streaming<Message>,
    outbound: broadcast::Receiver<Message>,
    tx: tokio::sync::mpsc::Sender<Result<Message, Status>>,
    shutdown_tx: broadcast::Sender<()>,
) {
    let inbound_task = spawn_inbound_task(
        user_id.clone(),
        channel_id,
        broadcast,
        inbound,
        shutdown_tx.clone(),
    );
    let outbound_task = spawn_outbound_task(user_id.clone(), channel_id, outbound, tx, shutdown_tx);

    let _ = tokio::join!(inbound_task, outbound_task); // make sure both tasks are finished
    info!(
        "user_id: {}, channel_id: {} fully disconnected",
        user_id, channel_id
    );
}

fn spawn_inbound_task(
    user_id: String,
    channel_id: i32,
    broadcast: broadcast::Sender<Message>,
    mut inbound: Streaming<Message>,
    shutdown_tx: broadcast::Sender<()>,
) -> tokio::task::JoinHandle<()> {
    let mut shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                res = inbound.message() => match res {
                    Ok(Some(mut msg)) => {
                        msg.user_id = user_id.to_string();
                        msg.timestamp = Utc::now().timestamp_millis();
                        info!("receive msg: {:?} from {}-{}", msg, user_id, channel_id);
                        broadcast.send(msg).unwrap(); // todo: handle err
                    }
                    Ok(None) => {
                        info!("receive None, closing connection for {}-{}", user_id, channel_id);
                        break;
                    }
                    Err(e) => {
                        error!("inbound message receives error: {:?}", e);
                        break;
                    }
                },
                _ = shutdown_rx.recv() => {
                    info!("inbound task received shutdown signal for {}-{}", user_id, channel_id);
                    break;
                }
            }
        }
        info!("{}-{} inbound connection closed", user_id, channel_id);
        let _ = shutdown_tx.send(()); // signal shutdown to the other task
    })
}

fn spawn_outbound_task(
    user_id: String,
    channel_id: i32,
    mut outbound: broadcast::Receiver<Message>,
    tx: tokio::sync::mpsc::Sender<Result<Message, Status>>,
    shutdown_tx: broadcast::Sender<()>,
) -> tokio::task::JoinHandle<()> {
    let mut shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                res = outbound.recv() => match res {
                    Ok(msg) => {
                        info!("send msg: {:?} to {}-{}", msg, user_id, channel_id);
                        if let Err(err) = tx.send(Ok(msg)).await {
                            error!("send msg to {}-{} failed: {}", user_id, channel_id, err);
                        }
                    }
                    Err(_) => {
                        error!("outbound recv error, closing connection for {}-{}", user_id, channel_id);
                        break;
                    }
                },
                _ = shutdown_rx.recv() => {
                    info!("outbound task received shutdown signal for {}-{}", user_id, channel_id);
                    break;
                }
            }
        }
        info!("{}-{} outbound connection closed", user_id, channel_id);
        let _ = shutdown_tx.send(()); // signal shutdown to the other task
    })
}

#[tonic::async_trait]
impl crate::chat_service_server::ChatService for ChatService {
    type ConnStream = Pin<Box<dyn Stream<Item = std::result::Result<Message, Status>> + Send>>;

    /// Conn takes a inbound stream and returns it's channel's broadcast stream.
    ///
    /// Use `tokio::select!` to ensure that the two connections are both closed when one of them disconnects.
    ///
    /// Set timeout to 30mins to avoid of resource wasting.
    ///
    /// todo: use `Redis` to limit by `user_id, channel_id` in distribute server,
    /// only `listen` on manager server will change user-channel-state, here we just check its constraint.
    async fn conn(
        &self,
        request: Request<Streaming<Message>>,
    ) -> Result<Response<Self::ConnStream>, Status> {
        info!("conn request: {:?}", request);
        let claims = get_claims_from!(request, &self.config.secret);
        let (user_id, channel_id) = (claims.user_id.clone(), claims.channel_id);
        let channel_core = self.core.entry(channel_id).or_insert_with(ChannelCore::new);

        // Initializing streams and channels
        let broadcast = channel_core.broadcast().clone();
        let inbound = request.into_inner();
        let outbound = channel_core.broadcast().subscribe();
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        tokio::spawn(async move {
            run_connection_tasks(
                user_id,
                channel_id,
                broadcast,
                inbound,
                outbound,
                tx,
                shutdown_tx,
            )
            .await;
        });

        Ok(Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        )))
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
