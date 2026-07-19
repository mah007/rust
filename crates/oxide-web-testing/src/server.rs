//! The [`TestServer`]: start an application on an ephemeral port and talk to it
//! over real HTTP.

use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use http::{Request as HttpRequest, header};
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use oxide_web::{Application, Method, ServerConfig, ServerError, StartError, serve_on};

use crate::response::TestResponse;

/// A running test server bound to an ephemeral local port.
///
/// Start one from an [`Application`], then issue real HTTP requests against it.
/// The server is shut down and its task aborted when the `TestServer` is
/// dropped, so each test cleans up after itself.
///
/// ```no_run
/// use oxide_web::{Application, routing::get};
/// use oxide_web_testing::TestServer;
///
/// # async fn demo() {
/// async fn hello() -> &'static str { "hi" }
///
/// let app = Application::new().route("/", get(hello));
/// let server = TestServer::start(app).await.unwrap();
/// server.get("/").await.assert_ok().assert_text("hi");
/// # }
/// ```
pub struct TestServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Bind `app` to `127.0.0.1:0` (an ephemeral port) and start serving.
    ///
    /// # Errors
    ///
    /// Returns [`StartError`] if the application has route errors or the
    /// listener cannot be bound.
    pub async fn start(app: Application) -> Result<TestServer, StartError> {
        Self::start_with_config(app, test_config()).await
    }

    /// Like [`start`](TestServer::start) but with an explicit [`ServerConfig`].
    ///
    /// The configuration's `bind_addresses` are ignored — the harness always
    /// binds an ephemeral port — but timeouts and limits are honored.
    ///
    /// # Errors
    ///
    /// Returns [`StartError`] if the application has route errors or the
    /// listener cannot be bound.
    pub async fn start_with_config(
        app: Application,
        config: ServerConfig,
    ) -> Result<TestServer, StartError> {
        let service = app.into_service().map_err(StartError::Route)?;

        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|err| StartError::Server(ServerError::Io(err)))?;
        let addr = listener
            .local_addr()
            .map_err(|err| StartError::Server(ServerError::Io(err)))?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(async move {
            let shutdown = async move {
                let _ = shutdown_rx.await;
            };
            if let Err(err) = serve_on(vec![listener], service, config, shutdown).await {
                eprintln!("test server terminated with error: {err}");
            }
        });

        Ok(TestServer {
            addr,
            shutdown_tx: Some(shutdown_tx),
            handle: Some(handle),
        })
    }

    /// The address the server is listening on.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The base URL of the server, e.g. `http://127.0.0.1:54321`.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Issue a `GET` request to `path`.
    pub async fn get(&self, path: &str) -> TestResponse {
        self.request(Method::GET, path, Bytes::new()).await
    }

    /// Issue a `POST` request to `path` with `body`.
    pub async fn post(&self, path: &str, body: impl Into<Bytes>) -> TestResponse {
        self.request(Method::POST, path, body).await
    }

    /// Issue a `PUT` request to `path` with `body`.
    pub async fn put(&self, path: &str, body: impl Into<Bytes>) -> TestResponse {
        self.request(Method::PUT, path, body).await
    }

    /// Issue a `DELETE` request to `path`.
    pub async fn delete(&self, path: &str) -> TestResponse {
        self.request(Method::DELETE, path, Bytes::new()).await
    }

    /// Issue a request with an arbitrary method and body, panicking on any
    /// transport error.
    ///
    /// Use [`try_request`](TestServer::try_request) to observe transport errors.
    pub async fn request(
        &self,
        method: Method,
        path: &str,
        body: impl Into<Bytes>,
    ) -> TestResponse {
        self.try_request(method, path, body)
            .await
            .expect("test HTTP request failed")
    }

    /// Issue a request, returning a transport error as `Err` instead of
    /// panicking.
    ///
    /// A fresh connection is opened per request, so this also exercises
    /// connection setup and teardown.
    ///
    /// # Errors
    ///
    /// Returns a message describing any connection, protocol, or body error.
    pub async fn try_request(
        &self,
        method: Method,
        path: &str,
        body: impl Into<Bytes>,
    ) -> Result<TestResponse, String> {
        let stream = TcpStream::connect(self.addr)
            .await
            .map_err(|err| format!("connect failed: {err}"))?;
        let io = TokioIo::new(stream);
        let (mut sender, conn) = http1::handshake(io)
            .await
            .map_err(|err| format!("handshake failed: {err}"))?;
        let conn_task = tokio::spawn(async move {
            let _ = conn.await;
        });

        let request = HttpRequest::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, self.addr.to_string())
            .body(Full::new(body.into()))
            .map_err(|err| format!("invalid request: {err}"))?;

        let response = sender
            .send_request(request)
            .await
            .map_err(|err| format!("request failed: {err}"))?;

        let (parts, body) = response.into_parts();
        let bytes = body
            .collect()
            .await
            .map_err(|err| format!("reading body failed: {err}"))?
            .to_bytes();

        conn_task.abort();
        Ok(TestResponse::new(parts.status, parts.headers, bytes))
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

fn test_config() -> ServerConfig {
    // `ServerConfig` is `#[non_exhaustive]`, so construct via mutation of the
    // defaults rather than a struct literal.
    let mut config = ServerConfig::default();
    config.graceful_shutdown_timeout = Duration::from_secs(2);
    config
}
