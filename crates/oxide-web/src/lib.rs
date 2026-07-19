//! # oxide-web
//!
//! An ergonomic, async web framework built on Tokio, Hyper, and the [`http`]
//! crate. This is the top-level facade crate: it re-exports the public API from
//! the internal `oxide-web-*` crates so applications depend on just `oxide-web`.
//!
//! ## Quick start
//!
//! ```no_run
//! use oxide_web::{Application, routing::get};
//!
//! async fn hello() -> &'static str {
//!     "Hello, world!"
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     Application::new()
//!         .route("/", get(hello))
//!         .bind("127.0.0.1:8080")
//!         .graceful_shutdown(oxide_web::shutdown::ctrl_c())
//!         .run()
//!         .await?;
//!     Ok(())
//! }
//! ```
//!
//! ## What's here (Phase 1)
//!
//! - [`Application`] — the builder: routes, state, config, bind, run.
//! - [`routing`] — `get`/`post`/… and [`MethodRouter`](routing::MethodRouter).
//! - [`IntoResponse`] — how handler return values become responses.
//! - [`shutdown`] — graceful-shutdown signals.
//!
//! Extractors, the typed error model, and the middleware stack arrive in later
//! phases; see the crate's `docs/roadmap.md`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

mod application;

pub use application::{AppService, Application, BoundServer, StartError, ToBindAddr};

// Routing surface (from `oxide-web-router`).
pub use oxide_web_router::{Params, RouteError, Router, routing};

// Shutdown signals (from `oxide-web-core`).
pub use oxide_web_core::shutdown;

// Core request/response and server types.
pub use oxide_web_core::{
    Body, BoxError, BoxFuture, Handler, IntoResponse, IntoResponseParts, RemoteAddr, Request,
    Response, ResponseParts, ServerConfig, ServerError, Service, serve, serve_on,
};

// Commonly used `http` types, re-exported for convenience.
pub use oxide_web_core::{
    HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, Version, header, http,
};
