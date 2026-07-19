//! Routing for the **oxide-web** framework.
//!
//! This crate matches incoming requests to handlers using a path-segment trie
//! (a radix tree keyed on `/`-delimited segments), then dispatches on the HTTP
//! method:
//!
//! - [`Router`] — the builder you register routes on.
//! - [`routing`] — the [`get`](routing::get)/[`post`](routing::post)/… helpers
//!   and [`MethodRouter`](routing::MethodRouter).
//! - [`Params`] — path parameters captured during matching, exposed via request
//!   extensions.
//!
//! Priority is **static > parameter > wildcard**, matching is deterministic, and
//! conflicting or malformed routes are reported as [`RouteError`]s when the
//! router is finalized.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

mod error;
mod method;
mod params;
mod router;
mod tree;

pub use error::RouteError;
pub use params::Params;
pub use router::{Router, RouterService, not_found_response};

/// Method-dispatch helpers: `get`, `post`, … and [`MethodRouter`](routing::MethodRouter).
pub mod routing {
    pub use crate::method::{MethodRouter, delete, get, head, on, options, patch, post, put};
}
