mod pb;
use std::pin::Pin;

use futures::Stream;
pub use pb::*;

mod auth;
pub mod config;
pub mod db;
mod error;
pub mod hash;
pub mod servers;
pub type Result<T> = std::result::Result<T, error::Error>;
pub type TonicStream<T> = Pin<Box<dyn Stream<Item = tonic::Result<T>> + Send + 'static>>;
pub mod client;
