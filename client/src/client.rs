use crate::audio::{Microphone, Speaker};
use crate::config::UserConfig;
use crate::utils::{Buffer, RING_BUFFER_SIZE};
use crate::utils::{FromBytes, ToBytes};
use abi::error::Error;
use abi::pb::{
    channel_service_client::ChannelServiceClient, chat_service_client::ChatServiceClient,
    user_service_client::UserServiceClient, Channel, RegisterRequest,
};
use abi::pb::{LoginRequest, Message};
use abi::traits::WithToken;
use abi::Result;
use cpal::traits::StreamTrait;
use log::error;
use ringbuffer::{AllocRingBuffer, RingBuffer};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Receiver;
use tonic::transport::Endpoint;
use tonic::Request;
/// Audio Client
pub struct Client {
    // User ID, currently logged in.
    user_id: Option<String>,

    // User Config.
    config: Arc<UserConfig>, // todo: support remote config

    // Token of logging in.
    token: Option<String>,

    // User Service Client.
    // For now, addr is same as manager addr.
    user_client: UserServiceClient<tonic::transport::Channel>,

    // Channel Manager Client. Manager addr must be provided at **new()**.
    mgr_client: ChannelServiceClient<tonic::transport::Channel>,

    // hold the audio data buffer from the listening channel.
    buffer: Arc<Mutex<Buffer<f32>>>,

    // hold the audio data of own microphone.
    buf: Arc<Mutex<AllocRingBuffer<f32>>>,

    speaker: Speaker,

    microphone: Microphone,
}

/// Impl Client Methods for User Service
#[allow(dead_code)]
impl Client {
    pub async fn new(mgr_addr: String) -> Result<Client> {
        let conn = Endpoint::from_str(&mgr_addr)?.connect().await?;
        Ok(Client {
            user_id: None,
            config: Arc::new(UserConfig::default()),
            token: None,
            mgr_client: ChannelServiceClient::new(conn.clone()),
            user_client: UserServiceClient::new(conn),

            buffer: Arc::new(Mutex::new(Buffer::new())),
            buf: Arc::new(Mutex::new(AllocRingBuffer::new(RING_BUFFER_SIZE))),

            speaker: Speaker::default(),
            microphone: Microphone::default(),
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
        self.user_id = Some(user_id.clone());
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

    /// Delete a channel.
    pub async fn delete_channel(&mut self, id: i32) -> Result<()> {
        let token = check_token(&self.token)?;
        let req = Request::new(Channel {
            id,
            ..Default::default()
        })
        .with(token);
        self.mgr_client.delete(req).await?;
        Ok(())
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

    /// Connect to a channel.
    async fn connect(&mut self, id: i32, input: Receiver<Message>) -> Result<Receiver<Message>> {
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

    pub async fn communicate(
        &mut self,
        id: i32,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let mut rx = self.connect(id, rx).await?;
        let speak_stream = self.speaker.play(self.buffer.clone(), self.config.clone());
        speak_stream.play().unwrap();

        let buffer = Arc::clone(&self.buffer);
        let user_id = self.user_id.clone().unwrap();
        let output = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if msg.user_id != user_id {
                    // todo: add audio_data field or define a serialization method
                    buffer
                        .lock()
                        .unwrap()
                        .extend(msg.user_id, FromBytes::from_bytes(msg.data));
                }
            }
        });

        let input_stream = self
            .microphone
            .record(Arc::clone(&self.buf), self.config.clone());
        input_stream.play().unwrap();

        let buf = Arc::clone(&self.buf);
        let user_id = self.user_id.clone().unwrap();
        let input = tokio::spawn(async move {
            loop {
                let data = buf.lock().unwrap().drain().collect::<Vec<f32>>().to_bytes();
                if let Err(e) = tx
                    .send(Message {
                        user_id: user_id.clone(),
                        timestamp: chrono::Utc::now().timestamp(),
                        data,
                    })
                    .await
                {
                    error!("error sending message: {}", e);
                    break;
                }
            }
        });

        drop(speak_stream);
        drop(input_stream);
        // any of these can be cancelled
        tokio::select! {
            _ = shutdown.recv() => {},
            _ = input => {},
            _ = output => {},
        }
        Ok(())
    }
}

fn check_token(token: &Option<String>) -> Result<&String> {
    if token.is_none() {
        Err(Error::TokenNotFound)
    } else {
        Ok(token.as_ref().unwrap())
    }
}
