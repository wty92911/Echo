use crate::auth::interceptor::{encrypt, AuthTokenInterceptor, Claims};
use crate::auth::validator::Validator;
use crate::channel_service_server::ChannelServiceServer;
use crate::config::ServerConfig;
use crate::db::SqlHelper;
use crate::hash::ConsistentHash;
use crate::pb::{LoginRequest, LoginResponse, RegisterRequest};
use crate::user_service_server::UserServiceServer;
use crate::{error::*, ChannelServer, ListResponse};
use crate::{Channel, ListenResponse, ReportRequest};
use argon2::Argon2;
use chrono::{Duration, Utc};
use log::info;
use password_hash::{PasswordHasher, SaltString};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status, Streaming};
#[derive(Debug)]
pub struct UserService {
    secret: &'static str,
    sql_helper: SqlHelper,
    hash_salt: SaltString,
}

impl UserService {
    pub fn new(sql_helper: SqlHelper) -> Self {
        Self {
            secret: "secret", // change it!
            sql_helper,
            hash_salt: SaltString::from_b64("dGhpc2lzbXlzYWx0").unwrap(),
        }
    }

    fn encrypt_password(&self, password: &str) -> String {
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &self.hash_salt);
        password_hash.unwrap().to_string()
    }
}

#[tonic::async_trait]
impl crate::user_service_server::UserService for UserService {
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let req = request.get_ref();
        info!("login request: {:?}", req);
        let password_hash = self.encrypt_password(&req.password);
        let real_hash = self.sql_helper.get_user_password(&req.user_id).await?;
        if let Some(hash) = real_hash {
            if hash == password_hash {
                let expiration = Utc::now()
                    .checked_add_signed(Duration::days(1))
                    .unwrap()
                    .timestamp();

                Ok(Response::new(LoginResponse {
                    token: encrypt(
                        self.secret,
                        &Claims {
                            sub: req.user_id.clone(),
                            exp: expiration,
                        },
                    ),
                }))
            } else {
                Err(Error::InvalidPassword.into())
            }
        } else {
            Err(Error::UserNotFound.into())
        }
    }

    async fn register(&self, request: Request<RegisterRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("register request: {:?}", req);

        let password_hash = self.encrypt_password(&req.password);
        self.sql_helper
            .insert_user(&req.user_id, &req.name, &password_hash)
            .await?;
        Ok(Response::new(()))
    }
}

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
#[derive(Debug)]
pub struct ChannelService {
    sql_helper: SqlHelper,

    svr_manager: Arc<RwLock<ServerManager>>,
}

impl ChannelService {
    pub fn new(sql_helper: SqlHelper) -> Self {
        Self {
            sql_helper,
            svr_manager: Arc::new(RwLock::new(ServerManager::new())),
        }
    }
}

#[tonic::async_trait]
impl crate::channel_service_server::ChannelService for ChannelService {
    /// list channels by request id
    /// if id is empty, return all channels
    /// if id is not empty, return channels by id
    async fn list(&self, request: Request<Channel>) -> Result<Response<ListResponse>, Status> {
        let channel_id = request.get_ref().id;
        info!("list channel request: {:?}", channel_id);
        let channels = self.sql_helper.get_channels(&channel_id).await?;
        Ok(Response::new(ListResponse { channels }))
    }

    /// create channel, generate serial number as id, and set owner
    async fn create(&self, request: Request<Channel>) -> Result<Response<Channel>, Status> {
        let channel = request.get_ref().clone();
        let user_id = request.metadata().get("token").unwrap().to_str().unwrap();
        info!(
            "create channel request: {:?} by user: {:?}",
            channel, user_id
        );

        channel.validate()?;
        let id = self.sql_helper.insert_channel(&channel, user_id).await?;
        self.svr_manager.write().await.add_channel(&id);
        Ok(Response::new(Channel { id, ..channel }))
    }

    /// delete channel by id
    /// check channel owner is or not the user
    /// if not, return error
    async fn delete(&self, request: Request<Channel>) -> Result<Response<()>, Status> {
        let channel = request.get_ref();
        let user_id = request.metadata().get("token").unwrap().to_str().unwrap();
        let owner_id = self.sql_helper.get_channel_owner(&channel.id).await?;
        if let Some(owner_id) = owner_id {
            if user_id == owner_id {
                info!("delete channel request: {:?}", channel);
                self.sql_helper.delete_channel(&channel.id).await?;
                self.svr_manager.write().await.delete_channel(&channel.id);
                Ok(Response::new(()))
            } else {
                Err(Error::PermissionDenied.into())
            }
        } else {
            Err(Error::ChannelNotFound.into())
        }
    }

