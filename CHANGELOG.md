# Changelog

All notable changes to ALICE-Eco-System will be documented in this file.

## [0.3.4] - 2026-07-01

### Changed
- README: 25 crate の version + description を SPACID / 12 商機軸対応済状態に更新
  - SPACID 5: Blockchain v1.4.0 / Audit v1.3.0 / Space v0.6.0 / Signal v1.4.0 / Carbon v0.5.0
  - 拡張 5: PKI v1.2.0 / Legal v0.3.0 / Identity v1.2.0 / Ledger v0.3.0 / Quant v1.2.0
  - 追加 5: Settlement v0.2.0 / Medical v1.1.0 / DNS v0.2.0 / Payment v1.1.0 / FIX v0.2.0
  - v5 拡張 5: Bio v0.2.0 / Genome v1.1.0 / Compliance v0.2.0 / Risk v0.2.0 / Billing v0.2.0
  - v6 拡張 5: Logistics v1.1.0 / Semantic-Telemetry v0.2.0 / Edge-Firewall v0.2.0 / Queue v0.2.0 / Search v0.2.0
- 各 crate の Feature 欄に SPACID / 業界標準準拠 (RFC 3161 / W3C VC / MiFID-II / Basel III / SOC2 / GDPR / ISO 28000 / DICOM PS3.10 / EN-16931 / DNSSEC / GS1 EPCIS 2.0 / PCI-DSS 等) 反映

## [0.3.3] - 2026-02-26

### Changed
- README: SaaS Platform section expanded from 40 to 52 products (#40-#52 added)
- README: Added Observability, API Gateway, Backup, Digital Twin, VectorDB, Agent Platform, DataShield, Edge Runtime, Compliance, Workflow, Collab, FinCompliance, Experiment
- lib.rs: doc comment updated to reflect 52 SaaS services

## [0.3.2] - 2026-02-23

### Added
- 8 new bridge modules: physics_2d, physics_softbody, physics_scene_io, sdf_material, sdf_destruction, crypto, fix, risk
- 45 new bridges (411 → 456 total), 116 new tests (613 → 727 total)
- Physics v0.6.0 coverage: 2D physics, cloth/fluid/rope/deformable, scene I/O, multi-world, particle system
- SDF coverage: PBR material, destruction tracking, volume estimation, 2D primitives
- Financial domain: crypto key lifecycle, FIX order/execution, risk limits/margin/circuit breaker
- Cargo.toml: alice-sdf destruction feature enabled

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
