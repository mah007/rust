//! Error types used across the server core.

use std::fmt;

/// A boxed, thread-safe error type used for body streams and other places where
/// the concrete error type is erased.
///
/// This mirrors the convention used by `hyper`/`tower` so the framework composes
/// cleanly with the wider ecosystem.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Errors that can occur while starting or running the server.
///
/// Network errors that occur *per connection* while the server is running are
/// logged and never surfaced here — they must not take the whole server down.
/// This type only represents failures that abort startup or the run loop.
#[derive(Debug)]
#[non_exhaustive]
pub enum ServerError {
    /// The server could not bind one of its configured addresses.
    Bind {
        /// The address that failed to bind.
        addr: std::net::SocketAddr,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// A configuration value was invalid (for example, no bind addresses).
    Config(String),
    /// A generic I/O error raised by the run loop.
    Io(std::io::Error),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::Bind { addr, source } => {
                write!(f, "failed to bind {addr}: {source}")
            }
            ServerError::Config(msg) => write!(f, "invalid server configuration: {msg}"),
            ServerError::Io(err) => write!(f, "server I/O error: {err}"),
        }
    }
}

impl std::error::Error for ServerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ServerError::Bind { source, .. } => Some(source),
            ServerError::Io(err) => Some(err),
            ServerError::Config(_) => None,
        }
    }
}

impl From<std::io::Error> for ServerError {
    fn from(err: std::io::Error) -> Self {
        ServerError::Io(err)
    }
}
