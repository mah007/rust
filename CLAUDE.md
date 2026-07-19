# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Current state

This is a **greenfield project — no code exists yet**. The directory contains a single file, `prompt.md`, which is the authoritative specification for what is to be built. Before doing anything, re-read `prompt.md`; it is the source of truth for scope, constraints, phasing, and acceptance criteria.

As of this writing:
- No `Cargo.toml`, `crates/`, or any Rust source exists.
- The Rust toolchain is **not installed** (`rustc`, `cargo`, `rustup` are all absent). Bootstrap it before building:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # if rustup is missing
  rustup update stable && rustup default stable
  rustup component add rustfmt clippy
  ```
- Not a git repository. Initialize one before the first commit.

## The project: `oxide-web`

A production-grade async HTTP web server **and** an ergonomic web framework built on top of it — conceptually an Axum-style framework with its own public API. Built on **Tokio, Hyper, the `http` crate, and Tower-compatible `Service`/`Layer` abstractions**. Axum may be studied as a reference, but the implementation and public API must be original.

Workspace name: `oxide-web` (crate names use underscores: `oxide_web`).

The target developer-facing API (from the spec) looks like:
```rust
let app = Application::new()
    .route("/health", get(health))
    .route("/users/:id", get(get_user))
    .route("/users", post(create_user))
    .with_state(state)
    .layer(middleware::request_id())
    .layer(middleware::tracing());
