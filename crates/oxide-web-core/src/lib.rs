//! Core building blocks for the **oxide-web** framework.
//!
//! This crate owns the low-level HTTP plumbing that the rest of the framework is
//! built on:
//!
//! - [`Body`] — the request/response body type (empty, buffered, or streaming).
//! - [`Request`] / [`Response`] — thin aliases over the [`http`] crate types.
//! - [`IntoResponse`] — how handler return values become responses.
//! - [`Handler`] — how `async fn`s become callable [`Route`]s.
//! - [`Service`] — the infallible request→response abstraction the server drives.
//! - [`serve`] + [`ServerConfig`] — the Tokio/Hyper server with graceful shutdown.
//! - [`shutdown`] — ready-made shutdown signals such as [`shutdown::ctrl_c`].
//!
//! Application authors normally use the higher-level `oxide-web` facade rather
//! than this crate directly.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

mod body;
mod error;
mod handler;
mod request;
mod response;
mod server;
mod service;
pub mod shutdown;

pub use body::Body;
pub use error::{BoxError, ServerError};
pub use handler::{BoxFuture, Handler, Route};
pub use request::RemoteAddr;
pub use response::{IntoResponse, IntoResponseParts, ResponseParts};
pub use server::{ServerConfig, serve, serve_on};
pub use service::Service;

/// A request whose body defaults to the framework [`Body`] type.
///
/// This is a re-export alias of [`http::Request`]; using it keeps handler and
/// extractor signatures familiar to anyone who knows the `http` crate.
pub type Request<B = Body> = http::Request<B>;

/// A response whose body defaults to the framework [`Body`] type.
///
/// This is a re-export alias of [`http::Response`].
pub type Response<B = Body> = http::Response<B>;

// Re-export the commonly used `http` types so downstream crates and users do not
// need to depend on `http` directly for everyday work.
pub use http::{
    self, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, Version, header,
};
