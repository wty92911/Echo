use crate::{config::DbConfig, Result};
use sqlx::{postgres::PgPoolOptions, PgPool};
#[derive(Debug, Clone)]
pub struct SqlHelper {
    pool: PgPool,
}

impl SqlHelper {
    pub async fn new(conf: &DbConfig) -> Result<Self> {
        Ok(Self {
            pool: PgPoolOptions::default()
                .max_connections(conf.max_connections)
                .connect(&conf.url())
                .await?,
        })
    }

    pub async fn insert_user(&self, id: &str, name: &str, password_hash: &str) -> Result<()> {
        sqlx::query("INSERT INTO chat.users (id, name, password_hash) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(name)
            .bind(password_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_user_password(&self, id: &str) -> Result<Option<String>> {
        let password_hash =
            sqlx::query_scalar("SELECT password_hash FROM chat.users WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(password_hash)
    }
}
