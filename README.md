# ALICE Ecosystem

**The Complete Edge-to-Cloud Data Pipeline with GPU Visualization**

> "Don't send data. Send the law."

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          ALICE Ecosystem (35 Components)                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─── Compression ───┐  ┌─── Data ────┐  ┌─── Network ───┐  ┌ Security ─┐ │
│  │ Edge   Zip  Codec │  │ DB    Cache │  │ API    CDN    │  │ Auth      │ │
│  │ Voice  Text  SDF  │  │ Queue Search│  │ DNS  Streaming│  │ Crypto    │ │
│  │ Synth  Font       │  │            │  │ Sync Cloud-GW │  │           │ │
│  └───────────────────┘  └────────────┘  └───────────────┘  └───────────┘ │
│                                                                             │
│  ┌──── Compute ──────┐  ┌─── Analytics ──┐  ┌─── Application ────────┐  │
│  │ Container  ML     │  │ Analytics      │  │ Browser  Print         │  │
│  │ Physics    TRT    │  │ View           │  │ Animation  Manga       │  │
│  │ RTOS              │  └────────────────┘  │ Eco-System             │  │
│  └───────────────────┘                       └────────────────────────┘  │
│                                                                             │
│  ┌──── Motion & VCS ─┐                                                     │
│  │ Motion  VCS      │                                                     │
│  │ Kinematics       │                                                     │
│  └───────────────────┘                                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## What is ALICE?

ALICE (**A**daptive **L**ightweight **I**ntelligent **C**ompression **E**ngine) is an ecosystem of libraries that work together to achieve extreme data compression by storing mathematical models instead of raw data.

