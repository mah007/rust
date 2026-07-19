//! Request-side helpers.

use std::net::SocketAddr;

/// The remote peer address of the connection a request arrived on.
///
/// The server inserts this into every request's extensions at accept time, so
/// handlers and extractors can read the *real* transport peer:
///
/// ```
/// use oxide_web_core::{Body, RemoteAddr, Request};
///
/// fn peer_of(req: &Request) -> Option<std::net::SocketAddr> {
///     req.extensions().get::<RemoteAddr>().map(RemoteAddr::addr)
/// }
/// # let _ = peer_of;
/// ```
///
/// This is deliberately the transport peer and **not** derived from
/// `X-Forwarded-For` / `Forwarded` headers — those are not trusted by default.
/// Trusted-proxy resolution is a later, opt-in feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteAddr(pub SocketAddr);

impl RemoteAddr {
    /// Return the wrapped socket address.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.0
    }
}

impl std::fmt::Display for RemoteAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<SocketAddr> for RemoteAddr {
    fn from(addr: SocketAddr) -> Self {
        RemoteAddr(addr)
    }
}
