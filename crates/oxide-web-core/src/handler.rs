//! The [`Handler`] trait and the type-erased [`Route`] it produces.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::response::IntoResponse;
use crate::{Request, Response};

/// A boxed, `Send` future — the return type of erased asynchronous work.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// A type-erased, cheaply-cloneable handler stored by the router.
///
/// Every registered route is reduced to this shape so the router can hold
/// handlers of many different concrete types uniformly. Cloning a `Route` is an
/// `Arc` clone, so routes are cheap to share across every connection.
pub type Route = Arc<dyn Fn(Request) -> BoxFuture<Response> + Send + Sync>;

mod markers {
    //! Zero-sized marker types that distinguish the [`super::Handler`]
    //! implementations by argument shape. They are an implementation detail and
    //! are never named by users.

    /// Marker for `async fn() -> impl IntoResponse`.
    #[doc(hidden)]
    #[derive(Debug, Clone, Copy)]
    pub struct NoArgs;

    /// Marker for `async fn(Request) -> impl IntoResponse`.
    #[doc(hidden)]
    #[derive(Debug, Clone, Copy)]
    pub struct WithRequest;
}

#[doc(hidden)]
pub use markers::{NoArgs, WithRequest};

/// Convert an `async fn` (or closure) into something the router can call.
///
/// The `T` type parameter is an internal marker describing the handler's
/// argument shape; callers never name it (it is inferred). This is the extension
/// point through which later phases add extractor-taking handlers
/// (`async fn(Path<u64>, Json<T>) -> ...`) by implementing `Handler` for more
/// argument arities.
///
/// Phase 1 supports two shapes:
///
/// - `async fn() -> impl IntoResponse`
/// - `async fn(Request) -> impl IntoResponse`
pub trait Handler<T>: Clone + Send + Sync + 'static {
    /// Call the handler, producing a response future.
    fn call(self, req: Request) -> BoxFuture<Response>;

    /// Erase this handler into a shareable [`Route`].
    #[must_use]
    fn into_route(self) -> Route
    where
        Self: Sized,
    {
        Arc::new(move |req| self.clone().call(req))
    }
}

impl<F, Fut, R> Handler<NoArgs> for F
where
    F: Fn() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
{
    fn call(self, _req: Request) -> BoxFuture<Response> {
        Box::pin(async move { self().await.into_response() })
    }
}

impl<F, Fut, R> Handler<WithRequest> for F
where
    F: Fn(Request) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
{
    fn call(self, req: Request) -> BoxFuture<Response> {
        Box::pin(async move { self(req).await.into_response() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Body;
    use crate::{RemoteAddr, StatusCode};
    use http_body_util::BodyExt as _;
    use std::net::SocketAddr;

    async fn body_string(response: Response) -> String {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn no_arg_handler_runs() {
        async fn hello() -> &'static str {
            "hello"
        }
        let route = hello.into_route();
        let req = Request::new(Body::empty());
        let response = route(req).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_string(response).await, "hello");
    }

    #[tokio::test]
    async fn request_handler_can_read_extensions() {
        async fn whoami(req: Request) -> String {
            match req.extensions().get::<RemoteAddr>() {
                Some(addr) => format!("peer={addr}"),
                None => "peer=unknown".to_owned(),
            }
        }
        let route = whoami.into_route();
        let mut req = Request::new(Body::empty());
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        req.extensions_mut().insert(RemoteAddr(addr));
        let response = route(req).await;
        assert_eq!(body_string(response).await, "peer=127.0.0.1:9000");
    }
}
