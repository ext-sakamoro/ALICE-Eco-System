# ALICE Ecosystem

[![CI](https://github.com/ext-sakamoro/ALICE-Eco-System/actions/workflows/ci.yml/badge.svg)](https://github.com/ext-sakamoro/ALICE-Eco-System/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

An index of the public ALICE crates and the integration hub that connects them.

The ALICE crates are independent Rust libraries for geometry (signed distance
functions), deterministic physics, ternary-weight inference, compression,
storage, networking and related domains. Each crate lives in its own
repository with its own README, CHANGELOG and CI. This repository holds:

- **the crate index** below: one row per public crate repository, with its
  license and a one-line description
- **`src/bridge_*.rs`**: conversion modules that map the types of one crate
  onto another (for example physics state into a database record)
- **three pipeline demos** that wire several crates together, described in
  [docs/DEMOS.md](docs/DEMOS.md) (they do not build from public checkouts
  alone yet; see there)

## Contents

- [What this repository is not](#what-this-repository-is-not)
- [Quick start](#quick-start)
- [Crate index](#crate-index)
- [Licensing model](#licensing-model)
- [Building and checking](#building-and-checking)
- [License](#license)

## What this repository is not

- **Not a single library to depend on.** Depend on the individual crates. The
  hub crate (`alice-eco-system`) is not published to crates.io.
- **Not buildable from this repository alone.** `Cargo.toml` refers to the
  other crates by sibling path (`../ALICE-*`), and some bridge modules target
  crates that are not public. A plain clone of this repository does not
  compile; CI checks formatting, workflow syntax and the index, not a build.
- **Not a statement of maturity.** Crates in the index range from published,
  versioned libraries to early implementations. The crates.io column marks the
  ones that are released; each crate's own README states its status and known
  defects.

## Quick start

Use a crate directly from crates.io:

```sh
cargo add alice-sdf        # signed distance functions
cargo add alice-physics    # deterministic fixed-point physics
cargo add alice-det-math   # bit-exact transcendentals
```

or from its repository:

```toml
[dependencies]
alice-simd = { git = "https://github.com/ext-sakamoro/ALICE-SIMD" }
```

The pipeline demos in this repository are documented in
[docs/DEMOS.md](docs/DEMOS.md); they do not build from public checkouts alone
yet.

## Crate index

Every public `ALICE-*` repository that is a Rust crate is listed here. Hosted
service repositories (the `*-SaaS` repositories and the service templates
built from them) are not.

The tables are generated from [docs/crate-index.tsv](docs/crate-index.tsv) by
`scripts/readme_index.py`; CI fails when they diverge, and a scheduled job
compares the TSV with the repositories themselves (visibility, license from
`Cargo.toml`, crates.io status, and repositories missing from the index).

<!-- crate-index:start -->

### Geometry & Rendering

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-SDF](https://github.com/ext-sakamoro/ALICE-SDF) | [![crates.io](https://img.shields.io/crates/v/alice-sdf.svg)](https://crates.io/crates/alice-sdf) | MIT OR Apache-2.0 | Signed distance functions: CSG, GLSL / WGSL / HLSL transpilation, sparse voxel octree, meshing |
| [ALICE-LOL](https://github.com/ext-sakamoro/ALICE-LOL) | [![crates.io](https://img.shields.io/crates/v/alice-lol.svg)](https://crates.io/crates/alice-lol) | MIT OR Apache-2.0 | Law-oriented SDF DSL as a proc macro, with shader transpilation and print export (STL / 3MF / OBJ / FBX) |
| [ALICE-Shader](https://github.com/ext-sakamoro/ALICE-Shader) | `alice-shader` | MIT OR LicenseRef-Commercial | GLSL and WGSL shader library (sky, terrain, PBR, SDF, VFX); frozen, SDF primitives moved to alice-sdf |
| [ALICE-View](https://github.com/ext-sakamoro/ALICE-View) | `alice-view` | MIT OR Apache-2.0 | Real-time procedural rendering engine |
| [ALICE-GameEngine](https://github.com/ext-sakamoro/ALICE-GameEngine) | `alice-game-engine` | MIT OR LicenseRef-Commercial | Hybrid mesh and SDF game engine on wgpu |
| [ALICE-VR](https://github.com/ext-sakamoro/ALICE-VR) | `alice-vr` | MIT OR Apache-2.0 | VR runtime: head tracking, lens distortion, stereo rendering, reprojection |
| [ALICE-Browser](https://github.com/ext-sakamoro/ALICE-Browser) | `alice-browser` | MIT OR Apache-2.0 | Semantic web browser built on the ecosystem crates |

### Physics & Simulation

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-Physics](https://github.com/ext-sakamoro/ALICE-Physics) | [![crates.io](https://img.shields.io/crates/v/alice-physics.svg)](https://crates.io/crates/alice-physics) | AGPL-3.0-or-later OR LicenseRef-Commercial | Deterministic physics on 128-bit fixed point: bit-exact replay, snapshots, rollback |
| [ALICE-DetMath](https://github.com/ext-sakamoro/ALICE-DetMath) | [![crates.io](https://img.shields.io/crates/v/alice-det-math.svg)](https://crates.io/crates/alice-det-math) | MIT OR Apache-2.0 | Cross-platform bit-exact f32 / f64 transcendentals, scalar and SIMD, no_std |
| [ALICE-Motion](https://github.com/ext-sakamoro/ALICE-Motion) | `alice-motion` | MIT OR Apache-2.0 | NURBS / Bezier trajectory control with trapezoidal and S-curve profiles, no_std |
| [ALICE-Kinematics](https://github.com/ext-sakamoro/ALICE-Kinematics) | [![crates.io](https://img.shields.io/crates/v/alice-kinematics.svg)](https://crates.io/crates/alice-kinematics) | MIT OR Apache-2.0 | Compression of human motion samples into compact kinematic intent |
| [ALICE-Navigation](https://github.com/ext-sakamoro/ALICE-Navigation) | `alice-navigation` | MIT OR Apache-2.0 | Path planning: RRT, PRM, potential fields, velocity obstacles, navigation mesh |
| [ALICE-Swarm](https://github.com/ext-sakamoro/ALICE-Swarm) | `alice-swarm` | MIT OR Apache-2.0 | Swarm control: Boids, formation, consensus, task allocation |
| [ALICE-Optics](https://github.com/ext-sakamoro/ALICE-Optics) | `alice-optics` | MIT OR Apache-2.0 | Optical simulation: lens systems, ray tracing, diffraction, polarization |
| [ALICE-Chemistry](https://github.com/ext-sakamoro/ALICE-Chemistry) | `alice-chemistry` | MIT OR Apache-2.0 | Molecular dynamics and chemistry: force fields, reaction kinetics, thermodynamics |
| [ALICE-Bio](https://github.com/ext-sakamoro/ALICE-Bio) | `alice-bio` | AGPL-3.0-only OR LicenseRef-Commercial | Molecular structure as SDF: amino acid potentials and pairwise interactions |
| [ALICE-Climate](https://github.com/ext-sakamoro/ALICE-Climate) | `alice-climate` | MIT OR Apache-2.0 | Climate fields as continuous SDF: atmosphere, ocean, anomaly detection |
| [ALICE-Energy](https://github.com/ext-sakamoro/ALICE-Energy) | `alice-energy` | AGPL-3.0-only OR LicenseRef-Commercial | Power grid simulation: phase synchronization, battery degradation, frequency regulation |
| [ALICE-Space](https://github.com/ext-sakamoro/ALICE-Space) | `alice-space` | AGPL-3.0-only OR LicenseRef-Commercial | Orbital mechanics and satellite positioning |
| [ALICE-Space-Drone-Bridge](https://github.com/ext-sakamoro/ALICE-Space-Drone-Bridge) | `alice-space-drone-bridge` | AGPL-3.0-only OR LicenseRef-Commercial | Bridge from satellite positioning to drone control: geodetic to local frames, waypoints, geofences |

### AI & ML

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-LLM](https://github.com/ext-sakamoro/ALICE-LLM) | [![crates.io](https://img.shields.io/crates/v/alice-llm.svg)](https://crates.io/crates/alice-llm) | AGPL-3.0-or-later OR LicenseRef-Commercial | LLM inference engine: GGUF, K-quants, CPU and wgpu GPU paths, speculative decoding, OpenAI-compatible server |
| [ALICE-ML](https://github.com/ext-sakamoro/ALICE-ML) | [![crates.io](https://img.shields.io/crates/v/alice-ml.svg)](https://crates.io/crates/alice-ml) | AGPL-3.0-or-later OR LicenseRef-Commercial | 1.58-bit ternary inference with add / subtract only matrix operations |
| [ALICE-TRT](https://github.com/ext-sakamoro/ALICE-TRT) | `alice-trt` | AGPL-3.0 OR LicenseRef-Commercial | GPU ternary inference engine with 2-bit bitplane weights on wgpu |
| [ALICE-Train](https://github.com/ext-sakamoro/ALICE-Train) | `alice-train` | AGPL-3.0 OR LicenseRef-Commercial | Backpropagation and training for ternary networks |
| [ALICE-Token](https://github.com/ext-sakamoro/ALICE-Token) | `alice-token` | MIT OR Apache-2.0 | BPE tokenizer |
| [ALICE-AutoML](https://github.com/ext-sakamoro/ALICE-AutoML) | `alice-automl` | MIT OR Apache-2.0 | Hyperparameter search, Bayesian optimization, early stopping, cross-validation |
| [ALICE-GAN](https://github.com/ext-sakamoro/ALICE-GAN) | `alice-gan` | MIT OR Apache-2.0 | Generative adversarial network framework |
| [ALICE-Agent](https://github.com/ext-sakamoro/ALICE-Agent) | `alice-agent` | AGPL-3.0 OR LicenseRef-Commercial | Local-first coding agent on the ternary models |

### Compression & Media

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-Zip](https://github.com/ext-sakamoro/ALICE-Zip) | [![crates.io](https://img.shields.io/crates/v/alice-zip.svg)](https://crates.io/crates/alice-zip) | MIT OR Apache-2.0 | Compression (LZ77, dictionary coding) and procedural signal generators, no_std |
| [ALICE-Edge](https://github.com/ext-sakamoro/ALICE-Edge) | [![crates.io](https://img.shields.io/crates/v/alice-edge.svg)](https://crates.io/crates/alice-edge) | MIT OR Apache-2.0 | Embedded model fitting that compresses sensor samples into a few bytes, no_std |
| [ALICE-Codec](https://github.com/ext-sakamoro/ALICE-Codec) | [![crates.io](https://img.shields.io/crates/v/alice-codec.svg)](https://crates.io/crates/alice-codec) | AGPL-3.0-or-later OR LicenseRef-Commercial | 3D wavelet video codec |
| [ALICE-Text](https://github.com/ext-sakamoro/ALICE-Text) | `alice-text` | MIT OR Apache-2.0 | Exception-based text compression |
| [ALICE-Voice](https://github.com/ext-sakamoro/ALICE-Voice) | `alice-voice` | MIT OR Apache-2.0 | Parametric voice codec (LPC, pitch, gain) |
| [ALICE-Synth](https://github.com/ext-sakamoro/ALICE-Synth) | `alice-synth` | MIT OR Apache-2.0 | Procedural audio synthesis: FM, additive, subtractive, wavetable, no_std |
| [ALICE-Streaming-Protocol](https://github.com/ext-sakamoro/ALICE-Streaming-Protocol) | [![crates.io](https://img.shields.io/crates/v/libasp.svg)](https://crates.io/crates/libasp) | MIT OR Apache-2.0 | Streaming protocol and wire format for video |
| [ALICE-Audio](https://github.com/ext-sakamoro/ALICE-Audio) | `alice-audio` | MIT OR Apache-2.0 | Audio processing: FFT, filters, mixer, effects, resampling |
| [ALICE-Video](https://github.com/ext-sakamoro/ALICE-Video) | `alice-video` | MIT OR Apache-2.0 | Video processing: GOP, motion compensation, DCT, entropy coding |
| [ALICE-Camera](https://github.com/ext-sakamoro/ALICE-Camera) | `alice-camera` | MIT OR Apache-2.0 | Camera ISP: white balance, demosaicing, exposure, lens correction |

### Data & Storage

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-DB](https://github.com/ext-sakamoro/ALICE-DB) | [![crates.io](https://img.shields.io/crates/v/alice-db.svg)](https://crates.io/crates/alice-db) | AGPL-3.0-or-later OR LicenseRef-Commercial | Model-based LSM-tree database |
| [ALICE-Cache](https://github.com/ext-sakamoro/ALICE-Cache) | [![crates.io](https://img.shields.io/crates/v/alice-cache.svg)](https://crates.io/crates/alice-cache) | AGPL-3.0-or-later OR LicenseRef-Commercial | Predictive distributed cache |
| [ALICE-Queue](https://github.com/ext-sakamoro/ALICE-Queue) | `alice-queue` | AGPL-3.0 OR LicenseRef-Commercial | Deterministic zero-copy message log |
| [ALICE-Search](https://github.com/ext-sakamoro/ALICE-Search) | `alice-search` | AGPL-3.0 OR LicenseRef-Commercial | FM-index full-text search |
| [ALICE-ObjectStore](https://github.com/ext-sakamoro/ALICE-ObjectStore) | `alice-objectstore` | AGPL-3.0 OR LicenseRef-Commercial | S3-compatible object storage engine |
| [ALICE-FileSystem](https://github.com/ext-sakamoro/ALICE-FileSystem) | `alice-filesystem` | AGPL-3.0 OR LicenseRef-Commercial | Virtual filesystem: inodes, permissions, symlinks, mounting |
| [ALICE-Analytics](https://github.com/ext-sakamoro/ALICE-Analytics) | [![crates.io](https://img.shields.io/crates/v/alice-analytics.svg)](https://crates.io/crates/alice-analytics) | MIT OR Apache-2.0 | Telemetry and statistical estimation with probabilistic data structures |
| [ALICE-History](https://github.com/ext-sakamoro/ALICE-History) | `alice-history` | AGPL-3.0-or-later OR LicenseRef-Commercial | Restoration of degraded historical data |

### Networking

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-HTTP](https://github.com/ext-sakamoro/ALICE-HTTP) | [![crates.io](https://img.shields.io/crates/v/alice-http.svg)](https://crates.io/crates/alice-http) | AGPL-3.0-or-later OR LicenseRef-Commercial | HTTP/1.1 and HTTP/2 parser and framework |
| [ALICE-gRPC](https://github.com/ext-sakamoro/ALICE-gRPC) | [![crates.io](https://img.shields.io/crates/v/alice-grpc.svg)](https://crates.io/crates/alice-grpc) | AGPL-3.0-or-later OR LicenseRef-Commercial | gRPC framework with Protobuf encoding |
| [ALICE-WebSocket](https://github.com/ext-sakamoro/ALICE-WebSocket) | [![crates.io](https://img.shields.io/crates/v/alice-websocket.svg)](https://crates.io/crates/alice-websocket) | AGPL-3.0-or-later OR LicenseRef-Commercial | WebSocket protocol: framing, masking, handshake, fragmentation |
| [ALICE-API](https://github.com/ext-sakamoro/ALICE-API) | `alice-api` | AGPL-3.0 OR LicenseRef-Commercial | API gateway with distributed rate limiting |
| [ALICE-Proxy](https://github.com/ext-sakamoro/ALICE-Proxy) | `alice-proxy` | AGPL-3.0 OR LicenseRef-Commercial | L7 reverse proxy: routing, header rewriting, load balancing, circuit breaker |
| [ALICE-CDN](https://github.com/ext-sakamoro/ALICE-CDN) | `alice-cdn` | AGPL-3.0 OR LicenseRef-Commercial | Latency-aware CDN with Vivaldi coordinates and Maglev hashing |
| [ALICE-DNS](https://github.com/ext-sakamoro/ALICE-DNS) | `alice-dns` | AGPL-3.0 OR LicenseRef-Commercial | Bloom-filter DNS blocker |
| [ALICE-Cloud-Gateway](https://github.com/ext-sakamoro/ALICE-Cloud-Gateway) | `alice-cloud-gateway` | AGPL-3.0 OR LicenseRef-Commercial | QUIC gateway: packet ingest, device keys, telemetry |
| [ALICE-Sync](https://github.com/ext-sakamoro/ALICE-Sync) | [![crates.io](https://img.shields.io/crates/v/alice-sync.svg)](https://crates.io/crates/alice-sync) | AGPL-3.0-or-later OR LicenseRef-Commercial | Peer-to-peer synchronization by event diffing |
| [ALICE-Bridge](https://github.com/ext-sakamoro/ALICE-Bridge) | [![crates.io](https://img.shields.io/crates/v/alice-bridge.svg)](https://crates.io/crates/alice-bridge) | AGPL-3.0-or-later OR LicenseRef-Commercial | Protocol-agnostic hardware communication layer |
| [ALICE-BLE](https://github.com/ext-sakamoro/ALICE-BLE) | [![crates.io](https://img.shields.io/crates/v/alice-ble.svg)](https://crates.io/crates/alice-ble) | MIT OR Apache-2.0 | BLE stack: GATT, advertising, pairing, ATT, L2CAP |
| [ALICE-LoRa](https://github.com/ext-sakamoro/ALICE-LoRa) | [![crates.io](https://img.shields.io/crates/v/alice-lora.svg)](https://crates.io/crates/alice-lora) | MIT OR Apache-2.0 | LoRaWAN: ADR, OTAA / ABP, MAC commands, device classes |
| [ALICE-NFC](https://github.com/ext-sakamoro/ALICE-NFC) | `alice-nfc` | MIT OR Apache-2.0 | NFC: NDEF, tag types 1-4, APDU, card emulation |

### Security

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-Auth](https://github.com/ext-sakamoro/ALICE-Auth) | [![crates.io](https://img.shields.io/crates/v/alice-auth.svg)](https://crates.io/crates/alice-auth) | AGPL-3.0-or-later OR LicenseRef-Commercial | Ed25519 and zero-knowledge proof authentication |
| [ALICE-Crypto](https://github.com/ext-sakamoro/ALICE-Crypto) | [![crates.io](https://img.shields.io/crates/v/alice-crypto.svg)](https://crates.io/crates/alice-crypto) | AGPL-3.0-or-later OR LicenseRef-Commercial | Information-theoretic security primitives |
| [ALICE-Audit](https://github.com/ext-sakamoro/ALICE-Audit) | `alice-audit` | AGPL-3.0 OR LicenseRef-Commercial | Hash-chained, signed audit trail with Merkle anchoring |
| [ALICE-DLP](https://github.com/ext-sakamoro/ALICE-DLP) | `alice-dlp` | AGPL-3.0 OR LicenseRef-Commercial | Data loss prevention: PII detection, classification, masking |
| [ALICE-WAF](https://github.com/ext-sakamoro/ALICE-WAF) | `alice-waf` | AGPL-3.0 OR LicenseRef-Commercial | Web application firewall: rule engine, SQLi / XSS detection, rate limiting |
| [ALICE-Presence](https://github.com/ext-sakamoro/ALICE-Presence) | `alice-presence` | MIT OR Apache-2.0 | Proof of encounter with Vivaldi coordinates and zero-knowledge proofs |

### Systems & Tooling

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-SIMD](https://github.com/ext-sakamoro/ALICE-SIMD) | `alice-simd` | MIT OR Apache-2.0 | SIMD, branchless and fast-math primitives, no_std |
| [ALICE-RTOS](https://github.com/ext-sakamoro/ALICE-RTOS) | `alice-rtos` | AGPL-3.0 OR LicenseRef-Commercial | Real-time OS kernel with rate-monotonic scheduling, no_std |
| [ALICE-RTOS-BSP](https://github.com/ext-sakamoro/ALICE-RTOS-BSP) | workspace | MIT OR Apache-2.0 | Board support packages for ALICE-RTOS on ESP32 and Cortex-M boards |
| [ALICE-Container](https://github.com/ext-sakamoro/ALICE-Container) | `alice-container` | AGPL-3.0 OR LicenseRef-Commercial | Container runtime with direct cgroup v2 and namespace control |
| [ALICE-VM](https://github.com/ext-sakamoro/ALICE-VM) | `alice-vm` | MIT OR Apache-2.0 | Bytecode virtual machine |
| [ALICE-Compiler](https://github.com/ext-sakamoro/ALICE-Compiler) | `alice-compiler` | MIT OR Apache-2.0 | DSL / JIT compiler infrastructure: AST, IR, code generation, optimization passes |
| [ALICE-Parser](https://github.com/ext-sakamoro/ALICE-Parser) | `alice-parser` | MIT OR Apache-2.0 | Parser combinators: PEG, Pratt, recursive descent, error recovery |
| [ALICE-VCS](https://github.com/ext-sakamoro/ALICE-VCS) | `alice-vcs` | AGPL-3.0 OR LicenseRef-Commercial | AST-level version control: tree diff, 3-way merge, no_std |
| [ALICE-Terraform](https://github.com/ext-sakamoro/ALICE-Terraform) | `alice-terraform` | AGPL-3.0 OR LicenseRef-Commercial | Infrastructure-as-code engine: resource graph, state, plan / apply |
| [ALICE-Monitor](https://github.com/ext-sakamoro/ALICE-Monitor) | `alice-monitor` | AGPL-3.0 OR LicenseRef-Commercial | Monitoring: health checks, alerts, SLA tracking, heartbeats |
| [ALICE-Signal](https://github.com/ext-sakamoro/ALICE-Signal) | [![crates.io](https://img.shields.io/crates/v/alice-signal.svg)](https://crates.io/crates/alice-signal) | MIT OR Apache-2.0 | Digital signal processing: FFT, FIR / IIR, wavelets, PSD |

### Finance

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-FIX](https://github.com/ext-sakamoro/ALICE-FIX) | `alice-fix` | MIT OR Apache-2.0 | FIX 4.4 / 5.0 message parser, builder and session management |
| [ALICE-Ledger](https://github.com/ext-sakamoro/ALICE-Ledger) | `alice-ledger` | AGPL-3.0-only OR LicenseRef-Commercial | Order book, matching engine, position management |
| [ALICE-Risk](https://github.com/ext-sakamoro/ALICE-Risk) | `alice-risk` | AGPL-3.0-only OR LicenseRef-Commercial | Pre-trade risk checks, margin, circuit breakers |
| [ALICE-Settlement](https://github.com/ext-sakamoro/ALICE-Settlement) | `alice-settlement` | AGPL-3.0-only OR LicenseRef-Commercial | Post-trade settlement, netting and clearing |

### Business

| Repository | Crate | License | Description |
|---|---|---|---|
| [ALICE-CRM](https://github.com/ext-sakamoro/ALICE-CRM) | `alice-crm` | AGPL-3.0 OR LicenseRef-Commercial | Customer relationship management: pipeline, lead scoring, segmentation |
| [ALICE-ERP](https://github.com/ext-sakamoro/ALICE-ERP) | `alice-erp` | AGPL-3.0 OR LicenseRef-Commercial | Resource planning: inventory, BOM, MRP, scheduling, cost accounting |
| [ALICE-HRM](https://github.com/ext-sakamoro/ALICE-HRM) | `alice-hrm` | AGPL-3.0 OR LicenseRef-Commercial | Human resource management: attendance, payroll, leave, shifts |
| [ALICE-LMS](https://github.com/ext-sakamoro/ALICE-LMS) | `alice-lms` | MIT OR Apache-2.0 | Learning management: courses, quizzes, grading, certificates |
| [ALICE-Legal](https://github.com/ext-sakamoro/ALICE-Legal) | `alice-legal` | MIT OR Apache-2.0 | Statutes and contracts as deterministic ASTs with audit trails |

<!-- crate-index:end -->

## Licensing model

Crates use one of two licensing models, shown per crate in the index:

- **Permissive**: `MIT OR Apache-2.0`
- **Copyleft with a commercial option**: `AGPL-3.0-*` or `MIT`, each
  `OR LicenseRef-Commercial`. The commercial terms are in each repository's
  `LICENSE-COMMERCIAL.md`.

The license column is the SPDX expression from the crate's `Cargo.toml`.

## Building and checking

```sh
cargo fmt -- --check                    # formatting (CI)
python3 scripts/readme_index.py --check # README index matches docs/crate-index.tsv (CI)
python3 scripts/readme_index.py --online  # TSV matches the repositories (needs network)
python3 scripts/docs_lint.py --check    # vocabulary and CHANGELOG structure (CI)
scripts/preflight.sh                    # the CI steps, locally
```

After editing `docs/crate-index.tsv`, regenerate the tables with
`python3 scripts/readme_index.py --write`.

## License

The hub crate in this repository is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE](LICENSE))

at your option. Each crate in the index carries its own license, shown in the
table.
