use std::pin::Pin;

use crate::pb::{LoginRequest, LoginResponse, RegisterRequest};
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use crate::{Channel, ListenResponse, ReportRequest};

#[derive(Debug, Default)]
pub struct UserService {}

#[tonic::async_trait]
impl crate::user_service_server::UserService for UserService {
    async fn login(
        &self,
        _request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        todo!()
    }

    async fn register(&self, _request: Request<RegisterRequest>) -> Result<Response<()>, Status> {
        todo!()
    }
}

#[derive(Debug, Default)]
pub struct ChannelService {}

#[tonic::async_trait]
impl crate::channel_service_server::ChannelService for ChannelService {
    type ListStream = Pin<Box<dyn Stream<Item = Result<Channel, Status>> + Send>>;

    async fn list(&self, _request: Request<Channel>) -> Result<Response<Self::ListStream>, Status> {
        todo!()
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
