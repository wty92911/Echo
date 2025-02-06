use abi::error::Error;
use abi::pb::{
    channel_service_client::ChannelServiceClient, chat_service_client::ChatServiceClient,
    user_service_client::UserServiceClient, Channel, RegisterRequest,
};
use abi::pb::{LoginRequest, Message};
use abi::traits::WithToken;
use abi::Result;
use std::str::FromStr;
use tokio::sync::mpsc::Receiver;
use tonic::transport::Endpoint;
use tonic::Request;
/// Audio Client
#[allow(dead_code)]
struct Client {
    /// Token of logging in.
    token: Option<String>,

    /// User Service Client.
    /// For now, addr is same as manager addr.
    user_client: UserServiceClient<tonic::transport::Channel>,

    /// Channel Manager Client. Manager addr must be provided at **new()**.
    mgr_client: ChannelServiceClient<tonic::transport::Channel>,
    // /// Chat Token of trying to listen to some channel.
    // chat_token: Option<String>,

    // /// Chat Client to connect to some  channel on specific chat server of chat_addr.
    // chat_client: Option<ChatServiceClient<tonic::transport::Channel>>,
}

/// Impl Client Methods for User Service
#[allow(dead_code)]
impl Client {
    pub async fn new(mgr_addr: String) -> Result<Client> {
        let conn = Endpoint::from_str(&mgr_addr)?.connect().await?;
        Ok(Client {
            token: None,
            mgr_client: ChannelServiceClient::new(conn.clone()),
            user_client: UserServiceClient::new(conn),
            // chat_token: None,
            // chat_client: None,
        })
    }

    pub async fn register(
        &mut self,
        user_id: String,
        password: String,
        name: String,
    ) -> Result<()> {
        let req = RegisterRequest {
            user_id,
            password,
            name,
        };
        self.user_client.register(req).await?;
        Ok(())
    }

    pub async fn login(&mut self, user_id: String, password: String) -> Result<()> {
        let req = LoginRequest { user_id, password };
        let res = self.user_client.login(req).await?.into_inner();
        self.token = Some(res.token);
        Ok(())
    }
}

/// Impl Client Methods for Channel Service
#[allow(dead_code)]
impl Client {
    /// Create a channel.
    pub async fn create_channel(&mut self, name: String, limit: i32) -> Result<Channel> {
        let token = check_token(&self.token)?;
        let req = Request::new(Channel {
            name,
            limit,
            ..Default::default()
        })
        .with(token);
        let channel = self.mgr_client.create(req).await?.into_inner();
        Ok(channel)
    }

    /// Get all channels(id = 0) or specific channel(id != 0)
    pub async fn get_channels(&mut self, id: i32) -> Result<Vec<Channel>> {
        let token = check_token(&self.token)?;
        let req = Request::new(Channel {
            id,
            ..Default::default()
        })
        .with(token);
        let rsp = self.mgr_client.list(req).await?.into_inner();
        Ok(rsp.channels)
    }

    /// Listen to a channel.
    pub async fn listen(&mut self, id: i32, input: Receiver<Message>) -> Result<Receiver<Message>> {
        let token = check_token(&self.token)?;
        let req = Request::new(Channel {
            id,
            ..Default::default()
        })
        .with(token);
        let rsp = self.mgr_client.listen(req).await?.into_inner();
        if let Some(server) = rsp.server {
            let mut client = ChatServiceClient::connect(server.addr).await?;
            let send_stream = tokio_stream::wrappers::ReceiverStream::new(input);
            let rsp = client.conn(Request::new(send_stream).with(token)).await?;
            let mut recv_stream = rsp.into_inner();
            let (tx, rx) = tokio::sync::mpsc::channel(32);
            tokio::spawn(async move {
                while let Ok(msg) = recv_stream.message().await {
                    if let Some(msg) = msg {
                        tx.send(msg).await.unwrap();
                    } else {
                        break;
                    }
                }
            });
            Ok(rx)
        } else {
            Err(Error::ServerNotFound)
        }
    }
}
fn check_token(token: &Option<String>) -> Result<&String> {
    if token.is_none() {
        Err(Error::TokenNotFound)
    } else {
        Ok(token.as_ref().unwrap())
    }
}
