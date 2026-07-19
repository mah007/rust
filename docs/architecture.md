# oxide-web architecture

This document records the design of **oxide-web**, an async HTTP server and web
framework, and the decisions taken while building it. It is updated as each
phase lands. See [roadmap.md](./roadmap.md) for the phase plan.

## Goals

- A production-grade asynchronous HTTP server built on Tokio + Hyper.
- An ergonomic, Axum-style framework layered on top of that server, with its own
  original public API.
- Safe Rust (`#![forbid(unsafe_code)]`), well tested, documented, and modular so
  later phases (extractors, middleware, TLS, WebSockets, …) slot in without
  rewriting the core.

## Crate layout

The workspace is split only where a crate has a genuinely distinct
responsibility. Crates that would be empty today (`oxide-web-middleware`,
`oxide-web-macros`) are intentionally **not** created yet; they arrive with the
phase that fills them, to avoid placeholder scaffolding.

```
oxide-web-core     server lifecycle, body/request/response types, IntoResponse,
                   Handler conversion, graceful shutdown, shutdown signals
      ▲
oxide-web-router   route matching (segment trie), MethodRouter, method dispatch,
                   404 / 405, conflict detection, param & wildcard capture
      ▲
oxide-web          facade: the Application builder that ties router + server
                   together, plus curated public re-exports
      ▲
oxide-web-testing  in-process test client on an ephemeral port + assertions
```

Dependency direction is strictly one-way (core ← router ← facade ← testing), so
the low-level HTTP plumbing never depends on the higher-level ergonomics.

### Why `Application` lives in the facade, not core

`core` owns the request/response machinery and the server loop; `router` owns
matching. `Application` composes a `Router` with server configuration and the
serve loop, so it belongs above both — in the `oxide-web` facade, which is the
only crate that depends on both `core` and `router`.

## Key type decisions

### Body

`oxide_web_core::Body` is an enum wrapper (`Empty` / `Full(Bytes)` /
`Boxed(BoxBody)`) that implements `http_body::Body<Data = Bytes, Error = BoxError>`.
Common cases (empty, fully-buffered bytes) avoid an allocation/boxing; the
`Boxed` variant is the extension point for streaming/SSE bodies in later phases.

Every variant is `Unpin`, so `Body` is `Unpin` and `poll_frame` is implemented
with `Pin::get_mut` — **no `unsafe` and no `pin-project` needed**, which keeps
`#![forbid(unsafe_code)]` intact.

### Request / Response

We reuse the `http` crate's types via aliases: `Request<B = Body>` and
`Response<B = Body>` are `http::Request` / `http::Response`. This means users get
the standard, familiar `http` API and we re-export the pieces (`StatusCode`,
`HeaderMap`, `Method`, `Uri`, …) rather than inventing parallel types.

### IntoResponse

Handlers return anything implementing `IntoResponse`. Phase 1 covers
`&'static str`, `String`, `StatusCode`, `()`, `Response`, `Cow<str>`, and
`(StatusCode, R)`. Text types set `Content-Type: text/plain; charset=utf-8`.
The trait is the seam through which JSON/HTML/redirect responses (Phase 2) and
error rendering (Phase 3) are added without touching handlers.

### Handler

`Handler<T>` converts an `async fn` into a callable that produces a
`Response`. Phase 1 supports zero-argument handlers (`async fn() -> impl IntoResponse`)
and single-`Request` handlers (`async fn(Request) -> impl IntoResponse`). The
`T` type parameter is a marker distinguishing the argument shape; it is the hook
that lets Phase 3 add tuples of extractors (`Path`, `Query`, `Json`, `State`, …)
by implementing `Handler` for more argument arities.

Extraction is deliberately split conceptually into **request-parts** extraction
(headers, method, path params — non-consuming) and **body-consuming** extraction
(`Json`, `Bytes`, `String`). This boundary is documented now and enforced by the
extractor trait design in Phase 3 so two extractors can never consume the body
twice.

### Erased route handler

The router stores handlers type-erased as
`Route = Arc<dyn Fn(Request) -> BoxFuture<Response> + Send + Sync>`. Arc makes a
route cheap to share across every connection and request; the boxed future keeps
the router non-generic over handler types.

## Routing

`oxide-web-router` implements a **path-segment trie** (a radix tree keyed on
`/`-delimited segments) — explicitly *not* a linear scan, per the spec.

Each node holds:

- `static_children: HashMap<segment, Node>` — exact-match segments,
- one optional **param** child (`:name`) matching any single segment,
- one optional **wildcard** child (`*name`) matching the remaining path.

Matching is recursive with backtracking and tries children in priority order
**static → param → wildcard**, so `/users/me` beats `/users/:id` beats
`/users/*rest`. Lookup cost is O(number of path segments), independent of the
number of registered routes.

- **Conflict detection** happens at registration: registering the same
  method+path twice, or two differently-named params at the same position, is a
  hard error surfaced at startup (not at request time).
- **Params & wildcards** are captured into a small `Params` map available to
  extractors (used from Phase 3; matching is built now to avoid a later rewrite).
- **Method dispatch**: a matched path resolves to a `MethodRouter`. A missing
  method yields `405 Method Not Allowed` with an `Allow` header; an unmatched
  path yields `404 Not Found` (or the app fallback).
- **Trailing slash**: matched exactly — `/foo` and `/foo/` are distinct routes.
  This is intentional and documented; opt-in redirect/normalization is a later
  enhancement.

## Server

`oxide-web-core` runs the accept loop on a Tokio `TcpListener` and serves each
connection with `hyper_util`'s automatic HTTP/1 + HTTP/2 connection builder
(`server::conn::auto`). We never hand-roll HTTP parsing — Hyper owns that.

- **Peer address** is captured at accept time and injected into request
  extensions so handlers/extractors can read the remote `SocketAddr`.
- **Tracing**: each request runs inside a span carrying method, path, matched
  status, and duration. Sensitive headers are never logged.
- **Graceful shutdown**: a user-supplied shutdown future (e.g.
  `shutdown::ctrl_c()`, which also listens for `SIGTERM` on Unix) stops the
  accept loop; in-flight connections drain via `hyper_util`'s
  `GracefulShutdown`, bounded by a configurable timeout after which remaining
  connections are dropped.
- **`ServerConfig`** carries bind addresses, timeouts, body-size limit, TCP
  nodelay, keep-alive, and HTTP/2 toggle, with sensible defaults. Environment
  overrides land in Phase 5.

## Errors

Network-level failures (accept errors, per-connection errors) are logged and
never panic the server. The typed application error model and JSON error
envelope (`{ "error": { code, message, request_id } }`) land in Phase 3; the
`IntoResponse` seam and the boxed-error type (`BoxError`) are in place now to
carry them.

## Safety & security posture (Phase 1)

- `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]` on every crate.
- No custom HTTP parser (Hyper) and no custom crypto (Rustls arrives feature-
  gated in Phase 5).
- Server does not panic on normal network errors.
- Forwarded/`X-Forwarded-*` headers are **not** trusted (trusted-proxy config is
  Phase 5); the peer address is always the real transport peer.
- Full threat-model coverage (Slowloris, body limits, path traversal, header
  abuse, log injection, secret leakage, …) is tracked in
  [security.md](./security.md) and implemented across Phases 3–7.

## What Phase 1 deliberately excludes

Extractors, the typed error envelope, the middleware stack, JSON/HTML/redirect
responses, TLS, static files, metrics, SSE, and WebSockets. Each has a dedicated
later phase. Phase 1 proves the vertical slice end to end: listener → server →
router → handler → response → graceful shutdown, with tests and a runnable
example.
