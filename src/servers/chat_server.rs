use crate::chat_service_server::ChatServiceServer;
use crate::client::ChannelClient;
use crate::error::Error;
use crate::{config::ServerConfig, db::SqlHelper, Channel, Message};
use crate::{get_claims_from, ReportRequest, ShutdownRequest, User};
use chrono::Utc;
use dashmap::DashMap;
use log::{error, info};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::mpsc::Sender;
use tokio_stream::Stream;
use tonic::{Request, Response, Status, Streaming};
#[derive(Debug)]
#[allow(unused)]
pub struct ChatService {
    manager_addr: String,
    config: ServerConfig,
    sql_helper: SqlHelper,

    // for chat
    core: Arc<DashMap<i32, ChannelCore>>, // drop channel when no one exists
}

/// !Concurrent Safe Channel Core Logic
///
#[derive(Debug)]
struct ChannelCore {
    pub id: i32,
    pub name: String,
    pub limit: i32,
    pub broadcast: broadcast::Sender<Message>,
    // record shutdown_tx for every user on this channel，Key is user_id
    user_shutdown_txs: DashMap<String, broadcast::Sender<()>>,
}

impl ChannelCore {
    fn new(channel: Channel) -> Self {
        Self {
            id: channel.id,
            name: channel.name,
            limit: channel.limit,
            broadcast: broadcast::channel(32).0,
            user_shutdown_txs: DashMap::new(),
        }
    }

    pub fn is_full(&self) -> bool {
        self.user_shutdown_txs.len() >= self.limit as usize
    }

    // add user's shutdown_tx
    pub fn add_user_shutdown_tx(&self, user_id: String, shutdown_tx: broadcast::Sender<()>) {
        self.user_shutdown_txs.insert(user_id, shutdown_tx);
    }

    // remove specific user from current channel
    fn shutdown_user(&self, user_id: &str) {
        if let Some((_, shutdown_tx)) = self.user_shutdown_txs.remove(user_id) {
            let _ = shutdown_tx.send(());
        }
    }

    pub fn exist_user(&self, user_id: &str) -> bool {
        self.user_shutdown_txs.contains_key(user_id)
    }
}

impl Drop for ChannelCore {
    fn drop(&mut self) {
        for shutdown_tx in self.user_shutdown_txs.iter() {
            let _ = shutdown_tx.send(());
        }
        self.user_shutdown_txs.clear();
    }
}
impl ChatService {
    pub async fn new(manager_addr: String, config: &ServerConfig, sql_helper: SqlHelper) -> Self {
        Self {
            manager_addr,
            config: config.clone(),
            sql_helper,
            core: Arc::new(DashMap::new()),
        }
        .register()
        .await
    }

    // register chat service on manager
    async fn register(self) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let mut client = ChannelClient::new(&self.manager_addr, &self.config.secret).await;
        let rsp = client
            .report(self.config.url_with(false), rx)
            .await
            .unwrap(); // todo handle err, and deal with retrying.

        let core = Arc::clone(&self.core);
        let mut stream = rsp.into_inner();
        tokio::spawn(async move {
            while let Ok(rsp) = stream.message().await {
                if let Some(rsp) = rsp {
                    // 1. check shutdown signal
                    if let Some(req) = rsp.shutdown {
                        info!("shutdown channel: {}", req.channel_id);
                        if let Some(channel_core) = core.get(&req.channel_id) {
                            if let Some(user_id) = req.user_id {
                                channel_core.shutdown_user(&user_id);
                            } else {
                                core.remove(&req.channel_id);
                            }
                        } else {
                            error!("channel: {} not found", req.channel_id);
                        }
                    }
                } else {
                    break;
                }
            }
        });
        self.report(tx, Duration::from_secs(self.config.report_duration));
        self
    }

    fn report(&self, tx: Sender<ReportRequest>, d: Duration) {
        // report channels
        let core = Arc::clone(&self.core);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(d).await;
                let mut vec = vec![];
                for channel in core.iter() {
                    vec.push(Channel {
                        id: channel.id,
                        name: channel.name.clone(),
                        limit: channel.limit,
                        users: channel
                            .user_shutdown_txs
                            .iter()
                            .map(|v| User {
                                id: v.key().to_string(),
                                ..Default::default()
                            })
                            .collect(),
                    })
                }

                if let Err(e) = tx
                    .send(ReportRequest {
                        channels: vec,
                        ..Default::default()
                    })
                    .await
                {
                    error!("report tx error: {}", e);
                    break;
                }
            }
        });
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
    /// And when `listen` changes users' channel, `Redis` will send a shutdown signal to chat server.
    ///
    async fn conn(
        &self,
        request: Request<Streaming<Message>>,
    ) -> Result<Response<Self::ConnStream>, Status> {
        info!("conn request: {:?}", request);
        let claims = get_claims_from!(request, &self.config.secret);
        // check claims.addr is equal to localhost.
        if claims.addr != self.config.url_with(false) {
            return Err(Error::PermissionDenied("wrong request chat server's addr").into());
        }

        let (user_id, channel_id) = (claims.user_id.clone(), claims.channel_id);
        // if channel not exists, add it
        // channel not update when exists users, and will update util removed and first user join it
        if !self.core.contains_key(&channel_id) {
            let channel = self
                .sql_helper
                .get_channels(&channel_id)
                .await?
                .pop()
                .unwrap();
            self.core.insert(channel_id, ChannelCore::new(channel));
        }
        let channel_core = self.core.get_mut(&channel_id).unwrap();

        // check if user is in channel
        if channel_core.exist_user(&user_id) {
            return Err(Error::InvalidRequest("user already in channel").into());
        }

        // check channel limit
        if channel_core.is_full() {
            return Err(Error::InvalidRequest("channel is full").into());
        }

        // Initializing streams and channels
        let broadcast = channel_core.broadcast.clone();
        let inbound = request.into_inner();
        let outbound: broadcast::Receiver<Message> = channel_core.broadcast.subscribe();
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        channel_core.add_user_shutdown_tx(user_id.clone(), shutdown_tx.clone());

        let core: Arc<DashMap<i32, ChannelCore>> = Arc::clone(&self.core);
        tokio::spawn(async move {
            run_connection_tasks(
                user_id.clone(),
                channel_id,
                broadcast,
                inbound,
                outbound,
                tx,
                shutdown_tx,
            )
            .await;
            // to remove user from channel
            if let Some(channel_core) = core.get_mut(&channel_id) {
                channel_core.shutdown_user(&user_id);
            }
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

    /// deprecated
    /// shutdown user-channel connection for manager.
    async fn shutdown(&self, _request: Request<ShutdownRequest>) -> Result<Response<()>, Status> {
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
