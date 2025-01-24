use crate::{config::DbConfig, Channel, Result};
use sqlx::{
    postgres::{PgPoolOptions, PgRow},
    FromRow, PgPool, Postgres, Row,
};
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

    pub async fn get_channels(&self, channel_id: &i32) -> Result<Vec<Channel>> {
        Ok(match *channel_id {
            0 => {
                sqlx::query_as("SELECT id, name, limit_num FROM chat.channels")
                    .fetch_all(&self.pool)
                    .await?
            }
            id => {
                sqlx::query_as("SELECT id, name, limit_num FROM chat.channels WHERE id = $1")
                    .bind(id)
                    .fetch_all(&self.pool)
                    .await?
            }
        })
    }

    pub async fn insert_channel(&self, channel: &Channel, user_id: &str) -> Result<i32> {
        let id = sqlx::query_scalar(
            "INSERT INTO chat.channels (name, limit_num, owner_id) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(channel.name.clone())
        .bind(channel.limit)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn delete_channel(&self, id: &i32) -> Result<()> {
        sqlx::query("DELETE FROM chat.channels WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_channel_owner(&self, id: &i32) -> Result<Option<String>> {
        let owner_id = sqlx::query_scalar("SELECT owner_id FROM chat.channels WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(owner_id)
    }
}

impl FromRow<'_, PgRow> for Channel {
    fn from_row(row: &PgRow) -> sqlx::Result<Self, sqlx::Error> {
        let id: i32 = row.get("id");
        let limit: i32 = row.get("limit_num");
        Ok(Channel {
            id,
            name: row.get("name"),
            users: vec![],
            limit,
        })
    }
}

impl From<sqlx::Pool<Postgres>> for SqlHelper {
    fn from(pool: sqlx::Pool<Postgres>) -> Self {
        Self { pool }
    }
}
