use crate::auth::interceptor::{Claims, MyInterceptor};
use crate::auth::validator::Validator;
use crate::channel_service_server::ChannelServiceServer;
use crate::config::ServerConfig;
use crate::db::SqlHelper;
use crate::pb::{LoginRequest, LoginResponse, RegisterRequest};
use crate::user_service_server::UserServiceServer;
use crate::{error::*, ListResponse};
use crate::{Channel, ListenResponse, ReportRequest};
use argon2::Argon2;
use chrono::{Duration, Utc};
use log::info;
use password_hash::{PasswordHasher, SaltString};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
#[derive(Debug)]
pub struct UserService {
    sql_helper: SqlHelper,
    hash_salt: SaltString,
}

impl UserService {
    pub fn new(sql_helper: SqlHelper) -> Self {
        Self {
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
                let interceptor = MyInterceptor::default();
                let expiration = Utc::now()
                    .checked_add_signed(Duration::days(1))
                    .unwrap()
                    .timestamp() as usize;

                Ok(Response::new(LoginResponse {
                    token: interceptor.encrypt(&Claims {
                        sub: req.user_id.clone(),
                        exp: expiration,
                    }),
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

#[derive(Debug)]
pub struct ChannelService {
    sql_helper: SqlHelper,
}

impl ChannelService {
    pub fn new(sql_helper: SqlHelper) -> Self {
        Self { sql_helper }
    }
}

#[tonic::async_trait]
impl crate::channel_service_server::ChannelService for ChannelService {
    async fn list(&self, request: Request<Channel>) -> Result<Response<ListResponse>, Status> {
        let channel_id = request.get_ref().id;
        info!("list channel request: {:?}", channel_id);
        let channels = self.sql_helper.get_channels(&channel_id).await?;
        Ok(Response::new(ListResponse { channels }))
    }

    async fn create(&self, request: Request<Channel>) -> Result<Response<Channel>, Status> {
        let channel = request.get_ref().clone();
        let user_id = request.metadata().get("user_id").unwrap().to_str().unwrap();
        info!(
            "create channel request: {:?} by user: {:?}",
            channel, user_id
        );

        channel.validate()?;
        let id = self.sql_helper.insert_channel(&channel, user_id).await?;
        Ok(Response::new(Channel { id, ..channel }))
    }

    async fn delete(&self, request: Request<Channel>) -> Result<Response<()>, Status> {
        let channel = request.get_ref();
        let user_id = request.metadata().get("user_id").unwrap().to_str().unwrap();
        let owner_id = self.sql_helper.get_channel_owner(&channel.id).await?;
        if let Some(owner_id) = owner_id {
            if user_id == owner_id {
                info!("delete channel request: {:?}", channel);
                self.sql_helper.delete_channel(&channel.id).await?;
                Ok(Response::new(()))
            } else {
                Err(Error::PermissionDenied.into())
            }
        } else {
            Err(Error::ChannelNotFound.into())
        }
    }

    async fn listen(&self, _request: Request<Channel>) -> Result<Response<ListenResponse>, Status> {
        todo!()
    }

    async fn report(&self, _request: Request<ReportRequest>) -> Result<Response<()>, Status> {
        todo!()
    }
}

pub async fn start_manager_server(
    sql_helper: SqlHelper,
    config: &ServerConfig,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
    let interceptor = MyInterceptor::new();
    let user_svc = UserService::new(sql_helper.clone());
    let channel_svc = ChannelService::new(sql_helper);

    let addr: std::net::SocketAddr = config.url().parse()?;
    info!("start manager server at {}", addr);

    let server = Server::builder()
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
