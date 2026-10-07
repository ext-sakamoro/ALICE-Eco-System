# Changelog

All notable changes to ALICE-Eco-System will be documented in this file.

## [Unreleased]

### Added
- `docs/crate-index.tsv` と `scripts/readme_index.py`: README の crate 索引の元データと検査器 `--check` は README の表と TSV の一致 (CI、3 OS)、`--online` は TSV と各 repository の一致 (public か、Cargo.toml の license、crates.io の公開状況、索引に無い public crate repository) を見る `crate-index.yml` が TSV の変更時と毎週実行する 比較件数 0 は失敗
- `scripts/docs_lint.py` と test: 公開文書と追跡 file の語彙、CHANGELOG の版見出しを検査 (CI、3 OS)
- `docs/DEMOS.md`: 3 本の pipeline demo (`cargo run` / `sdf_delivery` / `game_pipeline`) の説明を README から移した 現状は公開されていない crate への path 依存があり public の checkout だけでは build できないことを明記

### Changed
- README を crate 索引中心に改稿 掲載は public の Rust crate repository (hosted service の repository を除く) で、license は各 Cargo.toml の値、版数は本文に書かず crates.io の badge で表示する 旧 README の版数のずれ、公開されていない repository への言及、存在しない repository 名を除いた
- **`bridge_physics` / `bridge_physics_scene_io` の test を `ManifoldConfig` / `PhysicsScene` の struct literal から `Default` / `new` に変更 (2026-09-30)** `PhysicsScene` は alice-physics 1.5.0 で既に `#[non_exhaustive]` なので、literal のままでは test が compile できなかった (E0639) `ManifoldConfig` は将来の付与に備えた予防 公開 API の変更なし

### Removed
- build 生成物の `libbridge_*.rlib` 22 file を追跡対象から外し `.gitignore` に `*.rlib` を追加

## [0.3.4] - 2026-07-01

### Changed
- README: 25 crate の version + description を各 crate の現状に更新
  - 第 1 群 5: Blockchain v1.4.0 / Audit v1.3.0 / Space v0.6.0 / Signal v1.4.0 / Carbon v0.5.0
  - 拡張 5: PKI v1.2.0 / Legal v0.3.0 / Identity v1.2.0 / Ledger v0.3.0 / Quant v1.2.0
  - 追加 5: Settlement v0.2.0 / Medical v1.1.0 / DNS v0.2.0 / Payment v1.1.0 / FIX v0.2.0
  - v5 拡張 5: Bio v0.2.0 / Genome v1.1.0 / Compliance v0.2.0 / Risk v0.2.0 / Billing v0.2.0
  - v6 拡張 5: Logistics v1.1.0 / Semantic-Telemetry v0.2.0 / Edge-Firewall v0.2.0 / Queue v0.2.0 / Search v0.2.0
- 各 crate の Feature 欄に業界標準準拠 (RFC 3161 / W3C VC / MiFID-II / Basel III / SOC2 / GDPR / ISO 28000 / DICOM PS3.10 / EN-16931 / DNSSEC / GS1 EPCIS 2.0 / PCI-DSS 等) 反映

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
