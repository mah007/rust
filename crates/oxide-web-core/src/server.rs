//! The Tokio + Hyper server: [`ServerConfig`] and [`serve`].

use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use http::header::{self, HeaderValue};
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use hyper_util::server::graceful::GracefulShutdown;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::Instrument;

use crate::error::ServerError;
use crate::service::Service;
use crate::{Body, RemoteAddr, Request, Response, StatusCode};

/// Runtime configuration for the server.
///
/// Construct with [`ServerConfig::default`] and adjust fields, or build one from
/// the higher-level `oxide-web` `Application`. Defaults are safe for local
/// development; production deployments should review timeouts and body limits.
///
/// Environment-variable overrides (`OXIDE_WEB_*`) arrive in a later phase; the
/// fields here are the canonical, typed source of truth.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ServerConfig {
    /// Addresses to bind and accept connections on. At least one is required.
    pub bind_addresses: Vec<SocketAddr>,
    /// Maximum time a single request handler may run before the server returns
    /// `503 Service Unavailable`. A zero duration disables the timeout.
    pub request_timeout: Duration,
    /// Maximum time to wait for in-flight connections to drain during graceful
    /// shutdown before remaining connections are forcibly closed.
    pub graceful_shutdown_timeout: Duration,
    /// Maximum accepted request body size, in bytes.
    ///
    /// Recorded here as the canonical limit; enforcement is applied by body
    /// extractors and the body-limit layer in later phases.
    pub max_request_body_size: usize,
    /// Whether to set `TCP_NODELAY` on accepted connections.
    pub tcp_nodelay: bool,
    /// Whether HTTP/1.1 keep-alive is enabled.
    pub http1_keep_alive: bool,
    /// Whether to negotiate HTTP/2 in addition to HTTP/1.1.
    ///
    /// When `true`, connections are served by the automatic HTTP/1 + HTTP/2
    /// builder. When `false`, only HTTP/1.1 is served.
    pub http2_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addresses: vec![SocketAddr::from(([127, 0, 0, 1], 8080))],
            request_timeout: Duration::from_secs(30),
            graceful_shutdown_timeout: Duration::from_secs(20),
            max_request_body_size: 2 * 1024 * 1024,
            tcp_nodelay: true,
            http1_keep_alive: true,
            http2_enabled: true,
        }
    }
}

/// Run the server until the `shutdown` future completes, then drain.
///
/// This binds every address in [`ServerConfig::bind_addresses`], accepts
/// connections, and drives each through `service`. When `shutdown` resolves, the
/// accept loops stop and in-flight connections are given
/// [`ServerConfig::graceful_shutdown_timeout`] to finish before being closed.
///
/// Per-connection and per-accept network errors are logged and never abort the
/// server.
///
/// # Errors
///
/// Returns [`ServerError`] if no addresses are configured, if any address fails
/// to bind, or on a fatal I/O error during startup.
pub async fn serve<S, F>(service: S, config: ServerConfig, shutdown: F) -> Result<(), ServerError>
where
    S: Service,
    F: Future<Output = ()> + Send + 'static,
{
    if config.bind_addresses.is_empty() {
        return Err(ServerError::Config(
            "at least one bind address is required".to_owned(),
        ));
    }

    let mut listeners = Vec::with_capacity(config.bind_addresses.len());
    for addr in &config.bind_addresses {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|source| ServerError::Bind {
                addr: *addr,
                source,
            })?;
        listeners.push(listener);
    }

    serve_on(listeners, service, config, shutdown).await
}

/// Run the server on a set of already-bound listeners until `shutdown` fires.
///
/// This is the lower-level counterpart to [`serve`]. It is useful when the
/// caller needs the actual bound address *before* serving — for example the
/// testing harness, which binds `127.0.0.1:0` to obtain an ephemeral port.
///
/// # Errors
///
/// Returns [`ServerError`] if no listeners are provided.
pub async fn serve_on<S, F>(
    listeners: Vec<TcpListener>,
    service: S,
    config: ServerConfig,
    shutdown: F,
) -> Result<(), ServerError>
where
    S: Service,
    F: Future<Output = ()> + Send + 'static,
{
    if listeners.is_empty() {
        return Err(ServerError::Config(
            "at least one listener is required".to_owned(),
        ));
    }

    for listener in &listeners {
        if let Ok(local) = listener.local_addr() {
            tracing::info!(address = %local, "server listening");
        }
    }

    // Broadcast the shutdown signal to every accept loop.
    let (shutdown_tx, _seed_rx) = watch::channel(false);
    tokio::spawn({
        let tx = shutdown_tx.clone();
        async move {
            shutdown.await;
            tracing::info!("shutdown signal received; beginning graceful shutdown");
            let _ = tx.send(true);
        }
    });

    let mut tasks: JoinSet<()> = JoinSet::new();
    for listener in listeners {
        tasks.spawn(accept_loop(
            listener,
            service.clone(),
            config.clone(),
            shutdown_tx.subscribe(),
        ));
    }

    while let Some(joined) = tasks.join_next().await {
        if let Err(err) = joined {
            tracing::error!(error = %err, "accept task terminated unexpectedly");
        }
    }

    tracing::info!("server shutdown complete");
    Ok(())
}

