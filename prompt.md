# Build a Production-Grade Rust Web Server and Web Framework

You are working as a senior Rust systems engineer and framework architect.

Create a new Rust project that provides:

1. A high-performance asynchronous HTTP web server.
2. A reusable ergonomic web framework built on top of that server.
3. Middleware, routing, request extraction, response conversion, application state, configuration, logging, graceful shutdown, testing, and documentation.
4. Example applications demonstrating how developers use the framework.

The project must be production-oriented, modular, well-tested, documented, and designed for future expansion.

## Important Working Rules

* Work directly in the current project directory.
* Before modifying anything, inspect the current directory and existing files.
* Do not overwrite unrelated files.
* Use the latest stable Rust toolchain available through `rustup`.
* Use Rust 2024 edition unless a dependency compatibility issue requires otherwise.
* Use current stable, compatible crate versions rather than guessing versions.
* Record the resolved toolchain and important dependency versions in the README.
* Prefer safe Rust.
* Any use of `unsafe` requires:

  * A clear technical justification.
  * A documented safety invariant.
  * Dedicated tests.
* Do not implement custom TLS cryptography.
* Do not manually implement a complete HTTP parser when Hyper already provides a safe and robust HTTP implementation.
* Build the framework on top of Tokio, Hyper, the `http` crate, and Tower-compatible service abstractions where appropriate.
* Axum may be studied as an architectural reference, but the final framework must have its own public API and implementation.
* Run formatting, linting, tests, examples, and documentation checks after each major phase.
* Do not stop after creating empty folders or placeholder files.
* Implement working vertical slices.

## Initial Environment Inspection

Start by checking:

```bash
pwd
ls -la
find . -maxdepth 2 -type f | sort | head -200
rustc --version
cargo --version
rustup show
git status
```

Install or activate the stable Rust toolchain when needed:

```bash
rustup update stable
rustup default stable
rustup component add rustfmt clippy
```

Do not install system-wide packages unless they are genuinely required. Explain any required system package before installing it.

## Project Working Name

Use `oxide-web` as the temporary workspace name.

Before considering publication, verify whether the name conflicts with an existing project or crate. The internal Rust package names may use underscores where required, such as `oxide_web`.

## Architectural Goal

The framework should eventually allow application developers to write code similar to:

```rust
use oxide_web::{
    extract::{Json, Path, State},
    middleware,
    routing::{delete, get, post, put},
    Application, HttpError, HttpResult,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    service_name: String,
}

#[derive(Debug, Deserialize)]
struct CreateUser {
    name: String,
    email: String,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    id: u64,
    name: String,
    email: String,
}

async fn health() -> &'static str {
    "OK"
}

async fn get_user(
    Path(user_id): Path<u64>,
    State(state): State<Arc<AppState>>,
) -> HttpResult<Json<UserResponse>> {
    let user = UserResponse {
        id: user_id,
        name: format!("User from {}", state.service_name),
        email: "user@example.com".to_string(),
    };

    Ok(Json(user))
}

async fn create_user(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<CreateUser>,
) -> HttpResult<(http::StatusCode, Json<UserResponse>)> {
    let user = UserResponse {
        id: 1,
        name: payload.name,
        email: payload.email,
    };

    Ok((http::StatusCode::CREATED, Json(user)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState {
        service_name: "Example API".to_string(),
    });

    let app = Application::new()
        .route("/health", get(health))
        .route("/users/:id", get(get_user))
        .route("/users", post(create_user))
        .with_state(state)
        .layer(middleware::request_id())
        .layer(middleware::tracing())
        .layer(middleware::catch_panic());

    app.bind("0.0.0.0:8080")
        .graceful_shutdown(oxide_web::shutdown::ctrl_c())
        .run()
        .await?;

    Ok(())
}
```

The exact API may evolve, but it must remain clear, type-safe, discoverable, and ergonomic.

# Workspace Structure

Create a Cargo workspace with a structure similar to:

