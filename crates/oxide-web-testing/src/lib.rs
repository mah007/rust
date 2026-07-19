//! Test harness for the **oxide-web** framework.
//!
//! [`TestServer`] starts an [`Application`](oxide_web::Application) on an
//! ephemeral local port and lets you drive it with real HTTP requests, then make
//! chainable assertions with [`TestResponse`]:
//!
//! ```no_run
//! use oxide_web::{Application, routing::get};
//! use oxide_web_testing::TestServer;
//!
//! # async fn demo() {
//! async fn health() -> &'static str { "OK" }
//!
//! let app = Application::new().route("/health", get(health));
//! let server = TestServer::start(app).await.unwrap();
//!
//! server
//!     .get("/health")
//!     .await
//!     .assert_ok()
//!     .assert_text("OK");
//!
//! server.get("/missing").await.assert_not_found();
//! # }
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

mod response;
mod server;

pub use response::TestResponse;
pub use server::TestServer;
