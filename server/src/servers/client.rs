use chrono;
use log::info;
use std::str::FromStr;
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Endpoint, Request, Response, Status, Streaming};

use crate::auth::interceptor::{intercept_token, Claims};
use abi::pb::{
    channel_service_client::ChannelServiceClient,
    // chat_service_client::ChatServiceClient,
    ReportRequest,
    ReportResponse,
    // ShutdownRequest,
};

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

// pub struct ChatClient {
//     inner: ChatServiceClient<tonic::transport::Channel>,
//     secret: String,
// }

// impl ChatClient {
//     pub async fn new(addr: &str, secret: &str) -> Self {
//         let conn = Endpoint::from_str(addr).unwrap().connect().await.unwrap();
//         Self {
//             inner: ChatServiceClient::new(conn),
//             secret: secret.to_string(),
//         }
//     }

//     pub async fn shutdown(
//         &mut self,
//         req: ShutdownRequest,
//         addr: String,
//     ) -> Result<Response<()>, Status> {
//         let req = intercept_token(
//             Request::new(req),
//             Claims {
//                 addr,
//                 ..Default::default()
//             },
//             &self.secret,
//         )?;
//         self.inner.shutdown(req).await
//     }
// }
