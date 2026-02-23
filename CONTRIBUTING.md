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

## Design Constraints

- **Bridge pattern**: each `bridge_xxx.rs` contains 5 conversion functions with FNV-1a `content_hash`.
- **File-local FNV-1a**: every bridge file has its own `fnv1a()` — no shared dependency.
- **`content_hash` first**: all bridge output structs place `content_hash: u64` as the first field.
- **Branchless TTL**: Cache-targeted bridges compute TTL with branchless `base - condition * delta`.
- **Enum → u8 via match**: never use `as u8` cast; always explicit `match` mapping.
- **Minimum 8 tests per bridge file**: basic conversion, TTL paths, hash determinism.
- **Feature-gated optionals**: proprietary / platform-specific crates are `optional = true` with `#[cfg(feature)]`.
