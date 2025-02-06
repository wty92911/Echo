use crate::pb::Channel;
use sqlx::Row;
use sqlx::{postgres::PgRow, FromRow};

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

pub trait Validator {
    fn validate(&self) -> crate::Result<()>;
}

impl Validator for Channel {
    fn validate(&self) -> crate::Result<()> {
        // todo
        Ok(())
    }
}
