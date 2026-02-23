# Changelog

All notable changes to ALICE-Eco-System will be documented in this file.

## [0.3.1] - 2026-02-23

### Changed
- README: ALICE-Physics v0.4.0 → v0.6.0 (quality sweep: Debug/PartialEq/Display, CCD, 2D physics, cloth/fluid/rope)
- README: Added ALICE-SIMD v1.0.0 (shared SIMD & fast-math primitives, MIT)
- README: Added ALICE-DB-Enterprise v1.0.0 (security/audit, Proprietary)
- README: Added ALICE-Voice-Commercial v0.1.0 (semantic layer, Proprietary)
- README: Component count 55 → 58, ecosystem diagram updated
- Cargo.toml: description updated (51 → 52 crates)

## [0.3.0] - 2026-02-23

### Added
- 63 bridge modules connecting 51 ALICE crates
- 20 pipeline paths (A–U) orchestrating end-to-end workflows
- `pipeline` — `AlicePipeline` orchestration API for all paths
- `hash` — Shared FNV-1a utility
- Bridge modules for: analytics, animation, api, asp, atoms, auth, bio, browser, cache, cdn, climate, cloud-gateway, codec, container, cross, crypto, db, dns, edge, energy, firewall, fix, font, history, kinematics, ledger, legal, manga, ml, motion, neural, physics, presence, print, queue, risk, rtos, sdf, search, semantic-telemetry, settlement, space, sync, synth, text, trt, vcs, view, voice, zip
- Cross-domain bridges: bio_cross, climate_cross, energy_cross, history_cross, legal_cross, neural_cross, presence_cross, space_cross
- Feature-gated optional crates: animation, manga, print, firewall, edge-commercial, streaming-protocol-commercial, neural, atoms
- Re-exports of key types from all constituent crates
- 613 unit tests
