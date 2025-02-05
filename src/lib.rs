mod pb;
use std::pin::Pin;

use futures::Stream;
pub use pb::*;

mod auth;
pub mod config;
pub mod db;
mod error;
mod hash;
pub mod servers;
type Result<T> = std::result::Result<T, error::Error>;
type TonicStream<T> = Pin<Box<dyn Stream<Item = tonic::Result<T>> + Send + 'static>>;
