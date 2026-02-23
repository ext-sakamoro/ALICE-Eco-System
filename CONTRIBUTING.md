# Contributing to ALICE-Eco-System

## Build

```bash
cargo build
```

## Test

```bash
cargo test
```

## Lint

```bash
cargo clippy -- -W clippy::all
cargo fmt -- --check
cargo doc --no-deps 2>&1 | grep warning
```

## Optional Features

```bash
# Content-creation bridges (Animation, Manga, Print — proprietary)
cargo build --features content-creation

# Enterprise bridges (Edge-Commercial, ASP-Commercial)
cargo build --features enterprise

# Neural bridge
cargo build --features neural

# Atoms bridge
cargo build --features atoms
```

## License Boundary Policy

ALICE crates follow a **MIT Core + AGPL SaaS Shell** licensing strategy.
This boundary must be strictly maintained.

### What belongs in MIT-licensed crates (SDF, Eco-System, etc.)

- Pure computation algorithms (math, data structures, serialization)
- Platform-agnostic abstractions (no network, no I/O, no cloud)
- Bridge conversion functions (type A → type B)
- Deterministic, stateless transforms

### What does NOT belong in MIT-licensed crates

- HTTP/gRPC server logic, endpoints, or routing
- Authentication middleware, session management, token handling
- Cloud service integrations (AWS, GCP, Azure SDKs)
- Database connection pooling, ORM, or query builders
- Billing, metering, usage tracking, or analytics dashboards
- SaaS-specific features (multi-tenancy, subscription management)
- API key management or rate limiting with external state

These features belong in **AGPL-3.0** crates or **commercial-licensed** modules.

### PR Review Checklist for License Boundary

Before merging any PR to a MIT-licensed crate, reviewers must verify:

- [ ] **No network I/O**: PR does not add TCP, HTTP, gRPC, WebSocket, or any socket dependency
- [ ] **No cloud SDK**: PR does not add AWS, GCP, Azure, or other cloud provider dependencies
- [ ] **No SaaS logic**: PR does not add billing, metering, authentication flows, or multi-tenancy
- [ ] **No new `[dependencies]` that imply a server**: Check for tokio, hyper, actix, axum, tonic, reqwest
- [ ] **Feature isolation**: If the feature requires I/O, it is behind an optional feature flag pointing to an AGPL crate

Violations of this policy will be reverted. The MIT Core must remain a
pure computation library that cannot be trivially wrapped into a competing
SaaS without also needing the AGPL-licensed components.

See `BRAND_GUIDELINES.md` for trademark and SaaS wrapping rules.

## Design Constraints

- **Bridge pattern**: each `bridge_xxx.rs` contains 5 conversion functions with FNV-1a `content_hash`.
- **File-local FNV-1a**: every bridge file has its own `fnv1a()` — no shared dependency.
- **`content_hash` first**: all bridge output structs place `content_hash: u64` as the first field.
- **Branchless TTL**: Cache-targeted bridges compute TTL with branchless `base - condition * delta`.
- **Enum → u8 via match**: never use `as u8` cast; always explicit `match` mapping.
- **Minimum 8 tests per bridge file**: basic conversion, TTL paths, hash determinism.
- **Feature-gated optionals**: proprietary / platform-specific crates are `optional = true` with `#[cfg(feature)]`.
