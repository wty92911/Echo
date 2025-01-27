use chrono;
use std::str::FromStr;
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    service::Interceptor,
    transport::{Channel, Endpoint},
    Request, Response, Status,
};

use crate::{
    auth::interceptor::{Claims, ClientTokenInterceptor},
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
        let interceptor = ClientTokenInterceptor::new(
            self.secret.clone(),
            Claims {
                sub: chat_server_addr,
                exp: chrono::Utc::now().timestamp() + 60 * 60 * 24 * 30,
            },
        );
        let stream = ReceiverStream::new(rx);
        let req = Request::new(stream);
        let req = interceptor.call(req)?;
        self.inner.report(req).await
    }
}

pub fn create_interceptor(token: String) -> impl Interceptor {
    move |mut req: Request<()>| -> Result<Request<()>, Status> {
        req.metadata_mut().insert(
            "authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );
        Ok(req)
    }
}
