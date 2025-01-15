use crate::core::{Auth, EchoCore};
use std::sync::Arc;
use tonic::{Request, Status};

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct Claims {
    pub user_id: u32,
    pub exp: usize,
}
#[derive(Clone)]
pub struct Interceptor {
    core: Arc<EchoCore>,
}

impl Interceptor {
    pub fn new(core: Arc<EchoCore>) -> Self {
        Self { core }
    }
}
impl tonic::service::Interceptor for Interceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let authorization = request
            .metadata()
            .get("authorization")
            .ok_or_else(|| tonic::Status::unauthenticated("No auth token provided"))?
            .to_str()
            .map_err(|e| tonic::Status::unauthenticated(e.to_string()))?;
        let token = &authorization["Bearer ".len()..];
        let claims = self
            .core
            .verify(token)
            .map_err(|e| tonic::Status::unauthenticated(e.to_string()))?;
        request
            .metadata_mut()
            .insert("user_id", claims.user_id.to_string().parse().unwrap());
        Ok(request)
    }
}