enum ConnBuilder {
    Auto(auto::Builder<TokioExecutor>),
    Http1(hyper::server::conn::http1::Builder),
}

fn build_conn_builder(config: &ServerConfig) -> ConnBuilder {
    if config.http2_enabled {
        let mut builder = auto::Builder::new(TokioExecutor::new());
        builder.http1().keep_alive(config.http1_keep_alive);
        ConnBuilder::Auto(builder)
    } else {
        let mut builder = hyper::server::conn::http1::Builder::new();
        builder.keep_alive(config.http1_keep_alive);
        ConnBuilder::Http1(builder)
    }
}

async fn accept_loop<S: Service>(
    listener: TcpListener,
    service: S,
    config: ServerConfig,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let graceful = GracefulShutdown::new();
    let conn_builder = build_conn_builder(&config);
    let request_timeout = config.request_timeout;

    loop {
        tokio::select! {
            accept = listener.accept() => {
                let (stream, peer) = match accept {
                    Ok(pair) => pair,
                    Err(err) => {
                        tracing::warn!(error = %err, "failed to accept connection");
                        // Back off briefly to avoid a hot spin on persistent
                        // errors such as running out of file descriptors.
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        continue;
                    }
                };

                if config.tcp_nodelay && let Err(err) = stream.set_nodelay(true) {
                    tracing::debug!(error = %err, "failed to set TCP_NODELAY");
                }

                let io = TokioIo::new(stream);
                let svc = service.clone();
                let hyper_svc = hyper::service::service_fn(move |req: Request<Incoming>| {
                    let svc = svc.clone();
                    async move {
                        Ok::<_, Infallible>(handle_request(svc, req, peer, request_timeout).await)
                    }
                });

                // Inlined per-builder so the compiler infers the connection bounds;
                // both arms spawn a graceful-watched connection task.
                match &conn_builder {
                    ConnBuilder::Auto(builder) => {
                        let conn = builder.serve_connection(io, hyper_svc).into_owned();
                        let watched = graceful.watch(conn);
                        tokio::spawn(async move {
                            if let Err(err) = watched.await {
                                tracing::debug!(error = %err, "connection error");
                            }
                        });
                    }
                    ConnBuilder::Http1(builder) => {
                        let conn = builder.serve_connection(io, hyper_svc);
                        let watched = graceful.watch(conn);
                        tokio::spawn(async move {
                            if let Err(err) = watched.await {
                                tracing::debug!(error = %err, "connection error");
                            }
                        });
                    }
                }
            }
            changed = shutdown_rx.changed() => {
                if changed.is_ok() {
                    tracing::info!("stopping accept loop; draining in-flight connections");
                }
                break;
            }
        }
    }

    // Stop accepting, then drain in-flight connections with a bounded timeout.
    drop(listener);
    tokio::select! {
        () = graceful.shutdown() => {
            tracing::info!("all in-flight connections drained");
        }
        () = tokio::time::sleep(config.graceful_shutdown_timeout) => {
            tracing::warn!(
                timeout_ms = config.graceful_shutdown_timeout.as_millis() as u64,
                "drain timeout exceeded; closing remaining connections",
            );
        }
    }
}

async fn handle_request<S: Service>(
    service: S,
    req: Request<Incoming>,
    peer: SocketAddr,
    request_timeout: Duration,
) -> Response<Body> {
    let mut req = req.map(Body::from_incoming);
    req.extensions_mut().insert(RemoteAddr(peer));

    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let span = tracing::info_span!("http_request", method = %method, path = %path, peer = %peer);

    async move {
        let start = Instant::now();
        let response = if request_timeout.is_zero() {
            service.call(req).await
        } else {
            match tokio::time::timeout(request_timeout, service.call(req)).await {
                Ok(response) => response,
                Err(_) => {
                    tracing::warn!(
                        timeout_ms = request_timeout.as_millis() as u64,
                        "request handler timed out",
                    );
                    timeout_response()
                }
            }
        };
        let latency = start.elapsed();
        tracing::info!(
            status = response.status().as_u16(),
            latency_ms = latency.as_millis() as u64,
            "request completed",
        );
        response
    }
    .instrument(span)
    .await
}

fn timeout_response() -> Response<Body> {
    let mut response = Response::new(Body::from("request timed out"));
    *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_sane() {
        let config = ServerConfig::default();
        assert_eq!(config.bind_addresses.len(), 1);
        assert!(config.request_timeout > Duration::ZERO);
        assert!(config.http1_keep_alive);
        assert_eq!(config.max_request_body_size, 2 * 1024 * 1024);
    }

    #[test]
    fn timeout_response_is_503_text() {
        let response = timeout_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
    }
}
