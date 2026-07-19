# oxide-web roadmap

oxide-web is built in vertical slices. Each phase produces something that
compiles, is tested, and runs — never empty scaffolding.

| Phase | Theme | Status |
|------:|-------|--------|
| 0 | Investigation & design (`docs/architecture.md`) | ✅ done |
| 1 | Minimum working vertical slice | ✅ done |
| 2 | Routing & response system (all methods, params, wildcards, nesting, 405, `IntoResponse`, JSON/HTML/redirect) | ⏳ planned |
| 3 | Extractors & errors (`Path`/`Query`/`Json`/`State`/`Extension`, body limits, error envelope, `rest-api` example) | ⏳ planned |
| 4 | Middleware (request-id, tracing, panic catch, timeout, CORS, body limit, compression, security headers) | ⏳ planned |
| 5 | Production server features (config/env, HTTP/2, Rustls, trusted proxy, static files, health/readiness, metrics, draining) | ⏳ planned |
| 6 | Streaming (responses, SSE, WebSockets, `websocket-chat` example) | ⏳ planned |
| 7 | Quality & release (security review, `cargo-deny`/`cargo-audit`, CI matrix, fuzzing, benchmarks, container, `0.1.0`) | ⏳ planned |

## Phase 1 — delivered

- Tokio TCP listener + Hyper (HTTP/1.1, HTTP/2 via the auto connection builder).
- `Application` builder: `.route()`, `.fallback()`, `.with_state()`,
  `.bind().graceful_shutdown().run()`.
- Segment-trie router with static/param/wildcard matching, priority
  static > param > wildcard, conflict detection, `404`, and `405` with `Allow`.
- Async handlers → `IntoResponse` (text, `String`, `StatusCode`, tuples, …).
- Graceful shutdown via `shutdown::ctrl_c()` (Ctrl+C + `SIGTERM` on Unix) with a
  bounded connection-drain timeout.
- Structured request tracing (method, path, status, duration).
- `oxide-web-testing` in-process client on an ephemeral port.
- Unit + integration tests, a `hello-world` example, and full fmt/clippy/test/
  doc/build validation.

## Near-term (Phase 2) intent

Wire the remaining HTTP methods through `MethodRouter`, add automatic `HEAD`
(from `GET`, body discarded) and `OPTIONS`, param/wildcard extraction surface,
nested routers and route groups, and the `Json`/`Html`/`Redirect` response
types plus richer `IntoResponse` tuple impls.
