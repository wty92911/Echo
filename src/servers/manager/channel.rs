use crate::auth::interceptor::{encrypt, Claims};
use crate::auth::limiter::{FixedWindowLimiter, Limiter, LimiterConfig};
use crate::auth::validator::Validator;
use crate::config::ServerConfig;
use crate::db::SqlHelper;
use crate::servers::manager::server::ServerManager;
use crate::{error::*, get_claims_from, ChannelServer, ListResponse};
use crate::{Channel, ListenResponse, ReportRequest};
use chrono::Utc;
use dashmap::DashMap;
use log::{error, info};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status, Streaming};

/// Channel Service Implements:
/// as core service on manager server.
///
/// *list, create, delete* for channels.
///
/// *listen* is for users to listen to some channel.
///
/// *report* is for chat_server to report messages.
///
/// ChannelService will reload all channels from database to svr_manager when it starts.
///
/// todo: client to communicate with chat server.
#[derive(Debug)]
pub struct ChannelService {
    config: ServerConfig,
    sql_helper: SqlHelper,
    svr_manager: Arc<RwLock<ServerManager>>,
    channel_info: Arc<DashMap<i32, Channel>>, // channel info from servers

    limiter: FixedWindowLimiter,
}

impl ChannelService {
    pub fn new(config: &ServerConfig, sql_helper: SqlHelper) -> Self {
        Self {
            config: config.clone(),
            sql_helper,
            svr_manager: Arc::new(RwLock::new(ServerManager::new())),
            channel_info: Arc::new(DashMap::new()),
            limiter: FixedWindowLimiter::new(LimiterConfig::new(1, Duration::from_secs(3))),
        }
    }
}

#[tonic::async_trait]
impl crate::channel_service_server::ChannelService for ChannelService {
    /// list channels by request id
    /// if id is empty, return all channels
    /// if id is not empty, return channels by id
    async fn list(&self, request: Request<Channel>) -> Result<Response<ListResponse>, Status> {
        // todo: load channel from channel_info
        let _ = get_claims_from!(request, &self.config.secret);
        let channel_id = request.get_ref().id;
        info!("list channel request: {:?}", channel_id);
        let channels = self.sql_helper.get_channels(&channel_id).await?;
        Ok(Response::new(ListResponse { channels }))
    }

    /// create channel, generate serial number as id, and set owner
    async fn create(&self, request: Request<Channel>) -> Result<Response<Channel>, Status> {
        let channel = request.get_ref().clone();
        let claims = get_claims_from!(request, &self.config.secret);
        let user_id = claims.user_id;
        info!(
            "create channel request: {:?} by user: {:?}",
            channel, user_id
        );

        channel.validate()?;
        let id = self.sql_helper.insert_channel(&channel, &user_id).await?;
        self.svr_manager.write().await.add_channel(&id);
        Ok(Response::new(Channel { id, ..channel }))
    }

    /// delete channel by id
    /// check channel owner is or not the user
    /// if not, return error
    /// check channel is not using, give server a shutdown signal
    async fn delete(&self, request: Request<Channel>) -> Result<Response<()>, Status> {
        let channel = request.get_ref();
        let claims = get_claims_from!(request, &self.config.secret);
        let user_id = claims.user_id;
        let owner_id = self.sql_helper.get_channel_owner(&channel.id).await?;
        if let Some(owner_id) = owner_id {
            if user_id == owner_id {
                info!("delete channel request: {:?}", channel);
                self.sql_helper.delete_channel(&channel.id).await?;
                self.svr_manager.write().await.delete_channel(&channel.id);
                Ok(Response::new(()))
            } else {
                Err(Error::PermissionDenied("user is not the channel's owner").into())
            }
        } else {
            Err(Error::ChannelNotFound.into())
        }
    }

    /// user tries to listen to some channel
    /// return addr of the channel's server
    /// if channel not found, return error
    /// don't need to check channel's logic, it will be checked on specific server.
    ///
    /// if user wants to listen a new channel, manager server will send a shutdown signal of old channel to that chat server.
    ///
    ///
    async fn listen(&self, request: Request<Channel>) -> Result<Response<ListenResponse>, Status> {
        info!("listen channel request: {:?}", request);
        let claims = get_claims_from!(request, &self.config.secret);
        let user_id = claims.user_id;
        self.limiter.is_allowed(&user_id).await?;

        let channel = request.get_ref();
        let mgr = self.svr_manager.read().await;
        let addr = mgr.get_server(&channel.id)?;

        Ok(Response::new(ListenResponse {
            token: encrypt(
                &self.config.secret,
                &Claims {
                    exp: Utc::now().timestamp() + 5,
                    user_id: user_id.to_string(),
                    channel_id: channel.id,
                    addr: addr.clone(),
                    ..Claims::default()
                },
            ),
            server: Some(ChannelServer {
                addr: addr.clone(),
                ..ChannelServer::default()
            }),
        }))
    }

    // chat server will report to manager, here we use `token` as server_addr to identify server
    // server and manager will use same `secret` to encrypt and decrypt token
    async fn report(
        &self,
        request: Request<Streaming<ReportRequest>>,
    ) -> Result<Response<()>, Status> {
        info!("report request: {:?}", request);
        let claims = get_claims_from!(request, &self.config.secret);
        let server_addr = claims.addr;

        let mgr = self.svr_manager.clone();
        let channel_info = self.channel_info.clone();
        tokio::spawn(async move {
            info!("add server: {}", server_addr);
            // if use let mgr = mgr.write().await, we will drop mgr after this line.
            // to avoid of dead lock
            mgr.write().await.add_server(&server_addr);
            // change channel's belonging server
            let mut stream = request.into_inner();
            while let Ok(report) = stream.message().await {
                if let Some(report) = report {
                    // todo: handle metric
                    info!("report: {:?} from: {}", report, &server_addr);
                    for channel in report.channels.into_iter() {
                        // check if channel is not belong to server, todo: shutdown and why it exists?
                        if let Ok(addr) = mgr.read().await.get_server(&channel.id) {
                            if addr == server_addr {
                                // accept it
                                channel_info.insert(channel.id, channel);
                            } else {
                                error!("server: {:?} takes channel: {:?}, but it actually belongs to server: {:?}", server_addr, channel, addr);
                            }
                        } else {
                            error!("channel: {:?} doesn't belong to any server", channel);
                        }
                    }
                }
            }

            mgr.write().await.delete_server(&server_addr);
        });

        Ok(Response::new(()))
    }
}
