//! The [`Router`] builder and its runtime [`RouterService`].

use std::sync::Arc;

use oxide_web_core::{
    BoxFuture, Handler, HeaderValue, IntoResponse, Request, Response, Route, Service, StatusCode,
    header,
};

use crate::error::RouteError;
use crate::method::MethodRouter;
use crate::params::Params;
use crate::tree::Node;

/// A collection of routes, built up with [`Router::route`] and friends.
///
/// A `Router` is a *builder*: registering routes may fail (duplicate paths,
/// invalid patterns), and those errors are collected and surfaced when the
/// router is finalized with [`Router::into_service`]. The runtime type,
/// [`RouterService`], is cheap to clone and implements [`Service`].
///
/// # Examples
///
/// ```
/// use oxide_web_router::{Router, routing::get};
///
/// async fn index() -> &'static str { "hello" }
///
/// let router = Router::new().route("/", get(index));
/// let service = router.into_service().expect("valid routes");
/// # let _ = service;
/// ```
pub struct Router {
    tree: Node<MethodRouter>,
    fallback: Route,
    errors: Vec<RouteError>,
}

impl Default for Router {
    fn default() -> Self {
        Router::new()
    }
}

impl Router {
    /// Create an empty router with the default `404 Not Found` fallback.
    #[must_use]
    pub fn new() -> Self {
        Router {
            tree: Node::new(),
            fallback: default_not_found_route(),
            errors: Vec::new(),
        }
    }

    /// Register `method_router` at `path`.
    ///
    /// Registering multiple methods for the same path (across calls) merges
    /// them. Duplicate methods, duplicate paths, or invalid patterns are
    /// recorded and reported by [`Router::into_service`].
    #[must_use]
    pub fn route(mut self, path: &str, method_router: MethodRouter) -> Self {
        if !path.starts_with('/') {
            self.errors.push(RouteError::invalid_pattern(
                path,
                "route paths must start with `/`",
            ));
            return self;
        }

        match self.tree.at_or_insert(path) {
            Ok(slot) => match slot {
                Some(existing) => {
                    if let Err(method) = existing.merge(method_router) {
                        self.errors.push(RouteError::conflict(
                            path,
                            format!("method `{method}` is already registered for this path"),
                        ));
                    }
                }
                None => *slot = Some(method_router),
            },
            Err(err) => self.errors.push(err),
        }

        self
    }

    /// Set the fallback handler used when no route matches (replacing the
    /// default `404`).
    #[must_use]
    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.fallback = handler.into_route();
        self
    }

    /// Finalize the router into a runnable [`RouterService`].
    ///
    /// # Errors
    ///
    /// Returns the first [`RouteError`] recorded during registration, if any.
    pub fn into_service(self) -> Result<RouterService, RouteError> {
        if let Some(err) = self.errors.into_iter().next() {
            return Err(err);
        }
        Ok(RouterService {
            tree: Arc::new(self.tree),
            fallback: self.fallback,
        })
    }

    /// Return the registration errors collected so far, if any.
    #[must_use]
    pub fn errors(&self) -> &[RouteError] {
        &self.errors
    }
}

/// The runtime form of a [`Router`]: cheap to clone and ready to serve.
#[derive(Clone)]
pub struct RouterService {
    tree: Arc<Node<MethodRouter>>,
    fallback: Route,
}

enum Outcome {
    Route(Route),
    MethodNotAllowed(HeaderValue),
    NotFound,
}

impl Service for RouterService {
    fn call(&self, req: Request) -> BoxFuture<Response> {
        let path = req.uri().path().to_owned();
        let mut params = Vec::new();

        let outcome = match self.tree.match_path(&path, &mut params) {
            Some(method_router) => match method_router.route_for(req.method()) {
                Some(route) => Outcome::Route(route),
                None => Outcome::MethodNotAllowed(method_router.allow_header_value()),
            },
            None => Outcome::NotFound,
        };

        match outcome {
            Outcome::Route(route) => {
                let mut req = req;
                req.extensions_mut().insert(Params::from_pairs(params));
                route(req)
            }
            Outcome::MethodNotAllowed(allow) => {
                Box::pin(async move { method_not_allowed_response(allow) })
            }
            Outcome::NotFound => (self.fallback)(req),
        }
    }
}

fn default_not_found_route() -> Route {
    Arc::new(|_req: Request| -> BoxFuture<Response> { Box::pin(async { not_found_response() }) })
}

/// Build the standard `404 Not Found` response.
#[must_use]
pub fn not_found_response() -> Response {
    (StatusCode::NOT_FOUND, "Not Found").into_response()
}

fn method_not_allowed_response(allow: HeaderValue) -> Response {
    let mut response = (StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed").into_response();
    response.headers_mut().insert(header::ALLOW, allow);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::{get, post};
    use http_body_util::BodyExt as _;
    use oxide_web_core::{Body, Method};

    async fn body_of(response: Response) -> (StatusCode, String) {
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    fn request(method: Method, path: &str) -> Request {
        Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn routes_get_request() {
        async fn index() -> &'static str {
            "index"
        }
        let service = Router::new().route("/", get(index)).into_service().unwrap();

        let (status, body) = body_of(service.call(request(Method::GET, "/")).await).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "index");
    }

    #[tokio::test]
    async fn unknown_path_is_404() {
        let service = Router::new().into_service().unwrap();
        let (status, _) = body_of(service.call(request(Method::GET, "/nope")).await).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wrong_method_is_405_with_allow() {
        async fn index() -> &'static str {
            "index"
        }
        let service = Router::new().route("/", get(index)).into_service().unwrap();

        let response = service.call(request(Method::POST, "/")).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers().get(header::ALLOW).unwrap(), "GET");
    }

    #[tokio::test]
    async fn merges_methods_across_route_calls() {
        async fn a() -> &'static str {
            "a"
        }
        async fn b() -> &'static str {
            "b"
        }
        let service = Router::new()
            .route("/x", get(a))
            .route("/x", post(b))
            .into_service()
            .unwrap();

        let (_, ga) = body_of(service.call(request(Method::GET, "/x")).await).await;
        let (_, pb) = body_of(service.call(request(Method::POST, "/x")).await).await;
        assert_eq!(ga, "a");
        assert_eq!(pb, "b");
    }

    #[tokio::test]
    async fn duplicate_method_is_registration_error() {
        async fn a() -> &'static str {
            "a"
        }
        async fn b() -> &'static str {
            "b"
        }
        let result = Router::new()
            .route("/x", get(a))
            .route("/x", get(b))
            .into_service();
        assert!(matches!(result, Err(RouteError::Conflict { .. })));
    }

    #[tokio::test]
    async fn custom_fallback_is_used() {
        async fn fallback() -> (StatusCode, &'static str) {
            (StatusCode::NOT_FOUND, "custom missing")
        }
        let service = Router::new().fallback(fallback).into_service().unwrap();
        let (status, body) = body_of(service.call(request(Method::GET, "/anything")).await).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "custom missing");
    }

    #[tokio::test]
    async fn params_available_in_extensions() {
        async fn show(req: Request) -> String {
            let params = req.extensions().get::<Params>().unwrap();
            params.get("id").unwrap_or("?").to_owned()
        }
        let service = Router::new()
            .route("/users/:id", get(show))
            .into_service()
            .unwrap();
        let (_, body) = body_of(service.call(request(Method::GET, "/users/99")).await).await;
        assert_eq!(body, "99");
    }
}
