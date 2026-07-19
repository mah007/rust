//! The framework [`Body`] type.

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use http_body_util::BodyExt;
use http_body_util::combinators::UnsyncBoxBody;

use crate::error::BoxError;

/// A request or response body.
///
/// `Body` covers the three cases the framework needs while keeping the common
/// ones allocation-free:
///
/// - **empty** — no payload at all (e.g. a bare `204 No Content`);
/// - **full** — a single already-buffered [`Bytes`] chunk (plain text, JSON, …);
/// - **boxed** — an arbitrary streaming [`http_body::Body`], used for incoming
///   request bodies and (from later phases) streaming/SSE responses.
///
/// Every representation is [`Unpin`], so the [`http_body::Body`] implementation
/// needs neither `unsafe` nor `pin-project`.
#[derive(Debug)]
pub struct Body(BodyInner);

enum BodyInner {
    Empty,
    Full(Bytes),
    Boxed(UnsyncBoxBody<Bytes, BoxError>),
}

impl std::fmt::Debug for BodyInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BodyInner::Empty => f.write_str("Empty"),
            BodyInner::Full(bytes) => f.debug_tuple("Full").field(&bytes.len()).finish(),
            BodyInner::Boxed(_) => f.write_str("Boxed(..)"),
        }
    }
}

impl Body {
    /// Create an empty body.
    #[must_use]
    pub fn empty() -> Self {
        Body(BodyInner::Empty)
    }

    /// Create a body from an already-buffered chunk of bytes.
    ///
    /// An empty input is normalized to [`Body::empty`].
    #[must_use]
    pub fn from_bytes(bytes: impl Into<Bytes>) -> Self {
        let bytes = bytes.into();
        if bytes.is_empty() {
            Self::empty()
        } else {
            Body(BodyInner::Full(bytes))
        }
    }

    /// Wrap an arbitrary [`http_body::Body`] as a streaming body.
    ///
    /// The source body's error type is erased into [`BoxError`].
    #[must_use]
    pub fn new<B>(body: B) -> Self
    where
        B: HttpBody<Data = Bytes> + Send + 'static,
        B::Error: Into<BoxError>,
    {
        Body(BodyInner::Boxed(body.map_err(Into::into).boxed_unsync()))
    }

    /// Wrap an incoming Hyper request body.
    #[must_use]
    pub fn from_incoming(body: hyper::body::Incoming) -> Self {
        Self::new(body)
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<()> for Body {
    fn from((): ()) -> Self {
        Self::empty()
    }
}

impl From<Bytes> for Body {
    fn from(bytes: Bytes) -> Self {
        Self::from_bytes(bytes)
    }
}

impl From<Vec<u8>> for Body {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from_bytes(bytes)
    }
}

impl From<&'static [u8]> for Body {
    fn from(bytes: &'static [u8]) -> Self {
        Self::from_bytes(Bytes::from_static(bytes))
    }
}

impl From<String> for Body {
    fn from(text: String) -> Self {
        Self::from_bytes(text.into_bytes())
    }
}

impl From<&'static str> for Body {
    fn from(text: &'static str) -> Self {
        Self::from_bytes(Bytes::from_static(text.as_bytes()))
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // `Body` is `Unpin`, so obtaining a `&mut Self` from the pin is safe and
        // requires no `unsafe`.
        let this = self.get_mut();
        match &mut this.0 {
            BodyInner::Empty => Poll::Ready(None),
            BodyInner::Full(bytes) => {
                if bytes.is_empty() {
                    Poll::Ready(None)
                } else {
                    let chunk = std::mem::take(bytes);
                    Poll::Ready(Some(Ok(Frame::data(chunk))))
                }
            }
            BodyInner::Boxed(body) => Pin::new(body).poll_frame(cx),
        }
    }

    fn is_end_stream(&self) -> bool {
        match &self.0 {
            BodyInner::Empty => true,
            BodyInner::Full(bytes) => bytes.is_empty(),
            BodyInner::Boxed(body) => body.is_end_stream(),
        }
    }

    fn size_hint(&self) -> SizeHint {
        match &self.0 {
            BodyInner::Empty => SizeHint::with_exact(0),
            BodyInner::Full(bytes) => SizeHint::with_exact(bytes.len() as u64),
            BodyInner::Boxed(body) => body.size_hint(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_body_yields_no_frames() {
        let body = Body::empty();
        assert!(body.is_end_stream());
        let collected = body.collect().await.unwrap().to_bytes();
        assert!(collected.is_empty());
    }

    #[tokio::test]
    async fn full_body_round_trips() {
        let body = Body::from("hello");
        assert_eq!(body.size_hint().exact(), Some(5));
        let collected = body.collect().await.unwrap().to_bytes();
        assert_eq!(&collected[..], b"hello");
    }

    #[tokio::test]
    async fn empty_string_normalizes_to_empty() {
        let body = Body::from_bytes(Vec::new());
        assert!(body.is_end_stream());
    }
}
