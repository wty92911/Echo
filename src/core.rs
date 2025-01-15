use crate::Message;
use crate::Result;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::SendError;
pub trait Auth {
    fn verify(&self, token: &str) -> Result<u32>;
    fn login(&self, info: LoginInfo) -> Result<String>;
}

/// Channel for broadcasting message to users, recording users which are online.
#[derive(Debug)]
pub struct Channel {
    broadcast: broadcast::Sender<Message>,
    user_set: HashSet<u32>,
}

impl Default for Channel {
    fn default() -> Self {
        Self::new()
    }
}

impl Channel {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self {
            broadcast: sender,
            user_set: HashSet::new(),
        }
    }

    pub fn send(&self, msg: Message) -> std::result::Result<usize, SendError<Message>> {
        self.broadcast.send(msg)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.broadcast.subscribe()
    }

    pub fn is_empty(&self) -> bool {
        self.user_set.is_empty()
    }
    pub fn add_user(&mut self, user_id: u32) {
        self.user_set.insert(user_id);
    }
    pub fn remove_user(&mut self, user_id: u32) {
        self.user_set.remove(&user_id);
    }
}
/// Echo's core logic. MultiThread safe.
#[derive(Clone, Debug, Default)]
pub struct EchoCore {
    user_info_map: Arc<RwLock<HashMap<u32, User>>>,
    channel_map: Arc<RwLock<HashMap<u32, Channel>>>,
    secret: String,
}

impl EchoCore {
    pub fn new() -> Self {
        Self {
            user_info_map: Arc::new(RwLock::new(HashMap::new())),
            channel_map: Arc::new(RwLock::new(HashMap::new())),
            secret: "secret".to_string(),
        }
    }

    pub fn channel_users(&self) -> Result<HashMap<u32, Vec<User>>> {
        let channel_map = self.channel_map.read().unwrap();
        let user_info = self.user_info_map.read().unwrap();
        Ok(channel_map
            .iter()
            .map(|(channel_id, content)| {
                (
                    *channel_id,
                    content
                        .user_set
                        .iter()
                        .map(|user_id| user_info.get(user_id).unwrap().clone())
                        .collect(),
                )
            })
            .collect())
    }

    pub fn users(&self) -> Result<HashMap<u32, String>> {
        let user_info_map = self.user_info_map.read().unwrap();
        Ok(user_info_map
            .iter()
            .map(|(_, user)| (user.id, user.name.clone()))
            .collect())
    }

    pub fn listen(&self, user_id: u32, channel_id: u32) -> Result<broadcast::Receiver<Message>> {
        let mut channel_map = self.channel_map.write().unwrap();
        let mut user_info = self.user_info_map.write().unwrap();

        let user = user_info.get_mut(&user_id).unwrap();
        if let Some(old_channel_id) = user.channel {
            channel_map
                .get_mut(&old_channel_id)
                .unwrap()
                .remove_user(user_id);
        };

        user.channel = Some(channel_id);

        channel_map.entry(channel_id).or_default();

        let channel = channel_map.get_mut(&channel_id).unwrap();

        channel.add_user(channel_id);
        Ok(channel.subscribe())
    }

    pub fn chat(&self, user_id: u32, message: Message) -> Result<()> {
        let user_info_map = self.user_info_map.read().unwrap();
        let user = user_info_map.get(&user_id).unwrap();
        let channel_id = user.channel.ok_or("no channel")?;

        let channel_map = self.channel_map.read().unwrap();
        channel_map
            .get(&channel_id)
            .ok_or("no channel broadcast")?
            .send(message)?;
        Ok(())
    }

    pub fn quit(&self, user_id: u32) -> Result<()> {
        let mut user_info_map = self.user_info_map.write().unwrap();
        let mut channel_map = self.channel_map.write().unwrap();
        let user = user_info_map.get(&user_id).unwrap().clone();

        // Remove the user from the channel
        if let Some(channel_id) = user.channel {
            let channel = channel_map.get_mut(&channel_id).unwrap();
            channel.remove_user(user_id);
            // Remove the channel if no user in it
            if channel.is_empty() {
                channel_map.remove(&channel_id);
            }
        }

        user_info_map.remove(&user.id);
        Ok(())
    }
    fn generate_token(&self, user_id: u32) -> String {
        encode(
            &Header::default(),
            &user_id,
            &EncodingKey::from_secret(self.secret.as_ref()),
        )
        .unwrap()
    }
}

impl Auth for EchoCore {
    fn verify(&self, token: &str) -> Result<u32> {
        //todo: expire time
        decode::<u32>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &Validation::default(),
        )
        .map(|user_id| user_id.claims)
        .map_err(|e| e.into())
    }

    fn login(&self, info: LoginInfo) -> Result<String> {
        // todo: tts, jwt
        let token = self.generate_token(info.user_id);
        Ok(token)
    }
}
#[derive(Clone, Debug)]
pub struct User {
    id: u32,
    name: String,
    channel: Option<u32>,
}

pub struct LoginInfo {
    pub(crate) user_id: u32,
}