```text
oxide-web/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
├── deny.toml
├── .gitignore
├── .editorconfig
├── README.md
├── LICENSE-APACHE
├── LICENSE-MIT
├── CHANGELOG.md
├── CONTRIBUTING.md
├── SECURITY.md
├── Makefile
├── justfile
├── Dockerfile
├── docker-compose.yml
├── crates/
│   ├── oxide-web/
│   ├── oxide-web-core/
│   ├── oxide-web-router/
│   ├── oxide-web-middleware/
│   ├── oxide-web-macros/
│   └── oxide-web-testing/
├── examples/
│   ├── hello-world/
│   ├── rest-api/
│   ├── middleware-demo/
│   └── websocket-chat/
├── benches/
├── tests/
│   ├── integration/
│   └── fixtures/
├── docs/
│   ├── architecture.md
│   ├── routing.md
│   ├── extractors.md
│   ├── middleware.md
│   ├── errors.md
│   ├── configuration.md
│   ├── security.md
│   └── roadmap.md
└── .github/
    └── workflows/
        ├── ci.yml
        ├── security.yml
        └── release.yml
```

Do not create unnecessary crates merely to match the proposed tree. Every crate must have a clear responsibility. Merge crates when splitting them would only add complexity.

## Suggested Crate Responsibilities

### `oxide-web-core`

Responsible for:

* HTTP server lifecycle.
* Tokio listener management.
* Hyper connection handling.
* HTTP/1.1 support.
* HTTP/2 support when compatible with the selected stack.
* Connection configuration.
* Graceful shutdown.
* Request and response body types.
* Framework service abstractions.
* Server errors.
* Remote socket address propagation.
* Connection metadata.
* Timeouts and connection limits.

### `oxide-web-router`

Responsible for:

* Static routes.
* Named parameters such as `/users/:id`.
* Wildcard routes such as `/assets/*path`.
* HTTP method dispatch.
* Nested routers.
* Route groups and prefixes.
* Route-level middleware.
* `404 Not Found`.
* `405 Method Not Allowed`.
* Automatic `HEAD` handling for `GET` routes where appropriate.
* `OPTIONS` support where appropriate.
* Route conflict detection.
* Deterministic route matching.

Use an efficient routing structure such as a radix tree or segment tree. Do not begin with a linear scan that becomes the permanent implementation.

### `oxide-web-middleware`

Responsible for reusable middleware including:

* Request ID.
* Structured request tracing.
* Access logging.
* Panic catching.
* Request timeout.
* Response compression.
* CORS.
* Body size limit.
* Concurrency limit.
* Rate limiting interface.
* Security headers.
* Authentication extension points.
* Sensitive-header redaction.

Where practical, provide compatibility with Tower’s `Layer` and `Service` abstractions.

### `oxide-web-macros`

Initially optional.

Potential future responsibilities:

* Route attribute macros.
* Typed parameter derivation.
* Error response derivation.
* OpenAPI metadata derivation.

Do not make procedural macros mandatory for normal framework usage. The framework must remain usable with ordinary functions and builders.

### `oxide-web-testing`

Responsible for:

* In-process application testing.
* Request builders.
* Response body collection.
* JSON response assertions.
* Header assertions.
* Status assertions.
* Test application startup on an ephemeral port.
* Optional black-box HTTP testing helpers.

# Functional Requirements

## 1. HTTP Server

Implement:

* Asynchronous TCP listener using Tokio.
* Hyper-based HTTP handling.
* Configurable bind address.
* Multiple bind addresses where reasonably supported.
* HTTP/1.1 keep-alive.
* HTTP/2 support when supported by the chosen integration.
* Configurable connection timeout.
* Configurable request timeout.
* Configurable header limits.
* Configurable request-body limit.
* Graceful shutdown.
* Connection draining during shutdown.
* Proper error propagation.
* Structured server startup and shutdown events.
* Remote client address available to handlers.
* No panics for normal network failures.

Create a server configuration object such as:

```rust
pub struct ServerConfig {
    pub bind_addresses: Vec<std::net::SocketAddr>,
    pub request_timeout: std::time::Duration,
    pub graceful_shutdown_timeout: std::time::Duration,
    pub max_request_body_size: usize,
    pub tcp_nodelay: bool,
    pub http1_keep_alive: bool,
    pub http2_enabled: bool,
}
```

Provide sensible defaults and environment-variable overrides.

## 2. Application Builder

Create a central application API similar to:

```rust
let app = Application::new()
    .route("/", get(index))
    .route("/users", post(create_user))
    .nest("/api/v1", api_router)
    .fallback(not_found)
    .with_state(state)
    .layer(tracing_layer);
```

It must support:

* Global state.
* Nested routers.
* Global middleware.
* Route-specific middleware.
* A fallback handler.
* Application configuration.
* Extension insertion.
* Server startup.
* Graceful shutdown.

## 3. Router

Support:

```text
/
/users
/users/:id
/users/:user_id/orders/:order_id
/assets/*path
/api/v1/*
```

