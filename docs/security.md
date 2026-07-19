# oxide-web security notes

This document tracks the framework's security posture against the threat list in
the project spec (§24), and records which protections are implemented in-
framework versus delegated to a reverse proxy. It grows with each phase.

## Status by threat (Phase 1)

| Threat | Status | Notes |
|--------|--------|-------|
| Panic exposure | ✅ core | Server accept/connection errors are logged, never panic the process. Handler-panic isolation (catch → `500`) arrives with middleware in Phase 4. |
| Secret / detail leakage | ✅ core | No stack traces, paths, or internal messages are sent to clients. The typed error envelope (Phase 3) formalizes this. |
| No custom HTTP parser | ✅ core | Parsing is delegated entirely to Hyper. |
| No custom crypto | ✅ core | TLS is Rustls-only and feature-gated (Phase 5). |
| Untrusted proxy headers | ✅ core | `X-Forwarded-*` / `Forwarded` are **not** trusted; handlers see the real transport peer. Opt-in trusted-proxy config is Phase 5. |
| Unbounded request bodies | ⏳ Phase 3/4 | `ServerConfig::max_request_body_size` default is set; enforcement lands with the body-limit extractor/layer. |
| Slowloris / slow bodies | ⏳ Phase 5 | Connection & request timeouts are configured in `ServerConfig`; full read-timeout enforcement + reverse-proxy guidance in Phase 5. |
| Unbounded response buffering | ⏳ Phase 6 | Streaming bodies (the `Body::Boxed` variant is already in place). |
| Header abuse | ⏳ Phase 5 | Hyper enforces baseline header limits; configurable limits in Phase 5. |
| Path traversal | ⏳ Phase 5 | Applies to static-file serving (Phase 5); resolution will be confined to the public root, rejecting `../` and encoded traversal. |
| Request smuggling | ✅ delegated | Hyper's HTTP/1 + HTTP/2 handling; deploy behind a conforming reverse proxy for defense in depth. |
| Log injection | ✅ core | Structured tracing fields (not string concatenation); sensitive headers are never logged. |
| Excessive error detail | ⏳ Phase 3 | Production vs. development error modes with the error envelope. |
| DoS via unconstrained concurrency | ⏳ Phase 4/5 | Concurrency-limit and rate-limit interfaces (Phase 4) + connection limits (Phase 5). |
| Invalid-UTF-8 assumptions | ✅ core | Bodies/paths handled as bytes; UTF-8 is validated where text is required, never assumed. |
| Dependency vulnerabilities | ⏳ Phase 7 | `cargo-deny` + `cargo-audit` wired into CI. |

## Reverse-proxy delegation

Even at 1.0, oxide-web is designed to run comfortably behind Nginx / HAProxy /
Traefik / Envoy. Recommended to delegate to the proxy: TLS termination (unless
the built-in Rustls feature is used), aggressive connection rate limiting,
large-scale Slowloris mitigation, and IP allow/deny lists. What the framework
always owns: correct HTTP semantics, body-size limits, request timeouts, safe
error rendering, and never trusting forwarded headers without explicit config.
