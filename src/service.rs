use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use async_stream::stream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tonic::codegen::tokio_stream;
use tonic::codegen::tokio_stream::StreamExt;
use tonic::codegen::tokio_stream::wrappers::UnboundedReceiverStream;
use crate::core::EchoCore;
use echo::service_server::{Service, ServiceServer};
use echo::{GetUsersRequest, GetUsersResponse, LoginRequest, LoginResponse, QuitRequest, QuitResponse};
use log::info;
use crate::service::echo::{ChatRequest, ChatResponse, ListenRequest, ListenResponse};

pub mod echo {
    tonic::include_proto!("echo");
}

#[derive(Debug, Default)]
pub struct EchoService {
    core: Arc<EchoCore>,
}

impl EchoService {
    pub fn new() -> Self {
        Self {
            core: Arc::from(EchoCore::new()),
        }
    }
}
#[tonic::async_trait]
impl Service for EchoService {
    async fn get_users(
        &self,
        _request: Request<GetUsersRequest>,
    ) -> Result<Response<GetUsersResponse>, Status> {
        let users = self
            .core
            .users()
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(GetUsersResponse { users }))
    }

    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let user = request.into_inner().user.unwrap();
        let token = self
            .core
            .login(user.id, user.name)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(LoginResponse { token }))
    }

    async fn quit(&self, request: Request<QuitRequest>) -> Result<Response<QuitResponse>, Status> {
        let token = request.into_inner().token;
        self.core.quit(token).map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(QuitResponse {}))
    }


    async fn chat(&self, request: Request<Streaming<ChatRequest>>) -> Result<Response<ChatResponse>, Status> {
        let mut stream_req = request.into_inner();
        let core = self.core.clone();
        tokio::spawn(async move {
            while let Some(req) = stream_req.message().await.unwrap() {
                core.chat(req.token, req.message.unwrap()).unwrap();
            }
        });
        Ok(Response::new(ChatResponse {}))
    }

    type ListenStream = Pin<Box<dyn tokio_stream::Stream<
        Item=std::result::Result<ListenResponse, Status>,
    >
        + Send
        + Unpin,
    >>;
    async fn listen(&self, request: Request<ListenRequest>) -> Result<Response<Self::ListenStream>, Status> {
        let req = request.into_inner();
        info!("listen : {:?}", req);

        let mut sub = self.core.join(req.token, req.channel).map_err(|e| Status::internal(e.to_string()))?;
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok(msg) = sub.recv().await {
                let rsp = ListenResponse {
                    message: Some(msg)
                };
                tx.send(Ok(rsp)).unwrap()
            }
        });

        let stream = UnboundedReceiverStream::new(rx);
        Ok(Response::new(Pin::new(Box::new(stream))))
    }
}
