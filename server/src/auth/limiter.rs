use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::Instant;

use abi::error::Error;

#[async_trait]
pub trait Limiter: Send + Sync {
    async fn is_allowed(&self, key: &str) -> abi::Result<()>;
}

#[derive(Debug, Clone)]
pub struct LimiterConfig {
    pub limit: u32,
    pub duration: Duration,
}

impl LimiterConfig {
    pub fn new(limit: u32, duration: Duration) -> Self {
        Self { limit, duration }
    }
}

#[derive(Debug)]
pub struct FixedWindowLimiter {
    config: LimiterConfig,
    counts: Arc<RwLock<HashMap<String, (u32, Instant)>>>,
}

impl FixedWindowLimiter {
    pub fn new(config: LimiterConfig) -> Self {
        Self {
            config,
            counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Limiter for FixedWindowLimiter {
    async fn is_allowed(&self, key: &str) -> abi::Result<()> {
        let mut counts = self.counts.write().await;
        let now = Instant::now();

        let entry = counts.entry(key.to_string()).or_insert((0, now));
        if now.duration_since(entry.1) >= self.config.duration {
            *entry = (1, now);
            Ok(())
        } else if entry.0 < self.config.limit {
            entry.0 += 1;
            Ok(())
        } else {
            Err(Error::Limit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_fixed_window_limiter() {
        let config = LimiterConfig::new(1, Duration::from_secs(5));
        let limiter = FixedWindowLimiter::new(config);

        let key = "user:123";

        // First request should be allowed
        assert!(limiter.is_allowed(key).await.is_ok());

        // Second request within the window should not be allowed
        assert!(limiter.is_allowed(key).await.is_err());

        // Wait for the duration to pass
        sleep(Duration::from_secs(5)).await;

        // Request after the window should be allowed again
        assert!(limiter.is_allowed(key).await.is_ok());

        // And once again, the next request should not be allowed within the same window
        assert!(limiter.is_allowed(key).await.is_err());
    }
}
