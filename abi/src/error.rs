use tonic::Status;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error")]
    Db(sqlx::Error),

    #[error("Connect error")]
    Connect(tonic::transport::Error),

    #[error("Rpc error: `{0}`")]
    Rpc(Status),
    // business logic error
    #[error("Invalid password")]
    InvalidPassword,
    #[error("User not found")]
    UserNotFound,
    #[error("Channel not found")]
    ChannelNotFound,
    #[error("Server not found")]
    ServerNotFound,
    #[error("Permission denied: `{0}`")]
    PermissionDenied(&'static str),

    /// Config Error
    #[error("Config parse error")]
    ConfigParse,
    #[error("Config read error")]
    ConfigRead,

    // Validate Error
    #[error("Validate error")]
    Validate,
    #[error("Invalid Request: `{0}`")]
    InvalidRequest(&'static str),

    // Channel Chat Error
    #[error("Channel Broadcast Stopped")]
    ChannelBroadcastStopped,

    // intercept by limiter
    #[error("Intercepted By Limiter")]
    Limit,
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        Error::Db(e)
    }
}

impl From<tonic::transport::Error> for Error {
    fn from(e: tonic::transport::Error) -> Self {
        Error::Connect(e)
    }
}

impl From<tonic::Status> for Error {
    fn from(e: tonic::Status) -> Self {
        Error::Rpc(e)
    }
}
impl From<Error> for Status {
    fn from(e: Error) -> Self {
        match e {
            Error::Db(e) => Status::internal(e.to_string()),
            Error::InvalidPassword => Status::invalid_argument("Invalid password"),
            Error::UserNotFound => Status::not_found("User not found"),
            Error::ChannelNotFound => Status::not_found("Channel not found"),
            Error::ServerNotFound => Status::not_found("Server not found"),
            Error::PermissionDenied(s) => {
                Status::permission_denied(format!("Permission denied: `{}`", s))
            }
            Error::ChannelBroadcastStopped => Status::aborted("Channel Broadcast Stopped"),
            _ => Status::internal(e.to_string()),
        }
    }
}