    /// listen to some channel
    /// return addr of the channel's server
    /// if channel not found, return error
    /// don't need to check channel's logic, it will be checked on specific server.
    async fn listen(&self, request: Request<Channel>) -> Result<Response<ListenResponse>, Status> {
        info!("listen channel request: {:?}", request);
        let channel = request.get_ref();
        let mgr = self.svr_manager.read().await;
        let addr = mgr.get_server(&channel.id)?;
        Ok(Response::new(ListenResponse {
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
        let server_addr = request
            .metadata()
            .get("token")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let mgr = self.svr_manager.clone();

        tokio::spawn(async move {
            info!("add server: {}", server_addr);
            // if use let mgr = mgr.write().await, we will drop mgr after this line.
            // to avoid of dead lock
            mgr.write().await.add_server(&server_addr);
            // change channel's belonging server
            let mut stream = request.into_inner();
            while let Ok(report) = stream.message().await {
                if let Some(_report) = report {
                    // todo: handle report
                    info!("report: {:?} from: {}", _report, &server_addr);
                }
            }

            mgr.write().await.delete_server(&server_addr);
        });

        Ok(Response::new(()))
    }
}

// user and manager services are on manager_server
pub async fn start_manager_server(
    sql_helper: SqlHelper,
    config: &ServerConfig,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
    let interceptor = AuthTokenInterceptor::default();
    let user_svc = UserService::new(sql_helper.clone());
    let channel_svc = ChannelService::new(sql_helper);

    let addr: std::net::SocketAddr = config.url().parse()?;
    info!("start manager server at {}", addr);

    let server = tonic::transport::Server::builder()
        .add_service(UserServiceServer::new(user_svc))
        .add_service(ChannelServiceServer::with_interceptor(
            channel_svc,
            interceptor,
        ))
        .serve(addr);

    Ok(tokio::spawn(async move {
        if let Err(e) = server.await {
            eprintln!("Server error: {}", e);
        }
    }))
}

/// A cache for servers:
/// like the relation of server and channel.
///
/// For now, range all the channels when some server status changed.
#[derive(Debug)]
struct ServerManager {
    channel_to_server: HashMap<i32, Option<String>>,
    hash: ConsistentHash,
}

impl ServerManager {
    pub fn new() -> Self {
        Self {
            channel_to_server: HashMap::new(),
            hash: ConsistentHash::new(),
        }
    }

    // todo: use a performance method in avoid of range all the channels, and time cost will be O(N / M)
    // N is the number of channels, M is the number of servers.
    fn realloc(&mut self) {
        for (channel_id, server) in self.channel_to_server.iter_mut() {
            *server = self.hash.get_server(&channel_id.to_string()).cloned()
        }
        info!(
            "server manager reallocated, new relation: {:?}",
            self.channel_to_server
        );
    }

    /// Add a server to the cache.
    ///
    /// All channels will be reallocated to some server, time cost: O(N), N is the number of channels.
    pub fn add_server(&mut self, server: &str) {
        self.hash.add_server(server);
        self.realloc();
    }

    /// Delete a server from the cache.
    ///
    /// All channels will be reallocated to some server, time cost: O(N), N is the number of channels.
    pub fn delete_server(&mut self, server: &str) {
        self.hash.remove_server(server);
        self.realloc();
    }

    /// Add a channel to the cache.
    ///
    /// time cost: O(1).
    pub fn add_channel(&mut self, channel_id: &i32) {
        let server = self.hash.get_server(&channel_id.to_string());
        self.channel_to_server.insert(*channel_id, server.cloned());
    }

    /// Delete a channel from the cache.
    ///
    /// time cost: O(1).
    pub fn delete_channel(&mut self, channel_id: &i32) {
        self.channel_to_server.remove(channel_id);
    }

    /// Get the server that a channel is assigned to.
    ///
    /// time cost: O(1).
    ///
    /// there are two failed cases:
    ///     1. channel not found
    ///     2. server not found
    pub fn get_server(&self, channel_id: &i32) -> crate::Result<String> {
        if let Some(server) = self.channel_to_server.get(channel_id) {
            if let Some(server) = server {
                Ok(server.clone())
            } else {
                Err(Error::ServerNotFound)
            }
        } else {
            Err(Error::ChannelNotFound)
        }
    }
}
