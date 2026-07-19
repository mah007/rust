//! The [`Application`] builder and its supporting types.

use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use oxide_web_core::{
    BoxFuture, Handler, Request, Response, ServerConfig, ServerError, Service, serve, shutdown,
};
use oxide_web_router::{RouteError, Router, RouterService, routing::MethodRouter};

type Injector = Arc<dyn Fn(&mut http::Extensions) + Send + Sync>;

// Re-export `http::Extensions` indirectly through core's `http` re-export.
use oxide_web_core::http;

/// The central builder for an oxide-web application.
///
/// Register routes, attach shared state, choose configuration, then [`bind`] and
/// [`run`]:
///
/// ```no_run
/// use oxide_web::{Application, routing::get};
///
/// async fn health() -> &'static str { "OK" }
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// Application::new()
///     .route("/health", get(health))
///     .bind("127.0.0.1:8080")
///     .graceful_shutdown(oxide_web::shutdown::ctrl_c())
///     .run()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// [`bind`]: Application::bind
/// [`run`]: BoundServer::run
pub struct Application {
    router: Router,
    config: ServerConfig,
    injectors: Vec<Injector>,
}

impl Default for Application {
    fn default() -> Self {
        Application::new()
    }
}

impl Application {
    /// Create a new, empty application with default [`ServerConfig`].
    #[must_use]
    pub fn new() -> Self {
        Application {
            router: Router::new(),
            config: ServerConfig::default(),
            injectors: Vec::new(),
        }
    }

    /// Register a route: a [`MethodRouter`] (from `get(..)`, `post(..)`, …) at
    /// `path`.
    #[must_use]
    pub fn route(mut self, path: &str, method_router: MethodRouter) -> Self {
        self.router = self.router.route(path, method_router);
        self
    }

    /// Set the fallback handler used when no route matches.
    #[must_use]
    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.router = self.router.fallback(handler);
        self
    }

    /// Attach shared state, made available to handlers via request extensions.
    ///
    /// The value must be cheap to clone (typically an `Arc`); a clone is
    /// inserted into every request's extensions. Handlers can read it with
    /// `req.extensions().get::<S>()`. Typed `State<T>` extraction is added in a
    /// later phase and builds on this mechanism.
    #[must_use]
    pub fn with_state<S>(mut self, state: S) -> Self
    where
        S: Clone + Send + Sync + 'static,
    {
        self.injectors.push(Arc::new(move |extensions| {
            extensions.insert(state.clone());
        }));
        self
    }

    /// Replace the server configuration.
    ///
    /// Note that [`bind`](Application::bind) overrides
    /// [`ServerConfig::bind_addresses`].
    #[must_use]
    pub fn config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }

    /// Access the current configuration (for inspection or small tweaks).
    #[must_use]
    pub fn config_ref(&self) -> &ServerConfig {
        &self.config
    }

    /// Finalize the application into a runnable [`Service`], without binding a
    /// listener. Useful for in-process testing.
    ///
    /// # Errors
    ///
    /// Returns any [`RouteError`] recorded while registering routes.
    pub fn into_service(self) -> Result<AppService, RouteError> {
        Ok(AppService {
            router: self.router.into_service()?,
            injectors: Arc::from(self.injectors),
        })
    }

    /// Bind the application to `addr`, producing a [`BoundServer`] ready to run.
    ///
    /// This overrides any addresses in the configuration. Address-parse and
    /// route-registration errors are deferred to [`BoundServer::run`].
    #[must_use]
    pub fn bind(self, addr: impl ToBindAddr) -> BoundServer {
        let Application {
            router,
            mut config,
            injectors,
        } = self;

        let result = (|| {
            let addr = addr.to_bind_addr().map_err(StartError::Address)?;
            config.bind_addresses = vec![addr];
            let service = AppService {
                router: router.into_service().map_err(StartError::Route)?,
                injectors: Arc::from(injectors),
            };
            Ok((service, config))
        })();

        BoundServer {
            result,
            shutdown: None,
        }
    }
}

/// The finalized, cloneable application service.
///
/// Produced by [`Application::into_service`] or (internally) by
/// [`Application::bind`]. It inserts any shared state into each request's
/// extensions, then dispatches through the router.
#[derive(Clone)]
pub struct AppService {
    router: RouterService,
    injectors: Arc<[Injector]>,
}

impl Service for AppService {
    fn call(&self, mut req: Request) -> BoxFuture<Response> {
        for injector in self.injectors.iter() {
            injector(req.extensions_mut());
        }
        self.router.call(req)
    }
}

