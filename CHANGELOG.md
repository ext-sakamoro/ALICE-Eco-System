# Changelog

All notable changes to ALICE-Eco-System will be documented in this file.

## [Unreleased]

### Added
- CI: job `Test (public siblings)` 公開の sibling 85 個を固定の commit でこの repository の隣に checkout し、`cargo test` を走らせる (公開でない sibling は checkout で落ちる) 失敗した試験がある時と、通った試験が 1909 本より少ない時は失敗する `ci/**` への push でも走る `scripts/preflight.sh` (`--quick` 無し) も同じ試験と下限を見る `ALICE-Physics` は package 名が `alice-physics-core` に変わる前の commit に固定している `Cargo.lock` はこの sibling の組で作り直した
- `docs/crate-index.tsv` と `scripts/readme_index.py`: README の crate 索引の元データと検査器 `--check` は README の表と TSV の一致 (CI、3 OS)、`--online` は TSV と各 repository の一致 (public か、Cargo.toml の license、crates.io の公開状況、索引に無い public crate repository) を見る `crate-index.yml` が TSV の変更時と毎週実行する 比較件数 0 は失敗
- `scripts/docs_lint.py` と test: 公開文書と追跡 file の語彙、CHANGELOG の版見出しを検査 (CI、3 OS)
- `docs/DEMOS.md`: 3 本の pipeline demo (`cargo run` / `sdf_delivery` / `game_pipeline`) の説明を README から移した 現状は公開されていない crate への path 依存があり public の checkout だけでは build できないことを明記

### Changed
- `bridge_auth` の試験: `alice-auth` の `Identity::sign` が `Result<AliceSig, SignError>` を返す (予約接頭辞 `alice-auth/` で始まる message には署名しない) のに合わせ、試験の message `b"hello"` の署名を `expect` で取り出す 公開 API の変更なし
- 依存: lock を sibling の main に合わせ、yank 済みの `zip` 2.4.2 (`alice-train` 経由、9.0.4 へ) と `spin` 0.9.8 (0.9.9 へ) を外した この repository の CI は crate を build しない (path 依存の sibling 185 個が要る) ので、lock の古さと yank は CI に現れていなかった
- `bridge_analytics`: `alice_analytics::prelude` はどの公開版にも無く、使っている 2 つの型を `alice_analytics::sketch::{HyperLogLog12, DDSketch256}` から import する (挙動は同じ)
- **`bridge_presence` / `bridge_presence_cross` を `alice-presence` 0.2 の Ed25519 challenge-response に移した (意味が変わる)** 0.1 の `CrossingRecord::is_fully_verified` と `ZkProof::verified` は証明する側が書いた flag を読むだけで、誰でも偽造できた `fully_verified` は `verify_record(record) == VerifyResult::Valid` (両者の署名・近接・識別子 hash を検証する)、`proof_a_valid` / `proof_b_valid` は各 proof の署名を record の transcript と proof が持つ challenge で検証した結果 (challenge が今回の検証者の発行したものか、つまり再送でないかは検証する側の責任で、ここでは確かめない) `CrossingStatus::Verified` は `status()` がもう返さないので、status の byte 2 は現れない 判定と hash の入力が変わるので、`content_hash` の値も変わる (値を固定した試験や golden は無い) 試験: 片方の署名を偽造すると検証されない、両方の署名が通っても識別子の hash (record の `content_hash` か近接の proof の hash) を変えると `fully_verified` は偽で、検証だけが違う 2 つの record は analytics の `content_hash` が異なる
- **`presence_identity_to_auth_token(&IdentityCommitment)` を `presence_proof_to_auth_token(&ChallengeProof, &Challenge, transcript, timestamp_ns) -> Result<PresenceAuthToken, ProofError>` に置き換えた (破壊的変更)** `IdentityCommitment` は FNV の commitment で何も認証しないため `alice-presence` から消えた 新しい関数は、検証者の challenge と transcript に対して署名が通った proof にだけ token を出し、偽造・別の challenge (再送)・別の encounter の transcript・相手の proof には `ProofError` を返す token は公開鍵・challenge・時刻の FNV 識別子を持つ (認証は token を出す前の署名検証) 試験は `Identity::from_seed` で鍵を作り、偽造と再送で token が出ないことを確かめる
- README を crate 索引中心に改稿 掲載は public の Rust crate repository (hosted service の repository を除く) で、license は各 Cargo.toml の値、版数は本文に書かず crates.io の badge で表示する 旧 README の版数のずれ、公開されていない repository への言及、存在しない repository 名を除いた
- **`bridge_physics` / `bridge_physics_scene_io` の test を `ManifoldConfig` / `PhysicsScene` の struct literal から `Default` / `new` に変更 (2026-09-30)** `PhysicsScene` は alice-physics 1.5.0 で既に `#[non_exhaustive]` なので、literal のままでは test が compile できなかった (E0639) `ManifoldConfig` は将来の付与に備えた予防 公開 API の変更なし

### Removed
- **Breaking:** 公開されていない crate を使う module を外し、この crate は公開の crate だけに依存する bridge 48 module (`bridge_cross` を含む) と、`AlicePipeline::anime_production` / `AnimeProductionResult` / `MetaFontParams` の再公開、`bridge_edge` の Bridge 3 (`edge_to_semantic_threshold_event` と試験 3 本) を外した `bridge_edge_commercial` と `bridge_asp_commercial` も外した (どちらも依存の crate 名が合わず、feature を有効にすると compile できなかった) 外した module の試験 563 本は、公開されていない側の repository に同じ名前で在ることを確かめた feature はすべて外した (どれも公開されていない crate を有効にするだけだった: `animation` / `manga` / `print` / `content-creation` / `edge-commercial` / `streaming-protocol-commercial` / `neural` / `atoms` / `voice-commercial` / `enterprise`) 残る試験は 1909 本 `docs/DEMOS.md` / `README.md` / `CONTRIBUTING.md` の build と feature の説明を合わせた
- 依存: どこからも使われていない非公開の path 依存 62 個と、そのうち 1 つだけを有効にしていた feature `firewall` を外した `bridge_firewall` はその crate を使っておらず公開の crate だけで build できるので、feature なしで常に build される (file 内の `fnv1a` は試験だけが使うので `#[cfg(test)]` を付ける) 試験の名前と数は前後で同じ (2472) `Cargo.lock` は sibling を固定した CI の job を入れる時に作り直す
- **Breaking:** `bridge_physics` の Bridge 7 (`multiworld_to_analytics` / `MultiWorldAnalyticsEvent`) と試験 3 本を外し、`alice-physics` の `replay` feature を使わなくした (ALICE-Physics の責務分割で `multi_world` と `replay` が alice-physics から外れるため) `pipeline::AlicePipeline` は game tick の replay の記録を外し、`PipelineConfig` の `replay_path` と `num_bodies` (記録にだけ使っていた) を削除した 例 `game_pipeline` は replay の記録と再生 (旧 PHASE 6) を外し、ALICE-Sync の telemetry を ALICE-DB に保存して集計するところまでを示す (`docs/DEMOS.md` も同じ) この repository の CI は crate を build しないので、`bridge_physics` は分割後の alice-physics に対して単独で build と試験をし、`pipeline` と例は rustfmt の構文検査だけを通している (依存の多くが公開されていない)
- 使われていない bridge module 6 本と、それに対応する lib.rs の pipeline 説明 2 件を削除
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