Required behavior:

* Static segments take priority over parameters.
* Parameters take priority over wildcards.
* Method matching must be explicit.
* Duplicate or ambiguous routes must produce useful startup errors.
* Invalid parameter syntax must be rejected.
* Percent-decoding errors must produce a controlled client error.
* Route parameters must be available to extractors.
* Trailing-slash behavior must be explicitly defined and documented.
* Route matching must be thoroughly tested.

Support common HTTP methods:

* GET
* POST
* PUT
* PATCH
* DELETE
* HEAD
* OPTIONS
* CONNECT
* TRACE

## 4. Handler System

Design a handler abstraction capable of converting asynchronous Rust functions into framework services.

Support handler return values such as:

```rust
async fn handler() -> &'static str
async fn handler() -> String
async fn handler() -> StatusCode
async fn handler() -> Response
async fn handler() -> Json<T>
async fn handler() -> Result<T, E>
async fn handler() -> (StatusCode, T)
async fn handler() -> (HeaderMap, T)
async fn handler() -> (StatusCode, HeaderMap, T)
```

Implement traits similar in purpose to:

```rust
pub trait IntoResponse {
    fn into_response(self) -> Response;
}

pub trait IntoResponseParts {
    type Error;
    fn into_response_parts(
        self,
        parts: &mut ResponseParts,
    ) -> Result<(), Self::Error>;
}
```

Avoid overly complicated public generic signatures. Hide implementation complexity behind stable public abstractions.

## 5. Request Extractors

Implement type-safe extractors for:

* `Path<T>`
* `Query<T>`
* `Json<T>`
* `Form<T>`
* `State<T>`
* `Extension<T>`
* `HeaderMap`
* Typed header extraction.
* Raw request.
* Raw body.
* Byte body.
* String body.
* Multipart form data.
* Client socket address.
* Request metadata.
* Authentication context extension.

Example:

```rust
async fn update_user(
    Path(user_id): Path<u64>,
    Query(options): Query<UpdateOptions>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateUser>,
) -> Result<Json<User>, ApiError> {
    // ...
}
```

Requirements:

* Extractor failures must become structured HTTP responses.
* Malformed JSON must return `400 Bad Request`.
* Unsupported content types should return `415 Unsupported Media Type`.
* Oversized bodies must return `413 Payload Too Large`.
* Query and path deserialization failures must contain safe, actionable information.
* Internal implementation details must not leak to clients in production mode.

Design request-parts extraction separately from body-consuming extraction so multiple extractors cannot accidentally consume the body.

## 6. Response Types

Implement:

* Plain text.
* HTML.
* JSON.
* Redirect.
* File response.
* Streaming body.
* Empty response.
* Server-sent events.
* Custom headers.
* Cookies.
* Status code composition.

Examples:

```rust
Html("<h1>Hello</h1>")
Json(payload)
Redirect::temporary("/login")
(StatusCode::CREATED, Json(payload))
```

Ensure content types are applied correctly.

## 7. Error Handling

Create a coherent error model:

```rust
pub trait HttpError: std::error::Error + Send + Sync + 'static {
    fn status_code(&self) -> StatusCode;
    fn error_code(&self) -> &'static str;
    fn public_message(&self) -> std::borrow::Cow<'static, str>;
}
```

Provide a standard JSON error envelope:

```json
{
  "error": {
    "code": "validation_error",
    "message": "The submitted data is invalid",
    "request_id": "..."
  }
}
```

Implement:

* Framework internal errors.
* Extraction errors.
* Routing errors.
* Method-not-allowed errors.
* Body-limit errors.
* Timeout errors.
* User-defined application errors.
* Error conversion into responses.
* Internal error logging.
* Safe production error messages.
* Optional detailed development error responses.

Never expose:

* Stack traces.
* Filesystem paths.
* Secrets.
* Database connection strings.
* Internal panic messages.

## 8. Application State

Support immutable shared application state through `Arc`.

Example:

```rust
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub database: DatabasePool,
}
```

Requirements:

* State extraction must be type-safe.
* State cloning must be inexpensive.
* Avoid holding synchronous mutex guards across `.await`.
* Document patterns for `Mutex`, `RwLock`, channels, and actor-style state.
* Support extensions for per-request contextual data.

## 9. Middleware

Middleware must be composable and ordered predictably.

Implement an initial middleware stack:

