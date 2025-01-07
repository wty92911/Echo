//! A Simple Echo Server Implementation
//!
//!
//!

use tokio::net::TcpListener;

/// Server listener state. Created in the `run` call. It includes a `run` method
/// which performs the TCP listening and initialization of per-connection state.
#[derive(Debug)]
struct Listener {
    listener: TcpListener,
}

impl Listener {

}

/// Per-connection handler, take requests from the `connection` and apply it to the `service`.
struct Handler {

}