/// An [`Application`] bound to an address, awaiting [`run`](BoundServer::run).
pub struct BoundServer {
    result: Result<(AppService, ServerConfig), StartError>,
    shutdown: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
}

impl BoundServer {
    /// Set the shutdown signal that triggers graceful shutdown.
    ///
    /// If not set, the server runs until the process is terminated.
    #[must_use]
    pub fn graceful_shutdown<F>(mut self, shutdown: F) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.shutdown = Some(Box::pin(shutdown));
        self
    }

    /// Run the server until the shutdown signal fires, then drain and return.
    ///
    /// If no shutdown signal was configured, [`shutdown::ctrl_c`] is used.
    ///
    /// # Errors
    ///
    /// Returns [`StartError`] if route registration failed, the address was
    /// invalid, or the server could not bind/serve.
    pub async fn run(self) -> Result<(), StartError> {
        let BoundServer { result, shutdown } = self;
        let (service, config) = result?;
        let shutdown: Pin<Box<dyn Future<Output = ()> + Send>> =
            shutdown.unwrap_or_else(|| Box::pin(shutdown::ctrl_c()));
        serve(service, config, shutdown)
            .await
            .map_err(StartError::Server)
    }
}

/// A type that can be interpreted as a bind address.
///
/// Implemented for [`SocketAddr`], `&str`, and `String` (the string forms are
/// parsed as `IP:port`, e.g. `"127.0.0.1:8080"`).
pub trait ToBindAddr {
    /// Convert into a [`SocketAddr`], or return a human-readable parse error.
    ///
    /// # Errors
    ///
    /// Returns a message describing why the value is not a valid `IP:port`.
    fn to_bind_addr(self) -> Result<SocketAddr, String>;
}

impl ToBindAddr for SocketAddr {
    fn to_bind_addr(self) -> Result<SocketAddr, String> {
        Ok(self)
    }
}

impl ToBindAddr for &str {
    fn to_bind_addr(self) -> Result<SocketAddr, String> {
        self.parse()
            .map_err(|_| format!("`{self}` is not a valid `IP:port` bind address"))
    }
}

impl ToBindAddr for String {
    fn to_bind_addr(self) -> Result<SocketAddr, String> {
        self.as_str().to_bind_addr()
    }
}

/// An error that prevents an application from starting.
#[derive(Debug)]
#[non_exhaustive]
pub enum StartError {
    /// Route registration failed (conflict or invalid pattern).
    Route(RouteError),
    /// The bind address could not be parsed.
    Address(String),
    /// The server failed to bind or run.
    Server(ServerError),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartError::Route(err) => write!(f, "route registration failed: {err}"),
            StartError::Address(msg) => write!(f, "invalid bind address: {msg}"),
            StartError::Server(err) => write!(f, "server error: {err}"),
        }
    }
}

impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StartError::Route(err) => Some(err),
            StartError::Server(err) => Some(err),
            StartError::Address(_) => None,
        }
    }
}

impl From<RouteError> for StartError {
    fn from(err: RouteError) -> Self {
        StartError::Route(err)
    }
}

impl From<ServerError> for StartError {
    fn from(err: ServerError) -> Self {
        StartError::Server(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_web_core::{Body, Method, StatusCode};
    use oxide_web_router::routing::get;

    #[test]
    fn invalid_bind_address_errors_at_run_setup() {
        let bound = Application::new().bind("not-an-address");
        assert!(matches!(bound.result, Err(StartError::Address(_))));
    }

    #[test]
    fn duplicate_route_surfaces_route_error() {
        async fn a() -> &'static str {
            "a"
        }
        async fn b() -> &'static str {
            "b"
        }
        let bound = Application::new()
            .route("/x", get(a))
            .route("/x", get(b))
            .bind("127.0.0.1:0");
        assert!(matches!(bound.result, Err(StartError::Route(_))));
    }

    #[tokio::test]
    async fn state_is_injected_into_extensions() {
        #[derive(Clone)]
        struct AppState {
            name: &'static str,
        }
        async fn who(req: Request) -> String {
            req.extensions()
                .get::<AppState>()
                .map(|s| s.name.to_owned())
                .unwrap_or_default()
        }

        let service = Application::new()
            .route("/", get(who))
            .with_state(AppState { name: "svc" })
            .into_service()
            .unwrap();

        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let response = service.call(req).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = {
            use http_body_util::BodyExt as _;
            response.into_body().collect().await.unwrap().to_bytes()
        };
        assert_eq!(&bytes[..], b"svc");
    }
}