1. Request ID.
2. Sensitive-header redaction.
3. Tracing span.
4. Panic catching.
5. Request body limit.
6. Request timeout.
7. CORS.
8. Security headers.
9. Compression.
10. Access log.

Document how ordering affects behavior.

### Request ID

* Accept a valid incoming request ID when configured.
* Generate a UUID or equivalent identifier otherwise.
* Add it to the response headers.
* Add it to tracing fields.
* Include it in error responses.

### Tracing

Use structured tracing.

Log:

* Request ID.
* HTTP method.
* Sanitized URI.
* Matched route.
* Response status.
* Duration.
* Client address when available.
* User-agent when configured.
* Error classification.

Do not log:

* Authorization headers.
* Cookies by default.
* Passwords.
* Tokens.
* Complete request bodies.
* Sensitive query parameters.

### Panic Handling

Convert handler panics into controlled `500 Internal Server Error` responses while logging the incident.

Do not use panic catching as a replacement for correct error handling.

### CORS

Support configurable:

* Allowed origins.
* Allowed methods.
* Allowed headers.
* Exposed headers.
* Credentials.
* Preflight cache duration.

Reject insecure combinations such as wildcard origins with credentials when prohibited by browser behavior.

### Security Headers

Support defaults for:

* `X-Content-Type-Options`
* `X-Frame-Options`
* `Referrer-Policy`
* `Content-Security-Policy`
* `Permissions-Policy`
* HSTS when HTTPS is enabled and explicitly configured.

## 10. Configuration

Implement layered configuration:

1. Built-in defaults.
2. Configuration file.
3. Environment variables.
4. Explicit programmatic overrides.

Use a typed configuration structure.

Example environment variables:

```env
OXIDE_WEB_HOST=0.0.0.0
OXIDE_WEB_PORT=8080
OXIDE_WEB_LOG=info
OXIDE_WEB_REQUEST_TIMEOUT_SECONDS=30
OXIDE_WEB_GRACEFUL_SHUTDOWN_SECONDS=20
OXIDE_WEB_MAX_BODY_BYTES=2097152
```

Include:

* `.env.example`
* Development configuration.
* Test configuration.
* Production configuration example.
* Validation with useful startup errors.

Never commit real secrets.

## 11. Graceful Shutdown

Implement:

* `Ctrl+C` handling.
* Unix termination signal handling where supported.
* Stop accepting new connections.
* Allow in-flight requests to complete.
* Enforce a maximum draining timeout.
* Log shutdown phases.
* Return a meaningful error when shutdown fails.

Keep platform-specific signal handling behind clean conditional compilation.

## 12. Static Files

Implement secure static-file serving with:

* MIME type detection.
* Index file support.
* Configurable cache headers.
* Conditional requests where practical.
* Range requests as a later milestone.
* Directory traversal prevention.
* Symbolic-link behavior documented.
* Optional fallback for single-page applications.

Never allow `../` or encoded traversal attacks to escape the configured public directory.

## 13. WebSockets

After the core HTTP framework is stable, add WebSocket support:

* Upgrade requests.
* Typed WebSocket message API.
* Text and binary messages.
* Ping/pong handling.
* Close frames.
* Maximum message size.
* Clean connection shutdown.
* Example chat application.

WebSockets are not part of the first vertical slice. Complete routing, handlers, extractors, errors, and middleware first.

## 14. Server-Sent Events

Provide an SSE response type supporting:

* Event name.
* Event ID.
* Data.
* Retry interval.
* Keep-alive comments.
* Stream cancellation when clients disconnect.

## 15. TLS

Provide optional TLS support using Rustls.

Requirements:

* Feature-gated dependency.
* PEM certificate and private key loading.
* Secure defaults.
* Configurable certificate paths.
* Graceful handling of TLS errors.
* No custom cryptographic implementation.
* Document deployment behind Nginx, HAProxy, Traefik, or another reverse proxy.

The framework must work both with direct TLS and behind a reverse proxy.

## 16. Proxy Awareness

Implement optional trusted-proxy behavior for:

* `Forwarded`
* `X-Forwarded-For`
* `X-Forwarded-Proto`
* `X-Forwarded-Host`

Security requirements:

* Do not trust forwarded headers by default.
* Require an explicit trusted-proxy configuration.
* Document spoofing risks.
* Preserve the original peer address.

## 17. Observability

Provide:

* Structured logs.
* Tracing spans.
* Request duration metrics.
* Request count by route and status.
* Active connection count.
* In-flight request count.
* Error count.
* Optional Prometheus-compatible metrics endpoint.
* Health endpoint.
* Readiness endpoint.

