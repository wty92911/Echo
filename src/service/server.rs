use crate::{Channel, Message};
use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status, Streaming};

#[derive(Debug, Default)]
pub struct ChatService {}

#[tonic::async_trait]
impl crate::chat_service_server::ChatService for ChatService {
    type ConnStream = Pin<Box<dyn Stream<Item = std::result::Result<Message, Status>> + Send>>;

    async fn conn(
        &self,
        _request: Request<Streaming<Message>>,
    ) -> Result<Response<Self::ConnStream>, Status> {
        todo!()
    }

    async fn add(&self, _request: Request<Channel>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn remove(&self, _request: Request<Channel>) -> Result<Response<()>, Status> {
        todo!()
    }
}