app.bind("0.0.0.0:8080").graceful_shutdown(shutdown::ctrl_c()).run().await?;
```

## Intended architecture (Cargo workspace)

Multi-crate workspace under `crates/`. **Do not create a crate unless it has a clear, distinct responsibility — merge rather than split when splitting only adds complexity.** Planned crates and their responsibilities:

- **`oxide-web-core`** — server lifecycle: Tokio listener, Hyper connection handling, HTTP/1.1 (+HTTP/2 where compatible), request/response body types, `Service` abstractions, graceful shutdown, timeouts, connection limits, remote address propagation, server errors.
- **`oxide-web-router`** — route matching via an **efficient structure (radix/segment tree, not a permanent linear scan)**: static routes, named params (`/users/:id`), wildcards (`/assets/*path`), method dispatch, nested routers, route groups, conflict detection, 404/405, automatic HEAD/OPTIONS. Priority order: **static > parameter > wildcard**.
- **`oxide-web-middleware`** — reusable `Layer`s: request ID, tracing, access log, panic catch, timeout, compression, CORS, body-size limit, concurrency limit, rate-limit interface, security headers, sensitive-header redaction.
- **`oxide-web-macros`** — **optional**; the framework must remain fully usable with plain functions and builders. Procedural macros must never be mandatory.
- **`oxide-web-testing`** — in-process app testing, request builders, response/body/JSON/header/status assertions, ephemeral-port startup.
- **`oxide-web`** — the top-level facade crate that re-exports the public API (`extract`, `routing`, `middleware`, `Application`, `HttpError`, `HttpResult`, `shutdown`, …).

Supporting trees: `examples/` (hello-world, rest-api, middleware-demo, websocket-chat), `benches/`, `tests/` (integration + fixtures), `docs/` (architecture, routing, extractors, middleware, errors, configuration, security, roadmap).

### Key design boundaries (enforced by the spec)
- **Separate request-parts extraction from body-consuming extraction** so multiple extractors can't accidentally consume the body twice. This shapes the extractor trait design.
- Handlers are async fns converted to services; return values flow through an `IntoResponse` / `IntoResponseParts` model (text, `String`, `StatusCode`, `Json<T>`, tuples like `(StatusCode, T)`, `Result<T, E>`, etc.).
- Errors implement a single `HttpError` trait (`status_code`, `error_code`, `public_message`) and render to a standard JSON envelope `{ "error": { "code", "message", "request_id" } }`. **Never leak** stack traces, filesystem paths, secrets, connection strings, or panic messages to clients.
- State is immutable and shared via `Arc`; cheap to clone. **Never hold a sync mutex guard across `.await`.**
- Middleware ordering is significant and must be documented; the canonical stack order is request-id → header redaction → tracing → panic catch → body limit → timeout → CORS → security headers → compression → access log.
- Layered configuration precedence: **defaults < config file < environment variables < explicit programmatic overrides**. Env vars are prefixed `OXIDE_WEB_` (e.g. `OXIDE_WEB_PORT`, `OXIDE_WEB_REQUEST_TIMEOUT_SECONDS`, `OXIDE_WEB_MAX_BODY_BYTES`).
- Untrusted proxy headers (`X-Forwarded-*`, `Forwarded`) are **not trusted by default** — require explicit trusted-proxy config.
- TLS is **feature-gated, Rustls only**. Never implement custom crypto. Never hand-roll a full HTTP parser — rely on Hyper.

## Hard constraints (from `prompt.md`)

- **Rust 2024 edition**, latest stable toolchain. Record the resolved toolchain and key dependency versions in the README.
- **Prefer safe Rust.** Crates enable `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]`. Any `unsafe` requires a written justification, a documented safety invariant, and dedicated tests.
- Use current, real, compatible crate versions — do not guess versions.
- Build **working vertical slices**, not empty folders or placeholder files. Do not create scaffolding you don't implement.
- Run fmt, clippy, tests, examples, and doc checks **after each major phase**, not just at the end.
- Only claim a check passed if it was actually executed successfully.

## Implementation phasing

Build in vertical slices, in order. Do not chase advanced features before the core is correct.

1. **Phase 1 (first deliverable):** Tokio listener + Hyper server, `Application`, router with static GET routes, async handlers, plain-text responses, 404, graceful shutdown, request tracing, unit + integration tests, `hello-world` example.
2. **Phase 2:** all HTTP methods, params, wildcards, nested routers, 405, `IntoResponse`, status/header composition, JSON/HTML/redirect responses.
3. **Phase 3:** extractors (`Path`/`Query`/`Json`/`State`/`Extension`), body limits, structured rejections, app error model + error envelope, `rest-api` example.
4. **Phase 4:** middleware (request-id, tracing, panic catch, timeout, CORS, body limit, compression, security headers) + ordering tests.
5. **Phase 5:** server/env config, HTTP/2, Rustls, trusted proxies, static files, health/readiness, metrics, connection draining.
6. **Phase 6:** streaming responses, SSE, WebSockets (+`websocket-chat` example).
7. **Phase 7:** security review, dependency audit, CI matrix, fuzz targets, benchmarks, container image, docs, `0.1.0` prep.

WebSockets, SSE, TLS, static files, and metrics are explicitly **not** part of the first slice.

## Commands

These are the validation commands the spec mandates (they apply once the workspace exists):

```bash
# Format / lint / test / docs / build — run after each major phase and before finishing.
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --doc --workspace
cargo doc --workspace --all-features --no-deps
cargo build --workspace --all-targets --all-features

# Run a single test
cargo test --workspace <test_name>
cargo test -p <crate> <test_name>          # scope to one crate

# Run an example (Phase 1 acceptance test)
cargo run -p hello-world
curl -i http://127.0.0.1:8080/             # expect 200 OK
curl -i http://127.0.0.1:8080/not-found    # expect 404 Not Found
# Ctrl+C must trigger graceful shutdown
```

Supply-chain / quality tooling to configure (Phase 7): `cargo-deny` (licenses, advisories, bans, duplicates), `cargo-audit`, optionally `cargo-nextest`, `cargo machete` for unused deps. A `Makefile`/`justfile` should wrap these.

When running the example server for validation, launch it as a background process and **ensure it is terminated after testing**.

## Security baseline

The design must be reviewed against (see `prompt.md` §24): Slowloris, unbounded request bodies, unbounded response buffering, header abuse, path traversal (`../` and encoded), request smuggling, log injection, secret leakage, panic exposure, excessive error detail, unconstrained-concurrency DoS, untrusted proxy headers, invalid-UTF-8 assumptions, unsafe file paths, and dependency vulnerabilities. Static-file serving must never escape the configured public directory. Document which protections are implemented in-framework vs. delegated to a reverse proxy.
