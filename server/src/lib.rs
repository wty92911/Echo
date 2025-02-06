use std::pin::Pin;

use futures::Stream;

mod auth;
pub mod config;
pub mod db;
pub mod hash;
pub mod servers;

type TonicStream<T> = Pin<Box<dyn Stream<Item = tonic::Result<T>> + Send + 'static>>;
