//! The Phase 1 `hello-world` example.
//!
//! Run it with:
//!
//! ```bash
//! cargo run -p hello-world
//! # then, in another terminal:
//! curl -i http://127.0.0.1:8080/          # 200 OK
//! curl -i http://127.0.0.1:8080/health    # 200 OK
//! curl -i http://127.0.0.1:8080/not-found # 404 Not Found
//! # press Ctrl+C in the server terminal for a graceful shutdown
//! ```

use std::net::SocketAddr;

use oxide_web::{Application, RemoteAddr, Request, routing::get};
use tracing_subscriber::EnvFilter;

/// Root handler: a plain-text greeting.
async fn index() -> &'static str {
    "Hello, world!\n"
}

/// Liveness probe.
async fn health() -> &'static str {
    "OK"
}

/// Demonstrates reading the connection's remote address from the request.
async fn whoami(req: Request) -> String {
    match req.extensions().get::<RemoteAddr>() {
        Some(addr) => format!("You are {}\n", addr.addr()),
        None => "Unknown peer\n".to_owned(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Structured logging. Override verbosity with e.g. `RUST_LOG=debug`.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,oxide_web_core=info")),
        )
        .init();

    let app = Application::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/whoami", get(whoami));

    // Bind address defaults to 127.0.0.1:8080; override with `OXIDE_WEB_ADDR`
    // (e.g. `OXIDE_WEB_ADDR=127.0.0.1:9090 cargo run -p hello-world`).
    let addr: SocketAddr = std::env::var("OXIDE_WEB_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse()?;
    tracing::info!("hello-world listening on http://{addr} (press Ctrl+C to stop)");

    app.bind(addr)
        .graceful_shutdown(oxide_web::shutdown::ctrl_c())
        .run()
        .await?;

    tracing::info!("goodbye");
    Ok(())
}
