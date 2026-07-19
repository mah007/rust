//! The [`TestResponse`] type and its chainable assertions.

use bytes::Bytes;
use oxide_web_core::{HeaderMap, StatusCode};

/// A fully-buffered HTTP response captured by the test harness.
///
/// The assertion methods consume and return `self`, so they can be chained:
///
/// ```no_run
/// # async fn demo(server: oxide_web_testing::TestServer) {
/// server
///     .get("/health")
///     .await
///     .assert_ok()
///     .assert_text("OK");
/// # }
/// ```
///
/// Assertions panic with a descriptive message on failure, which is the
/// expected behavior inside a `#[test]`.
#[derive(Debug, Clone)]
pub struct TestResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

impl TestResponse {
    pub(crate) fn new(status: StatusCode, headers: HeaderMap, body: Bytes) -> Self {
        TestResponse {
            status,
            headers,
            body,
        }
    }

    /// The response status code.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// The response headers.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// The raw response body bytes.
    #[must_use]
    pub fn body_bytes(&self) -> &[u8] {
        &self.body
    }

    /// The response body decoded as UTF-8 (lossily).
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The first value of header `name`, decoded as UTF-8, if present.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    /// Assert the status equals `expected`.
    ///
    /// # Panics
    ///
    /// Panics if the status differs.
    pub fn assert_status(self, expected: StatusCode) -> Self {
        assert_eq!(
            self.status,
            expected,
            "expected status {expected}, got {} with body {:?}",
            self.status,
            self.text()
        );
        self
    }

    /// Assert the status is `200 OK`.
    ///
    /// # Panics
    ///
    /// Panics if the status is not `200`.
    pub fn assert_ok(self) -> Self {
        self.assert_status(StatusCode::OK)
    }

    /// Assert the status is `404 Not Found`.
    ///
    /// # Panics
    ///
    /// Panics if the status is not `404`.
    pub fn assert_not_found(self) -> Self {
        self.assert_status(StatusCode::NOT_FOUND)
    }

    /// Assert the body equals `expected` exactly (as text).
    ///
    /// # Panics
    ///
    /// Panics if the body differs.
    pub fn assert_text(self, expected: &str) -> Self {
        assert_eq!(self.text(), expected, "unexpected response body");
        self
    }

    /// Assert the body contains `needle` (as text).
    ///
    /// # Panics
    ///
    /// Panics if the substring is absent.
    pub fn assert_body_contains(self, needle: &str) -> Self {
        let text = self.text();
        assert!(
            text.contains(needle),
            "expected body to contain {needle:?}, body was {text:?}"
        );
        self
    }

    /// Assert header `name` is present and equals `value`.
    ///
    /// # Panics
    ///
    /// Panics if the header is missing or differs.
    pub fn assert_header(self, name: &str, value: &str) -> Self {
        match self.header(name) {
            Some(actual) => assert_eq!(actual, value, "unexpected value for header `{name}`"),
            None => panic!("expected header `{name}` to be present"),
        }
        self
    }

    /// Assert header `name` is present (any value).
    ///
    /// # Panics
    ///
    /// Panics if the header is missing.
    pub fn assert_header_present(self, name: &str) -> Self {
        assert!(
            self.headers.contains_key(name),
            "expected header `{name}` to be present"
        );
        self
    }
}
