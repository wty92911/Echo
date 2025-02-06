use abi::pb::LoginRequest;
use abi::pb::{
    channel_service_client::ChannelServiceClient, chat_service_client::ChatServiceClient,
    user_service_client::UserServiceClient, Channel, RegisterRequest,
};
use abi::Result;
use std::str::FromStr;
use tonic::transport::Endpoint;

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

    /// Chat Server Addr.
    chat_addr: Option<String>,

    /// Chat Token of trying to listen to some channel.
    chat_token: Option<String>,

    /// Chat Client to connect to some  channel on specific chat server of chat_addr.
    chat_client: Option<ChatServiceClient<tonic::transport::Channel>>,
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
            chat_addr: None,
            chat_token: None,
            chat_client: None,
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
    pub async fn create_channel(&mut self, name: String) -> Result<()> {
        let req = Channel {
            name,
            ..Default::default() // not need token
        };
        self.mgr_client.create(req).await?;
        Ok(())
    }
}
