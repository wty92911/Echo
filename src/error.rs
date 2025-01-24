use tonic::Status;
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error")]
    Db(sqlx::Error),

    // business logic error
    #[error("Invalid password")]
    InvalidPassword,
    #[error("User not found")]
    UserNotFound,
    #[error("Channel not found")]
    ChannelNotFound,
    #[error("Permission denied")]
    PermissionDenied,

    /// Config Error
    #[error("Config parse error")]
    ConfigParse,
    #[error("Config read error")]
    ConfigRead,

    // Validate Error
    #[error("Validate error")]
    Validate,
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        Error::Db(e)
    }
}

impl From<Error> for Status {
    fn from(e: Error) -> Self {
        match e {
            Error::Db(e) => Status::internal(e.to_string()),
            Error::InvalidPassword => Status::invalid_argument("Invalid password"),
            Error::UserNotFound => Status::not_found("User not found"),
            Error::ChannelNotFound => Status::not_found("Channel not found"),
            Error::PermissionDenied => Status::permission_denied("Permission denied"),
            _ => Status::internal(e.to_string()),
        }
    }
}
