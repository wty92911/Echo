//! Providing a protocol for the client/server to send and receive messages and
//! utilities to serialize/deserialize them from/into bytes.

use bytes::Bytes;
use crate::user::User;

/// Defines several types of frames that can be sent over the wire.
#[derive(Clone, Debug)]
pub enum Frame {
    Error(String),
    Message(Message),
    Login(User),
    Quit(User),
    Ping,
}

/// A message is a frame that contains audio data.
#[derive(Clone, Debug)]
pub struct Message {
    data: Bytes,
}
