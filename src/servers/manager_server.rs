use crate::auth::{Claims, MyInterceptor};
use crate::channel_service_server::ChannelServiceServer;
use crate::config::Config;
use crate::db::SqlHelper;
use crate::error::*;
use crate::pb::{LoginRequest, LoginResponse, RegisterRequest};
use crate::user_service_server::UserServiceServer;
use crate::{Channel, ListenResponse, ReportRequest};
use argon2::Argon2;
use chrono::{Duration, Utc};
use futures::stream::{self, Stream};
use log::info;
use password_hash::{PasswordHasher, SaltString};
use std::pin::Pin;
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
        let req = request.into_inner();
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
    _sql_helper: SqlHelper,
}

impl ChannelService {
    pub fn new(_sql_helper: SqlHelper) -> Self {
        Self { _sql_helper }
    }
}

#[tonic::async_trait]
impl crate::channel_service_server::ChannelService for ChannelService {
    type ListStream = Pin<Box<dyn Stream<Item = Result<Channel, Status>> + Send>>;

    async fn list(&self, _request: Request<Channel>) -> Result<Response<Self::ListStream>, Status> {
        let empty_stream = Box::pin(stream::empty());
        Ok(Response::new(empty_stream))
    }

    async fn create(&self, _request: Request<Channel>) -> Result<Response<Channel>, Status> {
        todo!()
    }

    async fn delete(&self, _request: Request<Channel>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn listen(&self, _request: Request<Channel>) -> Result<Response<ListenResponse>, Status> {
        todo!()
    }

    async fn report(&self, _request: Request<ReportRequest>) -> Result<Response<()>, Status> {
        todo!()
    }
}

pub async fn start_manager_server(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let sql_helper = SqlHelper::new(&config.db).await?;
    let interceptor = MyInterceptor::new();
    // pgPool clones share the same connection pool.
    let user_svc = UserService::new(sql_helper.clone());
    let channel_svc = ChannelService::new(sql_helper);

    let addr: std::net::SocketAddr = config.server.url().parse()?;
    info!("start manager server at {}", addr);

    Server::builder()
        .add_service(UserServiceServer::new(user_svc))
        .add_service(ChannelServiceServer::with_interceptor(
            channel_svc,
            interceptor,
        ))
        .serve(addr)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use tonic::transport::Endpoint;

    use crate::{
        channel_service_client::ChannelServiceClient, user_service_client::UserServiceClient,
    };

    use super::*;

    fn init() -> Config {
        Config::load("./config/manager.yaml").unwrap()
    }

    // from an empty database, should run migrate.
    #[tokio::test]
    #[ignore = "should be run manually"]
    async fn test_register_and_login() {
        let config = init();
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
        let token = client
            .login(LoginRequest {
                user_id: "test".to_string(),
                password: "test_password".to_string(),
            })
            .await
            .unwrap();
        println!("token: {:?}", token);

        // test multi register
        let rsp = client
            .register(RegisterRequest {
                user_id: "test".to_string(),
                password: "test_password".to_string(),
                name: "test_name".to_string(),
            })
            .await;
        println!("rsp: {:?}", rsp);
        assert!(rsp.is_err())
    }

    #[tokio::test]
    #[ignore = "should be run manually"]
    async fn test_interceptor() {
        let config = init();
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

        // check invalid/no token
        let rsp = chan_client.list(Request::new(Channel::default())).await;
        println!("before auth token, rsp: {:?}", rsp);
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
        println!("token: {:?}", token);

        let mut req = Request::new(Channel::default());
        req.metadata_mut().insert(
            "authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );
        // check valid token
        let rsp = chan_client.list(req).await;
        println!("after auth token, rsp: {:?}", rsp);
        assert!(rsp.is_ok());
    }
}
