//! Response conversion: the [`IntoResponse`] trait and its implementations.

use std::borrow::Cow;

use http::header::{self, HeaderValue};

use crate::body::Body;
use crate::{Response, StatusCode};

/// The owned parts (status, version, headers, extensions) of a [`Response`].
///
/// Re-exported so downstream crates can name the type without depending on
/// `http` directly.
pub type ResponseParts = http::response::Parts;

/// Convert a value into an HTTP [`Response`].
///
/// This is the seam through which every handler return type flows. Implement it
/// for your own types to control exactly how they are rendered.
///
/// # Examples
///
/// ```
/// use oxide_web_core::{IntoResponse, StatusCode};
///
/// let response = "hello".into_response();
/// assert_eq!(response.status(), StatusCode::OK);
///
/// let response = (StatusCode::CREATED, "made it").into_response();
/// assert_eq!(response.status(), StatusCode::CREATED);
/// ```
pub trait IntoResponse {
    /// Perform the conversion.
    fn into_response(self) -> Response;
}

/// Apply a value to a set of response parts (status, headers, …) without
/// producing a full body.
///
/// This is the counterpart to [`IntoResponse`] used when composing responses
/// from tuples. Phase 1 provides the trait and the header/status building blocks;
/// richer composition lands in Phase 2.
pub trait IntoResponseParts {
    /// The error produced if the parts cannot be applied.
    type Error: IntoResponse;

    /// Apply `self` to `parts`.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] if the value cannot be applied (for example an
    /// invalid header value).
    fn into_response_parts(self, parts: &mut ResponseParts) -> Result<(), Self::Error>;
}

fn text_response(body: Body) -> Response {
    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

impl IntoResponse for Body {
    fn into_response(self) -> Response {
        Response::new(self)
    }
}

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Response::new(Body::empty())
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = self;
        response
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        text_response(Body::from(self))
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        text_response(Body::from(self))
    }
}

impl IntoResponse for Cow<'static, str> {
    fn into_response(self) -> Response {
        match self {
            Cow::Borrowed(s) => s.into_response(),
            Cow::Owned(s) => s.into_response(),
        }
    }
}

impl<T> IntoResponse for (StatusCode, T)
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let (status, body) = self;
        let mut response = body.into_response();
        *response.status_mut() = status;
        response
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

impl<T> IntoResponse for Option<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Some(value) => value.into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt as _;

    async fn body_string(response: Response) -> String {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn str_sets_text_content_type_and_body() {
        let response = "hi".into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(body_string(response).await, "hi");
    }

    #[tokio::test]
    async fn status_tuple_overrides_status() {
        let response = (StatusCode::CREATED, "made".to_owned()).into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(body_string(response).await, "made");
    }

    #[test]
    fn bare_status_has_empty_body() {
        let response = StatusCode::NO_CONTENT.into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn result_renders_ok_and_err_arms() {
        let ok: Result<&str, StatusCode> = Ok("good");
        let err: Result<&str, StatusCode> = Err(StatusCode::BAD_REQUEST);
        assert_eq!(ok.into_response().status(), StatusCode::OK);
        assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
    }
}
