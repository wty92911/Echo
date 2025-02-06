use crate::auth::interceptor::{encrypt, Claims};
use abi::{
    pb::{
        channel_service_client::ChannelServiceClient,
        // chat_service_client::ChatServiceClient,
        ReportRequest,
        ReportResponse,
        // ShutdownRequest,
    },
    traits::WithToken,
};
use chrono;
use log::info;
use std::str::FromStr;
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Endpoint, Request, Response, Status, Streaming};

pub struct ChannelClient {
    inner: ChannelServiceClient<tonic::transport::Channel>,
    secret: String,
}

impl ChannelClient {
    pub async fn new(addr: &str, secret: &str) -> Self {
        info!("new channel client: {}", addr);
        let conn = Endpoint::from_str(addr).unwrap().connect().await.unwrap();
        Self {
            inner: ChannelServiceClient::new(conn),
            secret: secret.to_string(),
        }
    }

    pub async fn report(
        &mut self,
        chat_server_addr: String,
        rx: Receiver<ReportRequest>,
    ) -> Result<Response<Streaming<ReportResponse>>, Status> {
        let stream = ReceiverStream::new(rx);
        let claims = Claims {
            exp: chrono::Utc::now().timestamp() + 60 * 60 * 24 * 30,
            addr: chat_server_addr,
            ..Claims::default()
        };
        let secret: &str = &self.secret;
        let token = encrypt(secret, &claims);
        self.inner.report(Request::new(stream).with(&token)).await
    }
}
