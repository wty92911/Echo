use crate::auth::interceptor::{encrypt, Claims};
use crate::db::SqlHelper;
use abi::error::Error;
use abi::pb::{LoginRequest, LoginResponse, RegisterRequest};
use argon2::Argon2;
use chrono::{Duration, Utc};
use log::info;
use password_hash::{PasswordHasher, SaltString};
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct UserService {
    secret: String,
    sql_helper: SqlHelper,
    hash_salt: SaltString,
}

impl UserService {
    pub fn new(secret: String, sql_helper: SqlHelper) -> Self {
        Self {
            secret, // change it!
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
impl abi::pb::user_service_server::UserService for UserService {
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
                        &self.secret,
                        &Claims {
                            exp: expiration,
                            user_id: req.user_id.clone(),
                            ..Claims::default()
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
