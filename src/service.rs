use crate::core::{Auth, EchoCore, LoginInfo};
use crate::{ChatRequest, ChatResponse, ListenRequest, ListenResponse};
use crate::{
    GetUsersRequest, GetUsersResponse, LoginRequest, LoginResponse, QuitRequest, QuitResponse,
};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::codegen::tokio_stream;
use tonic::codegen::tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use log::info;

macro_rules! parse_metadata_to_u32 {
    ($request:expr, $key:expr) => {{
        $request.metadata().get($key).and_then(|value| {
            value
                .to_str()
                .ok()
                .and_then(|str_val| str_val.parse::<u32>().ok())
        })
    }};
}

#[derive(Debug)]
pub struct EchoService {
    core: Arc<EchoCore>,
}

impl EchoService {
    pub fn new(core: Arc<EchoCore>) -> Self {
        Self { core }
    }
}
#[tonic::async_trait]
impl crate::service_server::Service for EchoService {
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

    async fn quit(&self, request: Request<QuitRequest>) -> Result<Response<QuitResponse>, Status> {
        let user_id = parse_metadata_to_u32!(request, "user_id").unwrap();
        self.core
            .quit(user_id)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(QuitResponse {}))
    }

    async fn chat(
        &self,
        request: Request<Streaming<ChatRequest>>,
    ) -> Result<Response<ChatResponse>, Status> {
        let user_id = parse_metadata_to_u32!(request, "user_id").unwrap();
        let mut stream_req = request.into_inner();
        let core = self.core.clone();
        tokio::spawn(async move {
            while let Some(req) = stream_req.message().await.unwrap() {
                if let Some(message) = req.message {
                    // for safe logic
                    if message.user_id == user_id {
                        core.chat(user_id, message).unwrap();
                    }
                }
            }
        });
        Ok(Response::new(ChatResponse {}))
    }

    type ListenStream = Pin<
        Box<
            dyn tokio_stream::Stream<Item = std::result::Result<ListenResponse, Status>>
                + Send
                + Unpin,
        >,
    >;
    async fn listen(
        &self,
        request: Request<ListenRequest>,
    ) -> Result<Response<Self::ListenStream>, Status> {
        let user_id = parse_metadata_to_u32!(request, "user_id").unwrap();
        let channel_id = request.into_inner().channel;

        info!("listen : user={:?}, channel={:?}", user_id, channel_id);

        let mut sub = self
            .core
            .listen(user_id, channel_id)
            .map_err(|e| Status::internal(e.to_string()))?;
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok(msg) = sub.recv().await {
                let rsp = ListenResponse { message: Some(msg) };
                tx.send(Ok(rsp)).unwrap()
            }
        });

        let stream = UnboundedReceiverStream::new(rx);
        Ok(Response::new(Pin::new(Box::new(stream))))
    }
}

#[derive(Debug)]
pub struct UserService {
    core: Arc<EchoCore>,
}

impl UserService {
    pub fn new(core: Arc<EchoCore>) -> Self {
        Self { core }
    }
}
#[tonic::async_trait]
impl crate::user_service_server::UserService for UserService {
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let user = request.into_inner().user.unwrap();
        let token = self
            .core
            .login(LoginInfo { user_id: user.id })
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(LoginResponse { token }))
    }
}