Avoid high-cardinality metric labels. Do not use raw URLs containing IDs as metric labels; use matched route templates.

## 18. Testing

Write:

### Unit Tests

* Route parsing.
* Route priority.
* Parameter extraction.
* Query extraction.
* JSON extraction.
* Response conversion.
* Error conversion.
* Configuration validation.
* Middleware behavior.
* Graceful shutdown state transitions.

### Integration Tests

* Real HTTP server on an ephemeral port.
* GET, POST, PUT, PATCH, and DELETE.
* JSON request and response.
* Invalid JSON.
* Oversized body.
* Route parameters.
* Query parameters.
* Middleware ordering.
* Timeouts.
* Panic recovery.
* `404`.
* `405`.
* Graceful shutdown.
* Concurrent requests.
* Connection reuse.

### Property and Fuzz Tests

Add fuzzing or property tests for:

* Route parser.
* Path normalization.
* Percent decoding.
* Header handling boundaries.
* Static-file path resolution.

Never fuzz external networks or production systems.

### Concurrency Tests

Test:

* Multiple simultaneous clients.
* Slow handlers.
* Slow request bodies.
* Shutdown while requests are active.
* Cancellation.
* Timeout cleanup.
* No task leaks in tested scenarios.

## 19. Benchmarks

Use Criterion or another suitable stable Rust benchmarking tool.

Benchmark:

* Static route lookup.
* Parameterized route lookup.
* Deep nested route lookup.
* JSON serialization.
* Plain-text response.
* Middleware overhead.
* Concurrent request handling.
* Large routing tables.

Benchmarks must:

* Be reproducible.
* Separate framework overhead from network overhead.
* Avoid misleading claims.
* Include build mode, hardware, operating system, and command used.
* Compare against Axum only after the framework’s core implementation is correct and stable.

Performance must never come at the cost of unsoundness or incorrect HTTP behavior.

## 20. Documentation

Create a high-quality README containing:

* Project purpose.
* Current development status.
* Feature list.
* Non-goals.
* Quick start.
* Hello-world example.
* REST API example.
* Middleware example.
* Configuration.
* Testing commands.
* Benchmark commands.
* Security notes.
* Architecture summary.
* Roadmap.
* Contributing guide.
* License.

Add rustdoc documentation to all public APIs.

Enable:

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]
```

Use crate-level documentation and runnable documentation examples.

All documentation examples must be tested where practical.

## 21. Developer Tooling

Configure:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --doc --workspace
cargo doc --workspace --all-features --no-deps
cargo build --workspace --all-targets --all-features
```

Also configure:

* `cargo-deny` for licenses, advisories, bans, and duplicate dependencies.
* `cargo-audit` for known vulnerabilities.
* Optional `cargo-nextest`.
* Optional code coverage.
* Git hooks or documented pre-commit commands.
* GitHub Actions CI.

Use `cargo machete` or equivalent tooling to identify unused dependencies if appropriate.

## 22. CI/CD

Create GitHub Actions workflows that run:

* Formatting.
* Clippy.
* Unit tests.
* Integration tests.
* Documentation tests.
* Build on Linux.
* Build checks on macOS and Windows when practical.
* Dependency audit.
* License policy checks.
* Minimal supported Rust version check only after an MSRV is intentionally defined.
* Release build.
* Artifact generation where useful.

Pin GitHub Actions to stable major versions or commit SHAs according to the project’s security policy.

Do not place publishing credentials in the repository.

## 23. Container Support

Create a multi-stage Dockerfile:

* Rust build stage.
* Minimal runtime stage.
* Non-root runtime user.
* Read-only-friendly filesystem.
* Health check.
* Exposed configurable port.
* No development tools in the runtime image.

Create a Docker Compose example for running the REST API example.

The container must respond correctly to termination signals and perform graceful shutdown.

## 24. Security Baseline

Review the design against:

* Slowloris-style behavior.
* Unbounded request bodies.
* Unbounded response buffering.
* Header abuse.
* Path traversal.
* Request smuggling assumptions.
* Log injection.
* Secret leakage.
* Panic exposure.
* Excessive error detail.
* Denial of service through unconstrained concurrency.
* Untrusted proxy headers.
* Invalid UTF-8 assumptions.
* Unsafe file paths.
* Dependency vulnerabilities.

Document which protections are implemented and which depend on a reverse proxy.

## 25. Public API Stability

The project is initially pre-1.0.

