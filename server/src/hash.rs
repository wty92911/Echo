use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// A consistent hashing implementation that distributes keys across a set of servers.
///
/// Consistent hashing is a technique used to minimize the number of keys that need to be
/// remapped when servers are added or removed. This implementation uses virtual nodes
/// to ensure a more even distribution of keys across servers.
///
/// # Examples
///
/// ```
/// use echo_server::hash::ConsistentHash;
///
/// let mut ch = ConsistentHash::new();
/// ch.add_server("server1");
/// ch.add_server("server2");
///
/// let key = "my_key";
/// let server = ch.get_server(key).unwrap();
/// println!("Key '{}' is assigned to server '{}'", key, server);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsistentHash {
    ring: BTreeMap<u64, String>, // The hash ring: maps hash values to server addresses
    virtual_nodes: usize,        // Number of virtual nodes per server
}

impl Default for ConsistentHash {
    fn default() -> Self {
        Self::new()
    }
}
impl ConsistentHash {
    /// Creates a new `ConsistentHash` instance with a default of 10 virtual nodes per server.
    ///
    /// # Examples
    ///
    /// ```
    /// use echo_server::hash::ConsistentHash;
    ///
    /// let ch = ConsistentHash::new();
    /// ```
    pub fn new() -> Self {
        Self {
            ring: BTreeMap::new(),
            virtual_nodes: 10,
        }
    }

    /// Adds a new server to the hash ring.
    ///
    /// Each server is represented by multiple virtual nodes to ensure a more even
    /// distribution of keys.
    ///
    /// # Arguments
    ///
    /// * `server` - The address of the server to add (e.g., "127.0.0.1:8080").
    ///
    /// # Examples
    ///
    /// ```
    /// use echo_server::hash::ConsistentHash;
    ///
    /// let mut ch = ConsistentHash::new();
    /// ch.add_server("server1");
    /// ```
    pub fn add_server(&mut self, server: &str) {
        for i in 0..self.virtual_nodes {
            let virtual_node = format!("{}#{}", server, i);
            let hash = self.hash(&virtual_node);
            self.ring.insert(hash, server.to_string());
        }
    }

    /// Removes a server from the hash ring.
    ///
    /// All virtual nodes associated with the server are removed, and any keys
    /// previously assigned to the server will be reassigned to other servers.
    ///
    /// # Arguments
    ///
    /// * `server` - The address of the server to remove.
    ///
    /// # Examples
    ///
    /// ```
    /// use echo_server::hash::ConsistentHash;
    ///
    /// let mut ch = ConsistentHash::new();
    /// ch.add_server("server1");
    /// ch.remove_server("server1");
    /// ```
    pub fn remove_server(&mut self, server: &str) {
        for i in 0..self.virtual_nodes {
            let virtual_node = format!("{}#{}", server, i);
            let hash = self.hash(&virtual_node);
            self.ring.remove(&hash);
        }
    }

    /// Gets the server responsible for a particular key.
    ///
    /// The key is hashed and mapped to the closest server in the hash ring.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up (e.g., "my_key").
    ///
    /// # Returns
    ///
    /// An `Option<&String>` containing the address of the server responsible for the key.
    /// If no servers are available, `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use echo_server::hash::ConsistentHash;
    ///
    /// let mut ch = ConsistentHash::new();
    /// ch.add_server("server1");
    ///
    /// let key = "my_key";
    /// if let Some(server) = ch.get_server(key) {
    ///     println!("Key '{}' is assigned to server '{}'", key, server);
    /// } else {
    ///     println!("No server available for key '{}'", key);
    /// }
    /// ```
    pub fn get_server(&self, key: &str) -> Option<&String> {
        let hash = self.hash(&key);
        self.ring
            .range(hash..)
            .next()
            .map_or_else(|| self.ring.values().next(), |(_, server)| Some(server))
    }

    /// Computes the hash value for a given key.
    ///
    /// This method uses Rust's `DefaultHasher` to compute a 64-bit hash value.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to hash.
    ///
    /// # Returns
    ///
    /// A `u64` hash value.
    fn hash<T: Hash>(&self, key: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_server() {
        let mut ch = ConsistentHash::new();

        // Add servers
        ch.add_server("server1");
        ch.add_server("server2");

        // Verify that the servers are added with virtual nodes
        assert_eq!(ch.ring.len(), 20); // 2 servers * 10 virtual nodes each
    }

    #[test]
    fn test_remove_server() {
        let mut ch = ConsistentHash::new();

        // Add servers
        ch.add_server("server1");
        ch.add_server("server2");

        // Remove a server
        ch.remove_server("server1");

        // Verify that the server and its virtual nodes are removed
        assert_eq!(ch.ring.len(), 10); // Only server2 remains (10 virtual nodes)
    }

    #[test]
    fn test_get_server() {
        let mut ch = ConsistentHash::new();

        // Add servers
        ch.add_server("server1");
        ch.add_server("server2");

        // Test key assignment
        let key1 = "key1";
        let key2 = "key2";
        let key3 = "key3";

        let server1 = ch.get_server(key1).unwrap();
        let server2 = ch.get_server(key2).unwrap();
        let server3 = ch.get_server(key3).unwrap();

        // Verify that keys are assigned to one of the servers
        assert!(server1 == "server1" || server1 == "server2");
        assert!(server2 == "server1" || server2 == "server2");
        assert!(server3 == "server1" || server3 == "server2");
    }

    #[test]
    fn test_get_server_after_removal() {
        let mut ch = ConsistentHash::new();

        // Add servers
        ch.add_server("server1");
        ch.add_server("server2");

        // Assign a key to a server
        let key = "key1";
        let initial_server = ch.get_server(key).unwrap().clone();

        // Remove the server that the key was assigned to
        if initial_server == "server1" {
            ch.remove_server("server1");
        } else {
            ch.remove_server("server2");
        }

        // Reassign the key
        let new_server = ch.get_server(key).unwrap().clone();

        // Verify that the key is reassigned to the remaining server
        assert_ne!(initial_server, new_server);
        assert!(new_server == "server1" || new_server == "server2");
    }

    #[test]
    fn test_load_balancing() {
        let mut ch = ConsistentHash::new();

        // Add multiple servers
        ch.add_server("server1");
        ch.add_server("server2");
        ch.add_server("server3");

        // Track how many keys are assigned to each server
        let mut server_counts = std::collections::HashMap::new();
        for i in 0..1000 {
            let key = format!("key{}", i);
            let server = ch.get_server(&key).unwrap();
            *server_counts.entry(server.clone()).or_insert(0) += 1;
        }

        // Verify that the keys are distributed evenly across servers
        let min_count = *server_counts.values().min().unwrap();
        let max_count = *server_counts.values().max().unwrap();
        let diff = max_count - min_count;

        // Allow some variation due to hashing, but ensure it's within a reasonable range
        assert!(
            diff <= 100,
            "Load is not balanced: min={}, max={}",
            min_count,
            max_count
        );
    }

    #[test]
    fn test_empty_ring() {
        let ch = ConsistentHash::new();

        // Try to get a server when no servers are added
        let key = "key1";
        let server = ch.get_server(key);

        // Verify that no server is returned
        assert!(server.is_none());
    }
}
