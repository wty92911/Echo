//! A Simple Echo Server Implementation
//!
//!
//!
use crate::Result;
use crate::connection::Connection;

use std::future::Future;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, instrument};
use crate::shutdown::Shutdown;

/// Server listener state. Created in the `run` call. It includes a `run` method
/// which performs the TCP listening and initialization of per-connection state.
#[derive(Debug)]
struct Listener {
    /// Tcp Listener
    listener: TcpListener,

    /// Limiter to limit the number of concurrent connections
    limiter: Arc<tokio::sync::Semaphore>,

    /// Sender to notify the shutdown signal to the connections
    notify_shutdown: broadcast::Sender<()>,

    /// Channel to transmit shutdown notifications
    shutdown_complete_tx: mpsc::Sender<()>,
}

impl Listener {
    async fn run(&mut self) -> Result<()> {
        info!("listening");

        loop {
            // Wait until permit is ready
            // `acquire()` returns a `Err` when the semaphore is closed.
            let permit = self.limiter.clone().acquire_owned().await.expect("failed to acquire permit");


            let socket = self.accept().await?;

            // Spawn a new handler to handle the connection
            let mut handler = Handler {
                connection: Connection::new(socket),
                shutdown: Shutdown::new(self.notify_shutdown.subscribe()),
                _shutdown_complete: self.shutdown_complete_tx.clone(),
            };
            tokio::spawn(async move {
                if let Err(e) = handler.run().await {
                    error!(cause = %e, "connection error");
                }
                drop(permit);
            });
        }
        Ok(())
    }

    /// Accept a new connection
    /// # TODO
    /// exponential backoff strategy
    async fn accept(&self) -> Result<TcpStream> {
        let (socket, _) = self.listener.accept().await?;
        Ok(socket)
    }
}

/// Per-connection handler, take requests from the `connection` and apply it to the `service`.
struct Handler {
    connection: Connection,
    shutdown: Shutdown,

    /// when handler is dropped, _shutdown_complete will close, and notify receiver
    _shutdown_complete: mpsc::Sender<()>,
}

impl Handler {
    async fn run(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Hardcoded limit on the number of concurrent connections
const MAX_CONNECTIONS: usize = 256;
pub async fn run(listener: TcpListener, shutdown: impl Future) {
    // shutdown signal should be bidirectional
    let (notify_shutdown, _) = broadcast::channel(1);
    let (shutdown_complete_tx, mut shutdown_complete_rx) = mpsc::channel(1);

    let mut server = Listener {
        listener,
        limiter: Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS)),
        notify_shutdown,
        shutdown_complete_tx,
    };

    select! {
        res = server.run() => {
            if let Err(e) = res {
                error!(cause = %e, "failed to accept")
            }
        }
        _ = shutdown => {
            info!("shutting down");
        }
    }

    // Extract the shutdown complete transmitter and shutdown sender
    let Listener {
        shutdown_complete_tx,
        notify_shutdown,
        ..
    } = server;

    // When `notify_shutdown` is dropped, all tasks which have `subscribe`d will
    // receive the shutdown signal and can exit
    drop(notify_shutdown);
    // Drop final `Sender` so the `Receiver` below can complete
    drop(shutdown_complete_tx);

    // Wait for all active connections to finish processing. As the `Sender`
    // handle held by the listener has been dropped above, the only remaining
    // `Sender` instances are held by connection handler tasks. When those drop,
    // the `mpsc` channel will close and `recv()` will return `None`.
    let _ = shutdown_complete_rx.recv();
    info!("shutdown complete");
}