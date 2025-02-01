use std::collections::HashMap;

use log::info;

use crate::{error::Error, hash::ConsistentHash};

/// A cache for servers:
/// like the relation of server and channel.
///
/// For now, range all the channels when some server status changed.
#[derive(Debug)]
pub struct ServerManager {
    channel_to_server: HashMap<i32, Option<String>>,
    hash: ConsistentHash,
}

impl ServerManager {
    pub fn new() -> Self {
        Self {
            channel_to_server: HashMap::new(),
            hash: ConsistentHash::new(),
        }
    }

    // todo: use a performance method in avoid of range all the channels, and time cost will be O(N / M)
    // N is the number of channels, M is the number of servers.
    fn realloc(&mut self) {
        for (channel_id, server) in self.channel_to_server.iter_mut() {
            *server = self.hash.get_server(&channel_id.to_string()).cloned()
        }
        info!(
            "server manager reallocated, new relation: {:?}",
            self.channel_to_server
        );
    }

    /// Add a server to the cache.
    ///
    /// All channels will be reallocated to some server, time cost: O(N), N is the number of channels.
    pub fn add_server(&mut self, server: &str) {
        self.hash.add_server(server);
        self.realloc();
    }

    /// Delete a server from the cache.
    ///
    /// All channels will be reallocated to some server, time cost: O(N), N is the number of channels.
    pub fn delete_server(&mut self, server: &str) {
        self.hash.remove_server(server);
        self.realloc();
    }

    /// Add a channel to the cache.
    ///
    /// time cost: O(1).
    pub fn add_channel(&mut self, channel_id: &i32) {
        let server = self.hash.get_server(&channel_id.to_string());
        self.channel_to_server.insert(*channel_id, server.cloned());
    }

    /// Delete a channel from the cache.
    ///
    /// time cost: O(1).
    pub fn delete_channel(&mut self, channel_id: &i32) {
        self.channel_to_server.remove(channel_id);
    }

    /// Get the server that a channel is assigned to.
    ///
    /// time cost: O(1).
    ///
    /// there are two failed cases:
    ///     1. channel not found
    ///     2. server not found
    pub fn get_server(&self, channel_id: &i32) -> crate::Result<String> {
        if let Some(server) = self.channel_to_server.get(channel_id) {
            if let Some(server) = server {
                Ok(server.clone())
            } else {
                Err(Error::ServerNotFound)
            }
        } else {
            Err(Error::ChannelNotFound)
        }
    }
}
