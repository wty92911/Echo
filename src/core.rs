use crate::Result;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tonic::codegen::Bytes;

use crate::service::echo::Message;

/// Echo's core logic.
#[derive(Clone, Debug, Default)]
pub struct EchoCore {
    token_user: Arc<RwLock<HashMap<String, u32>>>,
    user_token: Arc<RwLock<HashMap<u32, String>>>,
    user_info: Arc<RwLock<HashMap<u32, User>>>,
    channel_users: Arc<RwLock<HashMap<u32, HashSet<u32>>>>,
    channel_broadcast: Arc<RwLock<HashMap<u32, broadcast::Sender<Message>>>>,
}

impl EchoCore {
    pub fn new() -> Self {
        Self {
            token_user: Arc::new(RwLock::new(HashMap::new())),
            user_token: Arc::new(RwLock::new(HashMap::new())),
            user_info: Arc::new(RwLock::new(HashMap::new())),
            channel_users: Arc::new(RwLock::new(HashMap::new())),
            channel_broadcast: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn channel_users(&self) -> Result<HashMap<u32, HashSet<String>>> {
        let channel_users = self.channel_users.read().unwrap();
        let user_info = self.user_info.read().unwrap();
        Ok(channel_users
            .iter()
            .map(|(channel_id, user_ids)| {
                (
                    *channel_id,
                    user_ids
                        .iter()
                        .map(|user_id| user_info.get(user_id).unwrap().name.clone())
                        .collect(),
                )
            })
            .collect())
    }

    pub fn users(&self) -> Result<HashMap<u32, String>> {
        let user_info = self.user_info.read().unwrap();
        Ok(user_info
            .iter()
            .map(|(user_id, user)| (user.id, user.name.clone()))
            .collect())
    }

    pub fn join(
        &self,
        token: String,
        channel_id: u32,
    ) -> Result<broadcast::Receiver<Message>> {
        let token_user = self.token_user.write().unwrap();
        let mut user_info = self.user_info.write().unwrap();
        let user_id = token_user.get(&token).ok_or("Invalid token")?;
        let user = user_info.get_mut(user_id).unwrap();
        let mut channel_users = self.channel_users.write().unwrap();
        let mut channel_broadcast = self.channel_broadcast.write().unwrap();
        match user.channel {
            Some(channel_id) => {
                channel_users.get_mut(&channel_id).unwrap().remove(&user.id);
            }
            _ => {}
        };
        user.channel = Some(channel_id);
        if !channel_users.contains_key(&channel_id) {
            channel_users.insert(channel_id, HashSet::new());
        }

        if !channel_broadcast.contains_key(&channel_id) {
            let (tx, _) = broadcast::channel(64);
            channel_broadcast.insert(channel_id, tx);
        }
        channel_users.get_mut(&channel_id).unwrap().insert(*user_id);
        let rx = channel_broadcast.get(&channel_id).unwrap().subscribe();
        Ok(rx)
    }


    pub fn chat(&self, token: String, mut message: Message) -> Result<()> {
        let token_user = self.token_user.write().unwrap();
        let user_info = self.user_info.write().unwrap();
        let user_id = token_user.get(&token).ok_or("Invalid token")?;
        let mut user = user_info.get(user_id).unwrap();
        if message.user_id != user.id {
            return Err("Invalid message".into());
        }
        let channel_broadcast = self.channel_broadcast.write().unwrap();
        let channel_id = user.channel.ok_or("no channel")?;
        let channel_broadcast = channel_broadcast.get(&channel_id).ok_or("no channel broadcast")?;
        channel_broadcast.send(message)?;
        Ok(())
    }

    pub fn quit(&self, token: String) -> Result<()> {
        let mut token_user = self.token_user.write().unwrap();
        let user_info = self.user_info.write().unwrap();
        let user_id = token_user.get(&token).ok_or("Invalid token")?.clone();

        let user = user_info.get(&user_id).unwrap();
        let mut user_token = self.user_token.write().unwrap();
        let mut channel_users = self.channel_users.write().unwrap();

        // Remove the user from the token's user list
        token_user.remove(&token);
        user_token.remove(&user.id);

        // Remove the user from the channel's user list
        let channel_users = channel_users
            .get_mut(&user.channel.unwrap())
            .ok_or("User not in channel")?;

        if channel_users.is_empty() {
            // If the channel is now empty, delete it
            channel_users.remove(&user.channel.unwrap());
            self.channel_broadcast
                .write()
                .unwrap()
                .remove(&user.channel.unwrap());
        }
        channel_users.remove(&user.id);
        Ok(())
    }

    pub fn login(&self, user_id: u32, name: String) -> Result<String> {
        let mut user_token = self.user_token.write().unwrap();
        if user_token.contains_key(&user_id) {
            return Err("User already logged in".into());
        }
        let token = crate::generate_token();
        let mut token_user = self.token_user.write().unwrap();
        user_token.insert(user_id, token.clone());
        token_user.insert(token.clone(), user_id);
        let mut user_info = self.user_info.write().unwrap();
        user_info.insert(
            user_id,
            User {
                id: user_id,
                name,
                channel: None,
            },
        );
        Ok(token)
    }
}

#[derive(Clone, Debug)]
pub struct User {
    id: u32,
    name: String,
    channel: Option<u32>,
}
