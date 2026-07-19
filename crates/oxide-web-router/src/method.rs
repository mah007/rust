//! Method dispatch: [`MethodRouter`] and the `get`/`post`/… constructors.

use std::collections::HashMap;

use oxide_web_core::{Handler, HeaderValue, Method, Route};

/// A set of handlers for a single path, keyed by HTTP method.
///
/// Build one with the free functions [`get`], [`post`], … and chain additional
/// methods:
///
/// ```
/// use oxide_web_router::routing::get;
///
/// async fn list() -> &'static str { "list" }
/// async fn create() -> &'static str { "created" }
///
/// let method_router = get(list).post(create);
/// ```
#[derive(Clone, Default)]
pub struct MethodRouter {
    handlers: HashMap<Method, Route>,
}

impl MethodRouter {
    /// Create an empty method router.
    #[must_use]
    pub fn new() -> Self {
        MethodRouter {
            handlers: HashMap::new(),
        }
    }

    /// Register `handler` for an arbitrary `method`.
    ///
    /// If the method is already set, the previous handler is replaced.
    #[must_use]
    pub fn on<H, T>(mut self, method: Method, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.handlers.insert(method, handler.into_route());
        self
    }

    /// Register a handler for `GET`.
    #[must_use]
    pub fn get<H, T>(self, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.on(Method::GET, handler)
    }

    /// Register a handler for `POST`.
    #[must_use]
    pub fn post<H, T>(self, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.on(Method::POST, handler)
    }

    /// Register a handler for `PUT`.
    #[must_use]
    pub fn put<H, T>(self, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.on(Method::PUT, handler)
    }

    /// Register a handler for `PATCH`.
    #[must_use]
    pub fn patch<H, T>(self, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.on(Method::PATCH, handler)
    }

    /// Register a handler for `DELETE`.
    #[must_use]
    pub fn delete<H, T>(self, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.on(Method::DELETE, handler)
    }

    /// Register a handler for `HEAD`.
    #[must_use]
    pub fn head<H, T>(self, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.on(Method::HEAD, handler)
    }

    /// Register a handler for `OPTIONS`.
    #[must_use]
    pub fn options<H, T>(self, handler: H) -> Self
    where
        H: Handler<T>,
    {
        self.on(Method::OPTIONS, handler)
    }

    /// Look up the handler for `method`, if one is registered.
    pub(crate) fn route_for(&self, method: &Method) -> Option<Route> {
        self.handlers.get(method).cloned()
    }

    /// Merge another method router into this one.
    ///
    /// Returns the first method that is defined in both, indicating a conflict.
    pub(crate) fn merge(&mut self, other: MethodRouter) -> Result<(), Method> {
        for (method, route) in other.handlers {
            if self.handlers.contains_key(&method) {
                return Err(method);
            }
            self.handlers.insert(method, route);
        }
        Ok(())
    }

    /// Build the `Allow` header value listing the registered methods (sorted for
    /// determinism), for use in `405 Method Not Allowed` responses.
    pub(crate) fn allow_header_value(&self) -> HeaderValue {
        let mut methods: Vec<&str> = self.handlers.keys().map(Method::as_str).collect();
        methods.sort_unstable();
        // Method names are always valid header-value tokens.
        HeaderValue::from_str(&methods.join(", ")).unwrap_or_else(|_| HeaderValue::from_static(""))
    }
}

/// Route `GET` requests to `handler`.
#[must_use]
pub fn get<H, T>(handler: H) -> MethodRouter
where
    H: Handler<T>,
{
    MethodRouter::new().get(handler)
}

/// Route `POST` requests to `handler`.
#[must_use]
pub fn post<H, T>(handler: H) -> MethodRouter
where
    H: Handler<T>,
{
    MethodRouter::new().post(handler)
}

/// Route `PUT` requests to `handler`.
#[must_use]
pub fn put<H, T>(handler: H) -> MethodRouter
where
    H: Handler<T>,
{
    MethodRouter::new().put(handler)
}

/// Route `PATCH` requests to `handler`.
#[must_use]
pub fn patch<H, T>(handler: H) -> MethodRouter
where
    H: Handler<T>,
{
    MethodRouter::new().patch(handler)
}

/// Route `DELETE` requests to `handler`.
#[must_use]
pub fn delete<H, T>(handler: H) -> MethodRouter
where
    H: Handler<T>,
{
    MethodRouter::new().delete(handler)
}

/// Route `HEAD` requests to `handler`.
#[must_use]
pub fn head<H, T>(handler: H) -> MethodRouter
where
    H: Handler<T>,
{
    MethodRouter::new().head(handler)
}

/// Route `OPTIONS` requests to `handler`.
#[must_use]
pub fn options<H, T>(handler: H) -> MethodRouter
where
    H: Handler<T>,
{
    MethodRouter::new().options(handler)
}

/// Route requests with an arbitrary `method` to `handler`.
#[must_use]
pub fn on<H, T>(method: Method, handler: H) -> MethodRouter
where
    H: Handler<T>,
{
    MethodRouter::new().on(method, handler)
}
