//! The [`Service`] abstraction the server drives.

use crate::handler::BoxFuture;
use crate::{Request, Response};

/// An infallible request → response service.
///
/// This is the contract between the router (which implements it) and the server
/// (which drives it). It is deliberately **infallible**: in this framework
/// errors are turned into responses *before* they reach the server boundary, so
/// there is no error type to leak to clients or crash a connection.
///
/// A `Service` must be cheap to [`Clone`] — the server clones it once per
/// connection — which in practice means holding shared state behind an `Arc`.
///
/// Tower interop (`tower::Service` / `tower::Layer`) is layered on top of this in
/// the middleware phase; this trait keeps the hot path simple and allocation-
/// light until then.
pub trait Service: Clone + Send + Sync + 'static {
    /// Handle a single request.
    fn call(&self, req: Request) -> BoxFuture<Response>;
}
