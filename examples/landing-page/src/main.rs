//! A Bootstrap-based landing page served by oxide-web.
//!
//! Run it with:
//!
//! ```bash
//! cargo run -p landing-page
//! # then open http://127.0.0.1:8080 in a browser
//! # override the address with OXIDE_WEB_ADDR=127.0.0.1:9090 cargo run -p landing-page
//! ```

use std::net::SocketAddr;

use oxide_web::{Application, Html, routing::get};
use tracing_subscriber::EnvFilter;

/// The landing page HTML, embedded at compile time.
///
/// It references Bootstrap 5 from a CDN, so the page styles itself in the
/// browser without the framework needing static-file serving (that arrives in a
/// later phase).
const INDEX_HTML: &str = include_str!("../static/index.html");

/// Serve the landing page as an HTML response.
async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

/// A simple liveness probe.
async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,oxide_web_core=info")),
        )
        .init();

    let app = Application::new()
        .route("/", get(index))
        .route("/health", get(health));

    let addr: SocketAddr = std::env::var("OXIDE_WEB_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse()?;
    tracing::info!("landing page live at http://{addr} (press Ctrl+C to stop)");

    app.bind(addr)
        .graceful_shutdown(oxide_web::shutdown::ctrl_c())
        .run()
        .await?;

    Ok(())
}