### Compression & Encoding

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Edge](https://github.com/ext-sakamoro/ALICE-Edge) | v0.1.0 | Embedded Model Generator | 500x compression, 751ns/1K samples, sensors, MQTT, dashboard | MIT (Core) |
| [ALICE-Zip](https://github.com/ext-sakamoro/ALICE-Zip) | v1.0.0 | Procedural Generation Compression | 10-1000x for patterns, LZMA fallback | Open Core (MIT core) |
| [ALICE-Codec](https://github.com/ext-sakamoro/ALICE-Codec) | v0.1.0 | 3D Wavelet Video/Audio Codec | CDF 9/7 Wavelet, rANS entropy coding | AGPL-3.0 |
| [ALICE-Voice](https://github.com/ext-sakamoro/ALICE-Voice) | v0.1.0 | Voice Procedural Codec | LPC parametric 100-600x, privacy-preserving | MIT |
| [ALICE-Text](https://github.com/ext-sakamoro/ALICE-Text) | v1.0.0 | Exception-Based Text Compression | Pattern recognition, columnar encoding | BSL 1.1 (→MIT 2028) |
| [ALICE-SDF](https://github.com/ext-sakamoro/ALICE-SDF) | v0.1.0 | 3D Signed Distance Functions | 10-1000x, infinite resolution, CSG ops | MIT |
| [ALICE-Synth](https://github.com/ext-sakamoro/ALICE-Synth) | v0.1.0 | Procedural Audio Synthesis | FM/Additive/Subtractive/Wavetable, 64-voice polyphony, no_std | MIT |
| [ALICE-Font](https://github.com/ext-sakamoro/ALICE-Font) | v0.1.0 | Parametric MetaFont Renderer | 40-byte params → SDF glyphs, variable-width pen, LRU atlas, no_std | MIT |

### Data & Storage

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-DB](https://github.com/ext-sakamoro/ALICE-DB) | v0.1.0 | Model-Based LSM-Tree Database | O(1) point queries, 50-1000x compression | Open Core (MIT core + BSL server) |
| [ALICE-Cache](https://github.com/ext-sakamoro/ALICE-Cache) | v0.2.0 | Predictive Distributed Cache | Slab alloc, TinyLFU, Markov prediction | AGPL-3.0 |
| [ALICE-Queue](https://github.com/ext-sakamoro/ALICE-Queue) | v0.1.0 | Deterministic Zero-Copy Message Log | Lock-free SPSC, mmap WAL, Vector Clock | AGPL-3.0 |
| [ALICE-Search](https://github.com/ext-sakamoro/ALICE-Search) | v0.1.0 | FM-Index Full-Text Search | Wavelet Matrix, backward search, ~1.0x size | AGPL-3.0 |

### Networking & Infrastructure

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-API](https://github.com/ext-sakamoro/ALICE-API) | v0.1.0 | API Gateway with Distributed Rate Limiting | GCRA lock-free, SFQ, zero-copy splice | AGPL-3.0 |
| [ALICE-CDN](https://github.com/ext-sakamoro/ALICE-CDN) | v0.2.0 | Decentralized Content Delivery | Vivaldi coordinates, SIMD, Maglev hashing | AGPL-3.0 |
| [ALICE-Streaming-Protocol](https://github.com/ext-sakamoro/ALICE-Streaming-Protocol) | v1.0.0 | High-Performance Video Streaming Codec | FlatBuffers, motion estimation, SIMD, **media-stack** (Codec+Voice) | MIT |
| [ALICE-Sync](https://github.com/ext-sakamoro/ALICE-Sync) | v0.6.0 | P2P Synchronization via Event Diffing | 18-byte events, bit-exact determinism, Lockstep/Rollback, PyO3 | AGPL-3.0 |
| [ALICE-Cloud-Gateway](https://github.com/ext-sakamoro/ALICE-Cloud-Gateway) | v0.1.0 | Edge-to-Cloud SDF Ingest Gateway | ASP decrypt, BLAKE3 KDF, DDSketch/HLL telemetry | AGPL-3.0 |
| [ALICE-DNS](https://github.com/ext-sakamoro/ALICE-DNS) | v0.1.0 | Bloom Filter DNS Ad-Blocker | 453KB binary, O(1) lookup, Pi-hole replacement | AGPL-3.0 |

### Security & Cryptography

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Auth](https://github.com/ext-sakamoro/ALICE-Auth) | v0.4.0 | Cryptographic Authentication | Ed25519, Zero-Knowledge Proofs | AGPL-3.0 |
| [ALICE-Crypto](https://github.com/ext-sakamoro/ALICE-Crypto) | v0.1.0 | Information-Theoretic Security | Shamir SSS, BLAKE3, XChaCha20-Poly1305 | AGPL-3.0 |

### Compute & Runtime

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Container](https://github.com/ext-sakamoro/ALICE-Container) | v0.2.0 | Minimal Container Runtime | Direct cgroup v2, io_uring, clone3, PSI | AGPL-3.0 |
| [ALICE-ML](https://github.com/ext-sakamoro/ALICE-ML) | v0.1.0 | 1.58-bit Ternary Inference Engine | {-1,0,+1} only, 16x compression, no multiply | AGPL-3.0 |
| [ALICE-TRT](https://github.com/ext-sakamoro/ALICE-TRT) | v0.1.0 | GPU Ternary Inference Engine | wgpu/CUDA, BitNet, GPU-accelerated matmul | AGPL-3.0 |
| [ALICE-Physics](https://github.com/ext-sakamoro/ALICE-Physics) | v0.3.0 | Deterministic 128-bit Physics Engine | I64F64, CORDIC, XPBD, GJK/EPA, BVH, Netcode, PyO3 | AGPL-3.0 |
| [ALICE-RTOS](https://github.com/ext-sakamoro/ALICE-RTOS) | v0.1.0 | Math-First Real-Time OS | RMS scheduler, Liu-Layland analysis, SPSC ring, < 2KB kernel | AGPL-3.0 |

### Motion & Version Control

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Motion](https://github.com/ext-sakamoro/ALICE-Motion) | v0.1.0 | NURBS/Bezier Trajectory Control | Cox-de Boor, de Casteljau, trapezoidal/S-curve profiles, no_std | MIT |
| [ALICE-VCS](https://github.com/ext-sakamoro/ALICE-VCS) | v0.1.0 | AST Semantic Version Control | Tree diff, 3-way merge, content-addressed snapshots, FNV-1a Merkle | AGPL-3.0 |
| [ALICE-Kinematics](https://github.com/ext-sakamoro/ALICE-Kinematics) | v0.1.0 | Human Motion Intent Compression | 7-DoF arm, jerk minimization, 8-byte intent packets, 10,000x compression | Open Core (MIT decoder) |

### Analytics & Visualization

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Analytics](https://github.com/ext-sakamoro/ALICE-Analytics) | v0.1.0 | Streaming Telemetry & Statistics | HyperLogLog++, DDSketch, CMS, LDP | AGPL-3.0 |
| [ALICE-View](https://github.com/ext-sakamoro/ALICE-View) | v0.2.0 | Infinite Canvas GPU Renderer | wgpu procedural rendering, 60 FPS | MIT |

### Application

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Browser](https://github.com/ext-sakamoro/ALICE-Browser) | v0.2.0 | Semantic Browser | SDF rendering, ML filtering, predictive cache | MIT OR Apache-2.0 |
| [ALICE-Print](https://github.com/ext-sakamoro/ALICE-Print) | v0.1.0 | Direct SDF-to-G-code Slicer | SIMD 8-wide Marching Squares, O(n) contour, Bambu .3mf | Proprietary |
| [ALICE-Animation](https://github.com/ext-sakamoro/ALICE-Animation) | v0.1.0 | Anime SDF Direction Engine | SceneGraph, Director/Cut, NPR cel-shading, fake perspective, 20-50KB episodes | Proprietary |
| [ALICE-Manga](https://github.com/ext-sakamoro/ALICE-Manga) | v0.1.0 | SDF Manga Creation Engine | Bezier strokes, screentone (moire-free), balloon/panel SDF, ASDF export, 2-10KB/page | Proprietary |

### Integration

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Eco-System](https://github.com/ext-sakamoro/ALICE-Eco-System) | v0.1.0 | Ecosystem Integration Demo | Edge → Streaming → DB → View pipeline | MIT |

**Total: 35 components** | AGPL-3.0: 18 | MIT: 8 | MIT (Core): 1 | MIT/Apache-2.0: 1 | BSL 1.1: 1 | Open Core: 3 | Proprietary: 3

## Quick Start

```bash
# Clone the ecosystem demo
git clone https://github.com/ext-sakamoro/ALICE-Eco-System.git
cd ALICE-Eco-System

# Run the integration demo (Edge → DB → View)
cargo run

# Run the SDF asset delivery demo (SDF → CDN → Cache)
cargo run --example sdf_delivery

# Run the game engine pipeline demo (SDF → CDN → Physics → Sync → DB)
cargo run --example game_pipeline

# Run with GPU visualization
cargo run -- --view
```

## Demo: Edge-to-Cloud Pipeline

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Sensor    │────▶│ ALICE-Edge  │────▶│  Network    │────▶│  ALICE-DB   │────▶│ ALICE-View  │
│  1000 pts   │     │  8 bytes    │     │  8 bytes    │     │   Query     │     │    GPU      │
│  (4000 B)   │     │  (500x)     │     │  (LoRaWAN)  │     │  Reconstruct│     │  Rendering  │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
```

### Demo Output

```
╔══════════════════════════════════════════════════════════════╗
║         ALICE ECOSYSTEM INTEGRATION DEMO                      ║
╚══════════════════════════════════════════════════════════════╝

━━━ PHASE 1: Sensor Data Generation (Stack Allocated) ━━━
  Sensor readings: 1000 samples
  Raw data size:   4000 bytes

━━━ PHASE 2: ALICE-Edge Compression (Ultimate) ━━━
  Model: y = slope × x + intercept
  Packet size:  8 bytes
  ┌─────────────────────────────────────────────────┐
  │ COMPRESSION: 4000 bytes → 8 bytes               │
  │ RATIO:       500x                               │
  └─────────────────────────────────────────────────┘

━━━ PHASE 3: Network Transmission ━━━
  [EDGE DEVICE] ──── 8 bytes ────▶ [CLOUD SERVER]

━━━ PHASE 4: ALICE-DB Storage (Batch Insert) ━━━
  Compression:  ~50x additional

━━━ PHASE 5: Query & Verification ━━━
  ✓ Point queries accurate to 0.0001°C
  ✓ Aggregations (AVG, MIN, MAX) working

━━━ PHASE 6: ALICE-View Visualization ━━━
  GPU-accelerated procedural rendering available!
  Run with --view to launch the visualization window

╔══════════════════════════════════════════════════════════════╗
║  [ALICE-View] (GPU Procedural Rendering)                     ║
║     └─ wgpu + egui, infinite zoom, X-Ray mode                ║
╠══════════════════════════════════════════════════════════════╣
║  TOTAL: 4000 bytes → 8 bytes → ~100 bytes                    ║
║  BANDWIDTH SAVED: 99.8%                                      ║
╚══════════════════════════════════════════════════════════════╝
```

## Demo: SDF Asset Delivery Pipeline

ALICE-SDF + ALICE-CDN + ALICE-Cache combine to deliver 3D assets as mathematical descriptions instead of polygon meshes, achieving **200-800x bandwidth reduction** vs glTF.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Client    │────▶│ ALICE-CDN   │────▶│ ALICE-Cache │────▶│ ALICE-SDF   │
│  Request    │     │  Vivaldi    │     │  Markov     │     │  ASDF       │
│  (asset_id) │     │  Routing    │     │  Prefetch   │     │  38 bytes   │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                    O(log n + k)         lock-free            vs glTF 15 KB
                    nearest node         prediction           = 395x
```

| Asset Type | glTF Size | ASDF Size | Ratio |
|------------|-----------|-----------|-------|
| Sphere | ~15 KB | **38 bytes** | **395x** |
| CSG (union+subtract) | ~200 KB | **58 bytes** | **3,448x** |
| Complex scene (100 nodes) | ~2 MB | **398 bytes** | **5,025x** |

## Demo: Full Game Engine Pipeline (6 Crates)

The `game_pipeline` example demonstrates cross-crate integration across 6 ALICE components:

```
[ALICE-SDF]        Create world geometry (ASDF binary)
     ↓
[ALICE-CDN]        Type-aware content routing (ASDF detection)
     ↓
[ALICE-Physics]    Deterministic simulation (128-bit fixed-point)
     ↓
[ALICE-Sync]       Input synchronization (Lockstep)
     ↓
[ALICE-DB]         Replay recording + Telemetry (model-based compression)
```

Cross-crate bridges demonstrated:
- **SDF → Physics** — `physics_bridge` (`impl SdfField for CompiledSdf`)
- **Physics → DB** — `replay.rs` (trajectory compression)
- **Sync → DB** — `telemetry.rs` (metric time-series)
- **CDN ← SDF** — `content_types` (ASDF detection)
- **Sync → Physics** — `physics_bridge` (`PhysicsRollbackSession`)

```bash
cargo run --example game_pipeline
```

### Cross-Crate Bridge Matrix

The ALICE ecosystem contains **103+ cross-crate bridges** connecting 33 components. Key bridge categories:

| Category | Bridges | Description |
|----------|---------|-------------|
| **Data Storage** | Cache↔Analytics, Queue→Text, Container→DB, Auth→DB, TRT→DB, Print→DB, Animation→DB, Manga→DB | Persistence and metrics |
| **Security** | Auth↔Crypto, Container→Crypto, Auth→API | Encryption, signing, secrets |
| **Synchronization** | Sync↔Cache, Container→Sync, Cloud-Gateway→Container | Distributed state |
| **Media** | Voice→Text, TRT→Voice, Browser→Voice, Animation→Voice | Audio/speech processing |
| **Content Delivery** | Browser→CDN, Browser→SDF, Browser→View, Animation→CDN, Manga→CDN | Routing and rendering |
| **Anime Pipeline** | Animation→SDF, Animation→Codec, Animation→Cache, Animation→Browser, Animation→ML, Animation→Streaming | Anime production & distribution |
| **Manga Pipeline** | Manga→SDF, Manga→Print, Manga→Codec, Manga→Cache, Manga→Browser, Manga→Search, Manga→Text | Manga creation & distribution |
| **Search & Analytics** | Text→Search, Browser→Search, Browser→Analytics, Print→Analytics | Indexing and telemetry |
| **Orchestration** | Cloud-Gateway→Queue, Cloud-Gateway→Container | Message routing and deploy |

### Build Profile Changes

- `[profile.release]`: Added complete release profile section (LTO, codegen-units, strip)
- `[profile.bench]`: Standardized bench profile added across ecosystem crates

## Demo: Game Engine Networking

ALICE-Sync + ALICE-Physics combine for deterministic multiplayer game networking. Only player inputs (~24 bytes) are synchronized — physics state is never transmitted.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Player    │────▶│ ALICE-Sync  │────▶│ALICE-Physics│────▶│ ALICE-View  │
│  InputFrame │     │  Rollback   │     │  Fix128     │     │  wgpu       │
│  (24 bytes) │     │  Lockstep   │     │  XPBD Step  │     │  Rendering  │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                    InputFrame(i16)     FrameInput(Fix128)
                    ──── bridge ────▶
                    SimChecksum(u64) ◀── WorldHash(u64)
```

| Metric | State Sync | ALICE Input Sync |
|--------|-----------|-----------------|
| Bandwidth (4p, 60fps) | ~960 KB/s | **5.6 KB/s** |
| Determinism | Approximate | **Bit-exact** |
| Rollback | Full state transfer | **24-byte input replay** |

The `physics_bridge` module (feature `physics`) provides:
- `sync_input_to_physics()` / `physics_input_to_sync()` — InputFrame (i16) ↔ FrameInput (Fix128)
- `physics_checksum_to_world_hash()` — Desync verification
- `PhysicsRollbackSession` — Combined rollback sync + deterministic physics

## Demo: Data Pipeline

ALICE-Queue + ALICE-Analytics + ALICE-DB combine for IoT/log collection with streaming aggregation and model-based persistent storage.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ Sensor/App  │────▶│ ALICE-Queue │────▶│  Analytics  │────▶│  ALICE-DB   │
│  MetricEvent│     │ SPSC + WAL  │     │ HLL,DDSketch│     │ LSM-Tree    │
│  (17 bytes) │     │ Exactly-once│     │  Streaming  │     │ Model-Based │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                    Lock-free O(1)      Counter, Gauge       O(1) point query
                    mmap persistence    P50/P90/P99          50-1000x compress
```

| Metric | Raw Logging | ALICE Data Pipeline |
|--------|------------|---------------------|
| Per-event size | ~200 bytes (JSON) | **17 bytes** (binary) |
| Storage (1M events) | ~200 MB | **~6 entries/metric** (aggregated) |
| Query latency | O(N) scan | **O(1)** model compute |
| Privacy | Full raw data | **Only aggregates stored** |

The `queue_bridge` module (feature `queue` in ALICE-Analytics) and `analytics_bridge` module (feature `analytics` in ALICE-DB) provide:
- `encode_metric_payload()` / `parse_metric_event()` — MetricEvent ↔ 17-byte queue payload
- `QueueConsumerPipeline` — Combined queue drain + streaming aggregation
- `flush_metrics_to_db()` — Persist pipeline slots to model-based DB
- `AnalyticsSink` — Combined MetricPipeline + AliceDB with windowed flush

## Demo: Media Streaming Pipeline

ALICE-Streaming-Protocol + ALICE-Codec + ALICE-Voice combine as a unified **media-stack** for ultra-low bandwidth video+voice streaming.

```
┌─────────────┐     ┌──────────────────┐     ┌──────────────────┐     ┌─────────────┐
│   Camera    │────▶│ ALICE-Codec      │────▶│ ALICE-Streaming  │────▶│  Receiver   │
│  RGB Frame  │     │ YCoCg-R+Wavelet  │     │  Protocol (ASP)  │     │  Decode +   │
│  (6.2 MB)   │     │ +rANS  (~50 KB)  │     │  FlatBuffers     │     │  Display    │
└─────────────┘     └──────────────────┘     └──────────────────┘     └─────────────┘

┌─────────────┐     ┌──────────────────┐     ┌──────────────────┐     ┌─────────────┐
│ Microphone  │────▶│ ALICE-Voice      │────▶│ ALICE-Streaming  │────▶│  Receiver   │
│  PCM 16kHz  │     │ LPC Parametric   │     │  Protocol (ASP)  │     │  Synthesize │
│  (32 KB/s)  │     │  (~50 bytes/frm) │     │  FlatBuffers     │     │  + Playback │
└─────────────┘     └──────────────────┘     └──────────────────┘     └─────────────┘
```

| Stream | Traditional | ALICE Media Stack |
|--------|-----------|-------------------|
| Video (1080p) | 5-10 Mbps (H.265) | **~0.5-2 Mbps** (Wavelet+rANS) |
| Voice | 32 KB/s (PCM) | **~50 bytes/frame** (LPC, 600x) |
| Combined | ~5-10 Mbps | **~0.5-2 Mbps** |

Enable with: `libasp = { features = ["media-stack"] }`

Key optimizations:
- **Rayon parallel 3-channel** video encode/decode (Y/Co/Cg via `rayon::join`)
- **Voice batch API** for multi-frame processing
- **Python bindings** with GIL release + NumPy zero-copy

## Demo: Edge-to-Cloud AR Pipeline (7 Crates)

ALICE-Cloud-Gateway orchestrates the full edge-to-cloud SDF streaming pipeline, connecting 7 ALICE crates for real-time AR data delivery.

```
┌──────── Edge (Raspberry Pi 5) ────────┐
│                                        │
│  Dolphin D5 Lite (USB 3.0)            │
│       │                                │
│  [ALICE-Edge]    depth → SDF compress  │
│  [ALICE-ML]      1.58-bit classify     │
│  [ALICE-Streaming-Protocol] ASP packet │
│  [ALICE-Crypto]  seal_packet()         │
│       │                                │
└───────┼────────────────────────────────┘
        │ QUIC/UDP
        ▼
┌──────── Cloud (ALICE-Cloud-Gateway) ───┐
│                                        │
│  IngestPipeline::process_packet()      │
│       │                                │
│       ├─→ ALICE-Crypto   decrypt       │
│       ├─→ ALICE-DB       SDF storage   │
│       ├─→ ALICE-Cache    hot frames    │
│       ├─→ ALICE-Sync     device sync   │
│       ├─→ ALICE-CDN      edge routing  │
│       └─→ ALICE-Analytics telemetry    │
└────────────────────────────────────────┘
```

| Stage | Data Size | Compression |
|-------|-----------|-------------|
| Raw point cloud (100K pts) | 4.8 MB | — |
| SDF primitives (CSG) | 200-600 B | **8,000-24,000x** |
| SDF SVO chunks | 2-50 KB | **96-2,400x** |
| ASP packet (encrypted) | +40 B overhead | negligible |

Cross-crate bridges:
- **Edge → Crypto** — `seal_packet()` per-device stream encryption (BLAKE3 KDF)
- **Gateway → DB** — `SdfStorage::store_keyframe()` Morton code spatial indexing
- **Gateway → Sync** — `CloudSyncHub::process_device_update()` star topology
- **Gateway → CDN** — `SdfCdnRouter::route_sdf_request()` Maglev + Vivaldi
- **Gateway → Analytics** — `GatewayTelemetry::record_packet()` DDSketch/HLL/CMS

## Demo: SDF-to-Print Pipeline (3 Crates)

ALICE-SDF + ALICE-Print combine to skip the traditional mesh → STL → slicer pipeline entirely. SDF nodes are sliced directly into G-code toolpaths using SIMD Marching Squares, with optional Bambu Lab .3mf packaging.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  SDF Node   │────▶│ ALICE-Print │────▶│   G-code    │────▶│  Printer    │
│  CSG tree   │     │  Slicer     │     │  Marlin /   │     │  Bambu Lab  │
│  (38 bytes) │     │  SIMD 8-wide│     │  Klipper    │     │  or FDM     │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                    CompiledSdf          154 KB (sphere)
                    → Z-slice (Rayon)    layer-by-layer
                    → Marching Squares
                    → O(n) Contour
                    → Toolpath → Gcode
```

| Shape | Traditional Pipeline | ALICE Direct Pipeline | Speedup |
|-------|---------------------|----------------------|---------|
| Sphere 15mm | SDF → Mesh (MC) → STL → PrusaSlicer | SDF → G-code (2.2ms) | **No intermediate mesh** |
| Box 60x40x30 | Mesh → STL export → Import → Slice | SDF → G-code (direct) | **Zero file I/O** |
| CSG subtract | Boolean mesh ops → Repair → Slice | SDF → G-code (native CSG) | **No mesh repair** |

Cross-crate bridges:
- **SDF → Print** — `CompiledSdf` bytecode VM + `eval_compiled_batch_simd()` for Z-slice evaluation
- **Print → .3mf** — `pack_bambu_3mf()` ZIP packaging for Bambu Lab printers

### Cloud-to-Print: Remote Fabrication Pipeline (4 Crates)

ALICE-Cloud-Gateway can route SDF scenes from edge devices to ALICE-Print for remote 3D printing, enabling cloud-based digital fabrication.

```
┌──────── Edge ─────────┐     ┌──────── Cloud ─────────┐     ┌─── Fabrication ───┐
│                       │     │                        │     │                   │
│  [ALICE-Edge]         │     │  [ALICE-Cloud-Gateway] │     │  [ALICE-Print]    │
│  3D scan → SDF        │────▶│  IngestPipeline        │────▶│  SDF → G-code     │
│  [ALICE-Crypto]       │     │  → decrypt + store     │     │  → .3mf / .gcode  │
│  seal_packet()        │     │  → ALICE-DB persist    │     │  → Bambu Lab H2S  │
│                       │     │  → route to printer    │     │                   │
└───────────────────────┘     └────────────────────────┘     └───────────────────┘
```

This enables **scan-to-print**: a Raspberry Pi with a 3D scanner captures an object as SDF, streams it through the cloud gateway, and a remote ALICE-Print instance generates G-code for fabrication — all without ever creating a polygon mesh.

## Demo: Container Orchestration Pipeline (5 Crates)

ALICE-Container + ALICE-Cloud-Gateway + ALICE-Sync + ALICE-Crypto + ALICE-DB combine for secure, synchronized container orchestration with audit logging.

```
┌─────────────┐     ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│  Container  │────▶│ ALICE-Sync  │────▶│ALICE-Cloud-GW│────▶│  ALICE-DB   │
│  Runtime    │     │  Sync Event │     │  Orchestrate │     │  Audit Log  │
│  cgroup v2  │     │  (18 bytes) │     │  Deploy/Scale│     │  (40 bytes) │
└─────────────┘     └─────────────┘     └──────────────┘     └─────────────┘
   │                                           │
   ▼                                           ▼
┌─────────────┐                        ┌──────────────┐
│ALICE-Crypto │                        │ALICE-Container│
│ seal/open   │                        │ queue_bridge  │
│ XChaCha20   │                        │ Priority route│
└─────────────┘                        └──────────────┘
```

Cross-crate bridges:
- **Container → DB** — `ContainerRecord` 40-byte serialization + `ContainerDbSink`
- **Container → Crypto** — `ContainerSecretStore` (XChaCha20-Poly1305 secret management)
- **Container → Sync** — `ContainerSyncEvent` 18-byte compact sync events + `container_world_hash()`
- **Cloud-Gateway → Queue** — `GatewayRouter` priority message routing
- **Cloud-Gateway → Container** — `ContainerOrchestrator` deploy/scale/health_check

## Demo: Compressed Log SIEM Pipeline (4 Crates)

ALICE-Queue + ALICE-Text + ALICE-Search + ALICE-DB combine for a compressed log ingestion pipeline where logs are exception-compressed, full-text indexed, and stored in model-based DB.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Log Source  │────▶│ ALICE-Queue │────▶│ ALICE-Text  │────▶│ALICE-Search │
│  Raw logs   │     │ text_bridge │     │ search_bridge│     │  FM-Index   │
│  (~200 B/ev)│     │ Batch+Compr.│     │ Compress+Idx│     │  Backward   │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                                                                    │
                                                                    ▼
                                                             ┌─────────────┐
                                                             │  ALICE-DB   │
                                                             │  LSM-Tree   │
                                                             │  50-1000x   │
                                                             └─────────────┘
```

| Metric | Traditional SIEM | ALICE Log Pipeline |
|--------|-----------------|-------------------|
| Per-event storage | ~200 bytes (JSON) | **~20 bytes** (exception-compressed) |
| Full-text search | Elasticsearch (GB RAM) | **FM-Index** (~1.0x compressed size) |
| Storage (1M logs) | ~200 MB | **~20 MB** (compressed + model-based) |
| Query | O(N) scan | **O(m)** backward search (m = pattern length) |

Cross-crate bridges:
- **Queue → Text** — `TextLogPipeline` batched log compression via `ALICEText`
- **Text → Search** — `CompressedSearchIndex` wrapping FM-Index for compressed text

## Demo: AI-Driven SDF Pipeline (3 Crates)

ALICE-TRT + ALICE-View + ALICE-Voice combine for GPU-accelerated inference with neural upscaling and voice feature extraction.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Ternary    │────▶│ ALICE-TRT   │────▶│ ALICE-View  │────▶│   Display   │
│  Model      │     │ db_bridge   │     │ view_bridge │     │   Neural    │
│  (2-bit GPU)│     │ Log metrics │     │ Upscale 4x  │     │   Upscaled  │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │ ALICE-Voice │
                    │ voice_bridge│
                    │ Mel features│
                    └─────────────┘
```

Cross-crate bridges:
- **TRT → DB** — `TrtDbStore` inference metrics persistence (34-byte `InferenceRecord`)
- **TRT → View** — `NeuralUpscaler` quality tiers (Performance/Balanced/Quality/UltraQuality)
- **TRT → Voice** — `GpuVoiceExtractor` mel-frequency feature extraction

## Demo: Zero-Trust Auth Pipeline (3 Crates)

ALICE-Auth + ALICE-DB + ALICE-Crypto combine for zero-trust authentication with audit logging, rate limiting, and Ed25519 signature verification.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Client    │────▶│ ALICE-Auth  │────▶│  ALICE-DB   │
│  Ed25519    │     │ api_bridge  │     │ db_bridge   │
│  token+sig  │     │ Verify+Rate │     │ Audit log   │
└─────────────┘     └─────────────┘     └─────────────┘
                    │                          │
                    │ verify(&identity,        │ AuthAuditLog
                    │  &token, &signature)     │ (43 bytes)
                    ▼                          ▼
              ┌─────────────┐          ┌─────────────┐
              │ALICE-Crypto │          │  Query by   │
              │ crypto_bridge│         │  identity + │
              │ BLAKE3 + XCC│          │  time range │
              └─────────────┘          └─────────────┘
```

Cross-crate bridges:
- **Auth → API** — `AuthMiddleware` Ed25519 token verification + sliding window rate limiter
- **Auth → DB** — `AuthDbStore` audit log persistence (43-byte `AuthAuditLog`, time-range queries)
- **Auth → Crypto** — Existing `crypto_bridge` for token hashing + session encryption

## Demo: Next-Gen Browser Pipeline (8 Crates)

ALICE-Browser connects to 8 ALICE crates for a fully integrated semantic browser with SDF rendering, CDN routing, voice input, analytics, and compressed text.

```
┌──────────────────────────────────────────────────────────────────────────┐
│                          ALICE-Browser                                    │
│                                                                          │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐       │
│  │ text_bridge│  │cache_bridge│  │cdn_bridge  │  │analytics   │       │
│  │ ALICE-Text │  │ ALICE-Cache│  │ ALICE-CDN  │  │ _bridge    │       │
│  │ Compress   │  │ DOM cache  │  │ Vivaldi rt │  │ DDSketch   │       │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘       │
│                                                                          │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐       │
│  │search_bridge│ │view_bridge │  │ sdf_bridge │  │voice_bridge│       │
│  │ALICE-Search│  │ SDF UI     │  │ Web SDF    │  │ Voice Act. │       │
│  │ In-page    │  │ Rounded    │  │ Scene eval │  │ Detection  │       │
│  │ FM-Index   │  │ Rects      │  │ Sphere tr. │  │ Downsample │       │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘       │
│                                                                          │
│  Core: dom, net, render, engine, simd, branchless, fast_math, mobile    │
└──────────────────────────────────────────────────────────────────────────┘
```

| Feature | Traditional Browser | ALICE-Browser |
|---------|-------------------|---------------|
| UI Rendering | Raster/Vector | **SDF** (infinite resolution) |
| Content Routing | DNS round-robin | **Vivaldi** coordinate nearest-node |
| Search | JavaScript DOM walk | **FM-Index** O(m) backward search |
| Text Compression | gzip (~3x) | **Exception-based** (~10-50x) |
| Analytics | External JS SDK | **Built-in** DDSketch/HLL/CMS |
| Voice | WebRTC | **Parametric** 600x compression |

Cross-crate bridges (8 total):
- **Browser → Text** — `text_bridge` compressed DOM content
- **Browser → Cache** — `cache_bridge` DOM classification caching
- **Browser → Search** — `search_bridge` in-page FM-Index search
- **Browser → Analytics** — `analytics_bridge` page load telemetry (DDSketch, HLL, CMS)
- **Browser → CDN** — `cdn_bridge` Vivaldi coordinate routing
- **Browser → View (SDF UI)** — `view_bridge` resolution-independent SDF rounded rects
- **Browser → SDF** — `sdf_bridge` WebSDF scene evaluation + sphere tracing
- **Browser → Voice** — `voice_bridge` voice activity detection + downsample

## Demo: Anime Production Pipeline (3-5 Crates)

ALICE-Animation + ALICE-SDF combine for anime episode production as compact SDF packages (~20-50 KB per episode), replacing hundreds of megabytes of traditional video.

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐     ┌─────────────┐
│  Storyboard │────▶│   ALICE-     │────▶│  ALICE-SDF   │────▶│ ALICE-View  │
│  Director   │     │  Animation   │     │  ASDF binary │     │  Real-time  │
│  Cuts/Scenes│     │  SceneGraph  │     │  (20-50 KB)  │     │  NPR render │
└─────────────┘     │  Camera/NPR  │     └──────────────┘     └─────────────┘
                    └──────────────┘
                     ↑ (optional)
               ┌─────────────┐
               │ ALICE-Voice │
               │  Lip sync   │
               │  (formants) │
               └─────────────┘
```

| Metric | Traditional Anime | ALICE Anime Pipeline |
|--------|------------------|---------------------|
| Episode file size | 200-500 MB (video) | **20-50 KB** (ASDF) |
| Resolution | Fixed (1080p/4K) | **Infinite** (SDF) |
| Character re-pose | Re-draw/re-render | **Timeline keyframe edit** |
| Localization | Subtitle overlay | **SDF balloon reflow** |

Cross-crate bridges:
- **Animation → SDF** — SceneGraph actors wrap `SdfNode`, `Timeline` keyframes
- **Animation → Voice** — `lip_sync` module: `ParametricParams` formant → phoneme → mouth `Timeline`
- **Animation → View** — CameraState + AnimeShading for NPR rendering
- **Animation → Streaming-Protocol** — Episode → `SdfSceneDescriptor` for streaming delivery

## Demo: SDF Manga Pipeline (2-3 Crates)

ALICE-Manga + ALICE-SDF produce resolution-independent manga pages as SDF trees (~2-10 KB per page), with mathematically moire-free screentones.

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐     ┌─────────────┐
│  Artist     │────▶│  ALICE-Manga │────▶│  ALICE-SDF   │────▶│  Reader     │
│  Strokes,   │     │  Panel/Tone  │     │  ASDF binary │     │  ALICE-View │
│  Balloons   │     │  Balloon     │     │  (2-10 KB)   │     │  or SVG     │
└─────────────┘     └──────────────┘     └──────────────┘     └─────────────┘
                     ↑ (optional)
               ┌─────────────┐
               │ ALICE-Text  │
               │ Localization│
               │ (compress)  │
               └─────────────┘
```

| Metric | Traditional Manga (raster) | ALICE Manga (SDF) |
|--------|---------------------------|-------------------|
| Page size | 2-5 MB (PNG/JPEG) | **2-10 KB** (ASDF) |
| Zoom quality | Pixelated at 200%+ | **Infinite resolution** |
| Screen tone | Moire at non-native DPI | **Mathematically moire-free** |
| Localization | Manual text replacement | **Balloon auto-reflow** |

Cross-crate bridges:
- **Manga → SDF** — Strokes (`Segment2D`/`Bezier`), Panels (`RoundedRect2D`+`Onion`), Tones (`RepeatInfinite`)
- **Manga → Text** — `compress_tuned()` for dialogue compression
- **Manga → Animation** — Optional: animated manga (page transitions, panel effects)

## Use Cases

### IoT / Edge Computing
- Smart sensors (temperature, humidity, pressure)
- Industrial monitoring (vibration, flow rate)
- Agriculture (soil moisture, weather stations)

### 3D Asset Delivery
- Game level streaming (SDF zones, Markov prefetch)
- Procedural content (CSG recipes instead of baked meshes)
- Collaborative 3D editing (SDF diffs at minimal bandwidth)
- IoT/Edge 3D (38 bytes vs 15 KB per object)

### Multiplayer Game Engine
- Deterministic lockstep / rollback netcode (5.6 KB/s for 4 players)
- Physics-accurate rollback with snapshot restore
- Cross-platform bit-exact simulation (128-bit fixed-point)
- SDF asset streaming for game worlds (200-800x vs glTF)

### 3D Printing / Digital Fabrication
- Direct SDF-to-G-code slicing (no mesh intermediary)
- Cloud-to-print via ALICE-Cloud-Gateway (scan → SDF → remote print)
- CSG operations natively supported (no boolean mesh repair)
- Bambu Lab .3mf packaging for one-click print
- LLM-assisted model generation (ALICE-SDF `llm_schema` → ALICE-Print)

### Benefits
- **Bandwidth**: 99% reduction in data transmission
- **Battery**: 90% less power for radio (biggest consumer)
- **Cost**: Fewer LoRaWAN/LTE-M packets = lower bills
- **Latency**: Immediate trend analysis on cloud

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                            ALICE Ecosystem Architecture                          │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ╔═══════════════════════════════════════════════════════════════════════════╗   │
│  ║  LAYER 7: Application                                                    ║   │
│  ║  ┌──────────────────────────────────────────────────────────────────┐     ║   │
│  ║  │ ALICE-Browser    (SDF render, ML filter, smart cache, search)   │     ║   │
│  ║  │ ALICE-Print      (SDF → G-code, SIMD slicer, Bambu .3mf)     │     ║   │
│  ║  │ ALICE-Animation  (Anime SDF direction, NPR, fake perspective) │     ║   │
│  ║  │ ALICE-Manga      (SDF manga, moire-free tone, balloon reflow) │     ║   │
│  ║  └─────────────────────────────┬────────────────────────────────────┘     ║   │
│  ╚════════════════════════════════╪═════════════════════════════════════════╝   │
│                                   │                                              │
│  ╔════════════════════════════════╪═════════════════════════════════════════╗   │
│  ║  LAYER 6: Visualization & Analytics                                      ║   │
│  ║  ┌────────────┐  ┌────────────────┐                                      ║   │
│  ║  │ ALICE-View │  │ ALICE-Analytics│                                      ║   │
│  ║  │ wgpu/egui  │  │ HLL, DDSketch  │                                      ║   │
│  ║  │ GPU render │  │ CMS, LDP       │                                      ║   │
│  ║  └─────┬──────┘  └───────┬────────┘                                      ║   │
│  ╚════════╪═════════════════╪═══════════════════════════════════════════════╝   │
│           │                 │                                                    │
│  ╔════════╪═════════════════╪═══════════════════════════════════════════════╗   │
│  ║  LAYER 5: Data & Storage ▼                                               ║   │
│  ║  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐             ║   │
│  ║  │ ALICE-DB │  │   Cache  │  │  Queue   │  │    Search    │             ║   │
│  ║  │ LSM-Tree │  │ TinyLFU  │  │ SPSC WAL │  │  FM-Index    │             ║   │
│  ║  └─────┬────┘  └────┬─────┘  └────┬─────┘  └──────┬───────┘             ║   │
│  ╚════════╪═════════════╪════════════╪═══════════════╪══════════════════════╝   │
│           │             │            │               │                            │
│  ╔════════╪═════════════╪════════════╪═══════════════╪══════════════════════╗   │
│  ║  LAYER 4: Networking & Streaming   ▼               ▼                      ║   │
│  ║  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌───────────────┐            ║   │
│  ║  │ ALICE-API│  │   CDN    │  │   Sync   │  │  Streaming    │            ║   │
│  ║  │ GCRA/SFQ │  │ Vivaldi  │  │ P2P Diff │  │  Protocol     │            ║   │
│  ║  └─────┬────┘  └────┬─────┘  └────┬─────┘  └──────┬────────┘            ║   │
│  ║  ┌──────────┐  ┌─────────────────────────────────────────────┐          ║   │
│  ║  │ALICE-DNS │  │ ALICE-Cloud-Gateway (ASP ingest, BLAKE3 KDF)│          ║   │
│  ║  │Bloom O(1)│  └─────────────────────────┬───────────────────┘          ║   │
│  ║  └─────┬────┘                            │                              ║   │
│  ╚════════╪═════════════╪════════════╪═══════════════╪═════════════════════╝   │
│           │             │            │                                            │
│  ╔════════╪═════════════╪════════════╪══════════════════════════════════════╗   │
│  ║  LAYER 3: Security    ▼            ▼                                     ║   │
│  ║  ┌──────────────┐  ┌──────────────────┐                                  ║   │
│  ║  │  ALICE-Auth  │  │  ALICE-Crypto    │                                  ║   │
│  ║  │ Ed25519, ZKP │  │ SSS, BLAKE3      │                                  ║   │
│  ║  │              │  │ XChaCha20-Poly   │                                  ║   │
│  ║  └──────┬───────┘  └────────┬─────────┘                                  ║   │
│  ╚═════════╪═══════════════════╪════════════════════════════════════════════╝   │
│            │                   │                                                  │
│  ╔═════════╪═══════════════════╪════════════════════════════════════════════╗   │
│  ║  LAYER 2: Compression & Encoding                                         ║   │
│  ║  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐     ║   │
│  ║  │  Edge  │ │  Zip   │ │ Codec  │ │ Voice  │ │  Text  │ │  SDF   │     ║   │
│  ║  │ 500x   │ │10-1000x│ │Wavelet │ │LPC 600x│ │Pattern │ │  CSG   │     ║   │
│  ║  │ no_std │ │  LZMA  │ │  rANS  │ │Privacy │ │Columnar│ │Infinite│     ║   │
│  ║  └────┬───┘ └────┬───┘ └────┬───┘ └────┬───┘ └────┬───┘ └────┬───┘     ║   │
│  ║  ┌────────┐ ┌────────┐                                                   ║   │
│  ║  │ Synth  │ │  Font  │  Procedural audio + parametric metafont          ║   │
│  ║  └────┬───┘ └────┬───┘                                                   ║   │
│  ╚═══════╪══════════╪══════════╪══════════╪══════════╪══════════╪══════════╝   │
│          │          │          │          │          │          │                  │
│  ╔═══════╪══════════╪══════════╪══════════╪══════════╪══════════╪══════════╗   │
│  ║  LAYER 1: Compute & Runtime                                              ║   │
│  ║  ┌───────────────┐ ┌───────────────┐ ┌───────────────┐ ┌─────────────┐  ║   │
│  ║  │ALICE-Container│ │   ALICE-ML    │ │  ALICE-TRT    │ │ALICE-Physics│  ║   │
│  ║  │ cgroup v2     │ │1.58-bit ternry│ │ GPU ternary   │ │128-bit Fixed│  ║   │
│  ║  │ io_uring      │ │ no multiply   │ │ wgpu/CUDA     │ │ XPBD,GJK   │  ║   │
│  ║  │ clone3, PSI   │ │ SIMD-ready    │ │ BitNet matmul │ │ CORDIC,BVH  │  ║   │
│  ║  └───────────────┘ └───────────────┘ └───────────────┘ └─────────────┘  ║   │
│  ║  ┌───────────────┐ ┌───────────────┐ ┌───────────────┐                  ║   │
│  ║  │  ALICE-RTOS   │ │ ALICE-Motion  │ │  ALICE-VCS    │                  ║   │
│  ║  │ RMS scheduler │ │ NURBS/Bezier  │ │ AST diff/merge│                  ║   │
│  ║  │ Liu-Layland   │ │ Trapezoidal   │ │ Merkle hash   │                  ║   │
│  ║  │ SPSC ring,<2KB│ │ S-curve prof. │ │ Content-addr  │                  ║   │
│  ║  └───────────────┘ └───────────────┘ └───────────────┘                  ║   │
│  ║  ┌───────────────────┐                                                  ║   │
│  ║  │ALICE-Kinematics   │  7-DoF arm, jerk min., 8-byte intent packets    ║   │
│  ║  │ MIT decoder       │  Open Core (encoder = AGPL-3.0)                  ║   │
│  ║  └───────────────────┘                                                  ║   │
│  ╚══════════════════════════════════════════════════════════════════════════╝   │
│                                                                                  │
│  All components: Rust | no_std compatible | Zero allocation | Deterministic      │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Mathematical Foundation

ALICE is based on **Kolmogorov Complexity**: the shortest program that produces the data is the optimal compression.

```
Traditional: Store [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]  → 40 bytes
ALICE:       Store "f(x) = x" for x in [1,10]       → 8 bytes
```

For sensor data that follows physical laws (temperature gradients, pressure decay, etc.), the mathematical model is often trivially small compared to the raw data.

## License Strategy — 3-Layer Monetization Architecture

The ALICE ecosystem employs a **3-layer license strategy** designed to maximize adoption while protecting high-value authoring tools.

```
┌─────────────────────────────────────────────────────────────────┐
│                    ALICE License Architecture                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Layer 3: Proprietary/BSL                ← Revenue generator     │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │ ALICE-Animation  ALICE-Manga  ALICE-Print              │     │
│  │ Pro authoring tools / Encoders / Production pipelines   │     │
│  │ License: Commercial required for production use         │     │
│  └─────────────────────────────────────────────────────────┘     │
│                                                                   │
│  Layer 2: AGPL-3.0                       ← SaaS protection       │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │ ALICE-Cache  ALICE-Queue  ALICE-DB  ALICE-CDN           │     │
│  │ ALICE-API  ALICE-Search  ALICE-Auth  ALICE-Crypto       │     │
│  │ ALICE-Container  ALICE-ML  ALICE-TRT  ALICE-Physics     │     │
│  │ ALICE-Sync  ALICE-Cloud-Gateway  ALICE-Analytics        │     │
│  │ ALICE-DNS  ALICE-Codec  ALICE-RTOS  ALICE-VCS           │     │
│  │ Distribution servers / Infrastructure / Backend         │     │
│  │ AGPL requires source disclosure if used in SaaS         │     │
│  └─────────────────────────────────────────────────────────┘     │
│                                                                   │
│  Layer 1: MIT                            ← Adoption driver       │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │ ALICE-SDF  ALICE-Edge  ALICE-Voice  ALICE-View          │     │
│  │ ALICE-Streaming-Protocol  ALICE-Eco-System              │     │
│  │ ALICE-Synth  ALICE-Motion  ALICE-Font                    │     │
│  │ Format definitions / Viewers / Renderers / Decoders     │     │
│  │ MIT = maximum adoption, anyone can build readers        │     │
│  └─────────────────────────────────────────────────────────┘     │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

### Strategy Rationale

| Layer | License | Purpose | Target |
|-------|---------|---------|--------|
| **Layer 1** | MIT | Maximize format adoption — free readers, viewers, and decoders ensure the ALICE format becomes ubiquitous | Developers, hobbyists, OSS projects |
| **Layer 2** | AGPL-3.0 | Prevent SaaS free-riding — any company deploying ALICE infrastructure as a service must open-source modifications or purchase commercial license | Netflix, Amazon, cloud providers |
| **Layer 3** | Proprietary | Protect revenue — authoring tools (Animation, Manga, Print) that create ALICE content require commercial licensing | Production studios, publishers |

### Target Markets

- **Anime**: Netflix, Amazon Prime Video, Crunchyroll — ALICE-Animation replaces 200-500 MB episodes with 20-50 KB ASDF packages
- **Manga**: ピッコマ, LINE Manga, Kindle — ALICE-Manga replaces 2-5 MB raster pages with 2-10 KB resolution-independent SDF pages
- **3D Printing**: Bambu Lab, Prusa — ALICE-Print skips mesh intermediary entirely

### Revenue Model

```
Reading (MIT) ──── FREE ───────────── Everyone can read ALICE content
Distributing (AGPL) ── OPEN ────────── SaaS providers must open-source or pay
Creating (Proprietary) ── PAID ─────── Studios/publishers pay for authoring tools
```

The free reader tier ensures content reaches maximum audience. The AGPL layer ensures infrastructure providers contribute back. The proprietary layer captures value from professional content creators.

## License

MIT License (this integration demo)

See individual component READMEs for per-crate licenses.

## Author

Moroya Sakamoto

---

*"The best data is the data you never had to send."*
