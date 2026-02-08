# ALICE Ecosystem

**The Complete Edge-to-Cloud Data Pipeline with GPU Visualization**

> "Don't send data. Send the law."

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          ALICE Ecosystem (24 Components)                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─── Compression ───┐  ┌─── Data ────┐  ┌─── Network ───┐  ┌ Security ─┐ │
│  │ Edge   Zip  Codec │  │ DB    Cache │  │ API    CDN    │  │ Auth      │ │
│  │ Voice  Text  SDF  │  │ Queue Search│  │ Sync Streaming│  │ Crypto    │ │
│  └───────────────────┘  └────────────┘  └───────────────┘  └───────────┘ │
│                                                                             │
│  ┌──── Compute ──────┐  ┌─── Analytics ──┐  ┌─── Application ────────┐  │
│  │ Container  ML     │  │ Analytics      │  │ Browser  Eco-System    │  │
│  │ Physics+NC TRT    │  │ View           │  │ (Edge → DB → View)     │  │
│  └───────────────────┘  └────────────────┘  └────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## What is ALICE?

ALICE (**A**daptive **L**ightweight **I**ntelligent **C**ompression **E**ngine) is an ecosystem of libraries that work together to achieve extreme data compression by storing mathematical models instead of raw data.

### Compression & Encoding

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Edge](https://github.com/ext-sakamoro/ALICE-Edge) | v0.1.0 | Embedded Model Generator | 500x compression, no_std, 1KB footprint | MIT |
| [ALICE-Zip](https://github.com/ext-sakamoro/ALICE-Zip) | v1.0.0 | Procedural Generation Compression | 10-1000x for patterns, LZMA fallback | Open Core (MIT core) |
| [ALICE-Codec](https://github.com/ext-sakamoro/ALICE-Codec) | v0.1.0 | 3D Wavelet Video/Audio Codec | CDF 9/7 Wavelet, rANS entropy coding | AGPL-3.0 |
| [ALICE-Voice](https://github.com/ext-sakamoro/ALICE-Voice) | v0.1.0 | Voice Procedural Codec | LPC parametric 100-600x, privacy-preserving | MIT |
| [ALICE-Text](https://github.com/ext-sakamoro/ALICE-Text) | v1.0.0 | Exception-Based Text Compression | Pattern recognition, columnar encoding | BSL 1.1 (→MIT 2028) |
| [ALICE-SDF](https://github.com/ext-sakamoro/ALICE-SDF) | v0.1.0 | 3D Signed Distance Functions | 10-1000x, infinite resolution, CSG ops | MIT |

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
| [ALICE-API](https://github.com/ext-sakamoro/ALICE-API) | v0.2.0 | API Gateway with Distributed Rate Limiting | GCRA lock-free, SFQ, zero-copy splice | AGPL-3.0 |
| [ALICE-CDN](https://github.com/ext-sakamoro/ALICE-CDN) | v0.2.0 | Decentralized Content Delivery | Vivaldi coordinates, SIMD, Maglev hashing | AGPL-3.0 |
| [ALICE-Streaming-Protocol](https://github.com/ext-sakamoro/ALICE-Streaming-Protocol) | v1.0.0 | High-Performance Video Streaming Codec | FlatBuffers, motion estimation, SIMD | MIT |
| [ALICE-Sync](https://github.com/ext-sakamoro/ALICE-Sync) | v0.6.0 | P2P Synchronization via Event Diffing | 18-byte events, bit-exact determinism, Lockstep/Rollback, PyO3 | AGPL-3.0 |

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

### Analytics & Visualization

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Analytics](https://github.com/ext-sakamoro/ALICE-Analytics) | v0.1.0 | Streaming Telemetry & Statistics | HyperLogLog++, DDSketch, CMS, LDP | AGPL-3.0 |
| [ALICE-View](https://github.com/ext-sakamoro/ALICE-View) | v0.1.0 | Infinite Canvas GPU Renderer | wgpu procedural rendering, 60 FPS | MIT |

### Application

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Browser](https://github.com/ext-sakamoro/ALICE-Browser) | v1.0.0 | Semantic Browser | SDF rendering, ML filtering, predictive cache | MIT OR Apache-2.0 |

### Integration

| Component | Version | Description | Feature | License |
|-----------|---------|-------------|---------|---------|
| [ALICE-Eco-System](https://github.com/ext-sakamoro/ALICE-Eco-System) | v0.1.0 | Ecosystem Integration Demo | Edge → Streaming → DB → View pipeline | MIT |

**Total: 24 components** | AGPL-3.0: 14 | MIT: 6 | MIT/Apache-2.0: 1 | BSL 1.1: 1 | Open Core: 2

## Quick Start

```bash
# Clone the ecosystem demo
git clone https://github.com/ext-sakamoro/ALICE-Eco-System.git
cd ALICE-Eco-System

# Run the integration demo
cargo run

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
║   ALICE ECOSYSTEM INTEGRATION DEMO (TRUE KARIKARI EDITION)   ║
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
│  (asset_id) │     │  Routing    │     │  Prefetch   │     │  ~80 bytes  │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                    O(log n + k)         lock-free            vs glTF 20 KB
                    nearest node         prediction           = 200-300x
```

| Asset Type | glTF Size | SDF Size | Ratio |
|------------|-----------|----------|-------|
| Sphere | 15-25 KB | ~80 bytes | **200-300x** |
| CSG (10 ops) | 200-500 KB | ~500 bytes | **400-1000x** |
| Complex scene (100 nodes) | 2-4 MB | 2-4 KB | **500-1000x** |

## Demo: Game Engine Pipeline

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

## Use Cases

### IoT / Edge Computing
- Smart sensors (temperature, humidity, pressure)
- Industrial monitoring (vibration, flow rate)
- Agriculture (soil moisture, weather stations)

### 3D Asset Delivery
- Game level streaming (SDF zones, Markov prefetch)
- Procedural content (CSG recipes instead of baked meshes)
- Collaborative 3D editing (SDF diffs at minimal bandwidth)
- IoT/Edge 3D (80 bytes vs 20 KB per object)

### Multiplayer Game Engine
- Deterministic lockstep / rollback netcode (5.6 KB/s for 4 players)
- Physics-accurate rollback with snapshot restore
- Cross-platform bit-exact simulation (128-bit fixed-point)
- SDF asset streaming for game worlds (200-800x vs glTF)

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
│  ║  │ ALICE-Browser  (SDF render, ML filter, smart cache, search)     │     ║   │
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

## License

MIT License

## Author

Moroya Sakamoto

---

*"The best data is the data you never had to send."*
