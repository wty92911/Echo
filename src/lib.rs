pub mod auth;
pub mod core;
mod pb;
pub mod service;
pub use pb::*;

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;
