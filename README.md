# oxide-web

A production-oriented **async HTTP server** and an ergonomic, Axum-style **web
framework** built on top of it — written from scratch in safe Rust on
[Tokio](https://tokio.rs), [Hyper](https://hyper.rs), the
[`http`](https://docs.rs/http) crate, and Tower-compatible service abstractions.

> **Status: Phase 1 (foundational vertical slice).** The core is real and
> tested: a Tokio+Hyper server, a radix/segment-trie router, async handlers,
> plain-text responses, `404`/`405`, graceful shutdown, request tracing, an
> in-process test harness, and a runnable example. Extractors, the typed error
> model, the middleware stack, TLS, static files, SSE, and WebSockets are
> planned — see [Roadmap](#roadmap). The public API is pre-1.0 and will evolve.

## Why

Most Rust web work is done on excellent existing frameworks. `oxide-web` exists
to build one *from the ground up* with a clear, documented architecture: a small
core you can read end to end, a router that is a real tree (not a linear scan),
and a design that keeps request-parts extraction separate from body-consuming
extraction so the extractor layer (Phase 3) can be correct by construction.

## Features (Phase 1)

- **Async server** on Tokio + Hyper's automatic HTTP/1.1 **and** HTTP/2
  connection builder — no hand-rolled HTTP parsing.
- **`Application` builder**: `route`, `fallback`, `with_state`, `config`, `bind`,
  `graceful_shutdown`, `run`.
- **Router**: a path-segment trie with static routes, named params (`/users/:id`),
  and wildcards (`/assets/*path`); priority **static > param > wildcard**;
  deterministic matching; startup errors for conflicts and invalid patterns;
  automatic `404` and `405` (with an `Allow` header).
- **Handlers**: any `async fn() -> impl IntoResponse` or
  `async fn(Request) -> impl IntoResponse`.
- **Responses** via `IntoResponse` for `&str`, `String`, `Cow<str>`,
  `StatusCode`, `()`, `Response`, `(StatusCode, T)`, `Result`, and `Option`.
- **Shared state** injected into request extensions with `with_state`.
- **Remote peer address** exposed to handlers (never trusting forwarded headers).
- **Graceful shutdown** on `Ctrl+C`/`SIGTERM` with bounded connection draining.
- **Structured tracing** per request (method, path, status, latency, peer).
- **Per-request timeout** returning `503` (configurable; `0` disables).
- **Testing harness** (`oxide-web-testing`): start an app on an ephemeral port
  and make chainable HTTP assertions.

## Non-goals (for now)

- Not a batteries-included, all-features-on-day-one framework — it grows in
  documented phases.
- No custom TLS/crypto (TLS will be Rustls-only, feature-gated, in Phase 5).
- No custom HTTP parser — Hyper owns wire-level HTTP.
- Trusting proxy headers, static-file serving, metrics, SSE, and WebSockets are
  intentionally deferred to later phases.

## Quick start

Add the workspace crate (from a path/git dependency while pre-release) and write:

```rust
use oxide_web::{Application, routing::get};

async fn hello() -> &'static str {
    "Hello, world!"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new()
        .route("/", get(hello))
        .bind("127.0.0.1:8080")
        .graceful_shutdown(oxide_web::shutdown::ctrl_c())
        .run()
        .await?;
    Ok(())
}
```

### Run the example

```bash
cargo run -p hello-world
# in another terminal:
curl -i http://127.0.0.1:8080/           # 200 OK, "Hello, world!"
curl -i http://127.0.0.1:8080/health     # 200 OK, "OK"
curl -i http://127.0.0.1:8080/whoami     # 200 OK, your peer address
curl -i http://127.0.0.1:8080/not-found  # 404 Not Found
# press Ctrl+C in the server terminal to trigger graceful shutdown
```

## Workspace layout

```
crates/
  oxide-web-core/     server lifecycle, Body/Request/Response, IntoResponse,
                      Handler, Service, graceful shutdown, shutdown signals
  oxide-web-router/   segment-trie router, MethodRouter, 404/405, params
  oxide-web/          facade: the Application builder + curated re-exports
  oxide-web-testing/  in-process TestServer + response assertions
examples/hello-world/ the Phase 1 acceptance example
docs/                 architecture.md, roadmap.md, security.md
```

The `oxide-web-middleware` and `oxide-web-macros` crates from the long-term plan
are intentionally **not** created yet — they arrive with the phase that fills
them, to avoid empty scaffolding. See [docs/architecture.md](docs/architecture.md).

## Configuration

`ServerConfig` is the typed source of truth (bind addresses, request timeout,
graceful-shutdown timeout, max body size, `TCP_NODELAY`, keep-alive, HTTP/2).
Layered configuration precedence will be **defaults < file < env (`OXIDE_WEB_*`)
< programmatic overrides**; env/file loading lands in Phase 5. Today, defaults
and programmatic overrides via `Application::config` / `ServerConfig` are wired.

## Testing & validation commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --doc --workspace
cargo doc --workspace --all-features --no-deps
cargo build --workspace --all-targets --all-features

# a single test
cargo test -p oxide-web-router priority
```

## Security notes

Phase 1 already: never leaks internal details/panics to clients, does not trust
`X-Forwarded-*`/`Forwarded` headers, does not panic on normal network errors,
and delegates all HTTP parsing to Hyper. The full threat-model matrix (Slowloris,
body limits, path traversal, log injection, secret leakage, …) and what is
in-framework vs. delegated to a reverse proxy is tracked in
[docs/security.md](docs/security.md).

## Architecture

See [docs/architecture.md](docs/architecture.md) for the design and the key
decisions (why `Body` is a `Unpin` enum needing no `unsafe`, why extraction is
split into request-parts vs. body-consuming, how the router trie matches with
backtracking, and how graceful shutdown drains connections).

## Roadmap

Phases 2–7 add all HTTP methods + JSON/HTML/redirect responses, extractors + the
error envelope, the middleware stack, production server features (env config,
TLS, static files, metrics, trusted proxies), streaming/SSE/WebSockets, and
release hardening. Details in [docs/roadmap.md](docs/roadmap.md).

## Resolved toolchain & dependency versions

Recorded from the pinned `Cargo.lock` at the time of writing.

| Component | Version |
|-----------|---------|
| rustc / cargo | 1.97.1 (stable, Rust 2024 edition) |
| tokio | 1.53.0 |
| hyper | 1.10.1 |
| hyper-util | 0.1.20 |
| http | 1.4.2 |
| http-body / http-body-util | 1.1.0 / 0.1.4 |
| bytes | 1.12.1 |
| tracing / tracing-subscriber | 0.1.44 / 0.3.23 |

## Contributing

Before opening a PR, run the full validation command block above; all of fmt,
clippy (`-D warnings`), tests, and doc checks must pass. Every crate enables
`#![forbid(unsafe_code)]` and `#![warn(missing_docs)]`.

## License

Dual-licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT)
at your option.
