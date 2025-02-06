pub mod pb;
pub mod traits;

pub mod error;

pub type Result<T> = std::result::Result<T, error::Error>;
