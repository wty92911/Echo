use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

use abi::error::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub db: DbConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
    #[serde(default = "default_pool_size")]
    pub max_connections: u32,
}

fn default_pool_size() -> u32 {
    5
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub secret: String,
    pub listen_interval: u64,
    pub report_duration: u64,
    pub empty_live_time: i64,
}

impl Config {
    pub fn load(filename: impl AsRef<Path>) -> Result<Self, Error> {
        let config = fs::read_to_string(filename.as_ref()).map_err(|_| Error::ConfigRead)?;
        serde_yaml::from_str(&config).map_err(|_| Error::ConfigParse)
    }
}

impl DbConfig {
    pub fn server_url(&self) -> String {
        if self.password.is_empty() {
            format!("postgres://{}@{}:{}", self.user, self.host, self.port)
        } else {
            format!(
                "postgres://{}:{}@{}:{}",
                self.user, self.password, self.host, self.port
            )
        }
    }
    pub fn url(&self) -> String {
        format!("{}/{}", self.server_url(), self.dbname)
    }
}

impl ServerConfig {
    pub fn url_with(&self, https: bool) -> String {
        if https {
            format!("https://{}:{}", self.host, self.port)
        } else {
            format!("http://{}:{}", self.host, self.port)
        }
    }
    pub fn url(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load() {
        let config = Config::load("../config/manager.yaml").unwrap();
        assert_eq!(
            config,
            Config {
                db: DbConfig {
                    user: "postgres".to_string(),
                    password: "postgres".to_string(),
                    host: "localhost".to_string(),
                    port: 5432,
                    dbname: "chat".to_string(),
                    max_connections: 5,
                },
                server: ServerConfig {
                    secret: "secret".to_string(),
                    host: "0.0.0.0".to_string(),
                    port: 50051,
                    listen_interval: 1,
                    report_duration: 3,
                    empty_live_time: 30,
                },
            }
        )
    }
}
