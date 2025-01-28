use chrono;
use std::str::FromStr;
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    transport::{Channel, Endpoint},
    Request, Response, Status,
};

use crate::{
    auth::interceptor::{intercept_token, Claims},
    channel_service_client::ChannelServiceClient,
    ReportRequest,
};

pub struct ChannelClient {
    inner: ChannelServiceClient<Channel>,
    secret: String,
}

impl ChannelClient {
    pub async fn new(addr: &str, secret: &str) -> Self {
        println!("Connecting to {}", addr);
        let conn = Endpoint::from_str(addr).unwrap().connect().await.unwrap();
        println!("Connected");
        Self {
            inner: ChannelServiceClient::new(conn),
            secret: secret.to_string(),
        }
    }

    pub async fn report(
        &mut self,
        chat_server_addr: String,
        rx: Receiver<ReportRequest>,
    ) -> Result<Response<()>, Status> {
        let stream = ReceiverStream::new(rx);
        let mut req = Request::new(stream);
        req = intercept_token(
            req,
            Claims {
                exp: chrono::Utc::now().timestamp() + 60 * 60 * 24 * 30,
                addr: chat_server_addr,
                ..Claims::default()
            },
            &self.secret,
        )?;
        self.inner.report(req).await
    }
}