Follow these rules:

* Keep internal types private by default.
* Expose narrow, intentional interfaces.
* Avoid leaking dependency-specific types unnecessarily.
* Re-export standard HTTP types where it improves usability.
* Mark experimental features clearly.
* Document breaking changes.
* Maintain a changelog.
* Add compile tests for important public API patterns.

## Implementation Phases

### Phase 0 — Investigation and Design

Before coding:

1. Inspect the local environment.
2. Verify Rust installation.
3. Study current stable APIs for Tokio, Hyper, Tower, HTTP, HTTP body utilities, tracing, serde, and Rustls.
4. Write `docs/architecture.md`.
5. Record key architectural decisions.
6. Identify technical risks.
7. Define the first vertical slice.
8. Create the workspace.

Do not spend the whole phase writing theoretical documents. Move promptly to a working server.

### Phase 1 — Minimum Working Vertical Slice

Deliver a working framework that supports:

* Tokio TCP listener.
* Hyper HTTP server.
* `Application`.
* Router.
* Static routes.
* GET method.
* Async handler.
* Plain-text response.
* `404`.
* Graceful shutdown.
* Request tracing.
* Unit tests.
* Hello-world example.

Acceptance test:

```bash
cargo run -p hello-world
curl -i http://127.0.0.1:8080/
curl -i http://127.0.0.1:8080/not-found
```

Expected:

* `/` returns `200 OK`.
* Unknown path returns `404 Not Found`.
* Server logs request metadata.
* `Ctrl+C` triggers graceful shutdown.

### Phase 2 — Routing and Response System

Add:

* All major HTTP methods.
* Parameters.
* Wildcards.
* Nested routers.
* Method-not-allowed handling.
* `IntoResponse`.
* Status and header composition.
* JSON responses.
* Redirects.
* HTML responses.
* Comprehensive routing tests.

### Phase 3 — Extractors and Errors

Add:

* Path extraction.
* Query extraction.
* JSON extraction.
* State extraction.
* Extension extraction.
* Body limits.
* Structured rejection responses.
* Application error abstraction.
* Standard error envelope.

Create the REST API example during this phase.

### Phase 4 — Middleware

Add:

* Request ID.
* Tracing.
* Panic catching.
* Timeout.
* CORS.
* Body size limit.
* Compression.
* Security headers.
* Middleware ordering tests.

### Phase 5 — Production Server Features

Add:

* Server configuration.
* Environment configuration.
* HTTP/2 where supported.
* Rustls TLS.
* Trusted proxy support.
* Static files.
* Health and readiness endpoints.
* Metrics.
* Graceful connection draining.

### Phase 6 — Streaming Features

Add:

* Streaming responses.
* Server-sent events.
* WebSockets.
* Backpressure documentation.
* Cancellation tests.
* WebSocket example.

### Phase 7 — Quality and Release Preparation

Complete:

* Security review.
* Dependency audit.
* CI matrix.
* Fuzz targets.
* Benchmarks.
* Container image.
* Complete documentation.
* API review.
* Changelog.
* Contribution guide.
* Version `0.1.0` release preparation.

## First Session Deliverables

During this first execution, complete at least:

1. Environment inspection.
2. Git repository initialization if this is a new empty project.
3. Cargo workspace creation.
4. Architecture document.
5. Working server core.
6. Basic router.
7. GET route registration.
8. Async handler execution.
9. Plain-text responses.
10. `404` handling.
11. Request tracing.
12. Graceful shutdown.
13. Unit tests.
14. Integration tests.
15. Hello-world example.
16. README with exact commands.
17. Formatting, Clippy, tests, and documentation checks.

Do not attempt every advanced feature in the first session if that would reduce implementation quality.

## Required Validation

Before finishing the first session, run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --doc --workspace
cargo build --workspace --all-targets --all-features
cargo doc --workspace --all-features --no-deps
```

Then run the example server and validate it with HTTP requests.

Use an ephemeral background process carefully and ensure it is terminated after testing.

## Final Agent Report

At the end, provide:

1. A concise summary of what was implemented.
2. The final directory tree.
3. Important architectural decisions.
4. Public API examples.
5. Commands executed.
6. Test, Clippy, and formatting results.
7. Known limitations.
8. Security considerations.
9. Next recommended milestone.
10. Files added or changed.

Do not claim that a check passed unless it was actually executed successfully.

Begin by inspecting the current directory and Rust environment, then create the architecture document and implement Phase 1.

