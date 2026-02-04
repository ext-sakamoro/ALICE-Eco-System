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
│  │ Physics    TRT    │  │ View           │  │ (Edge → DB → View)     │  │
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
| [ALICE-Sync](https://github.com/ext-sakamoro/ALICE-Sync) | v0.6.0 | P2P Synchronization via Event Diffing | 18-byte events, bit-exact determinism | AGPL-3.0 |

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
| [ALICE-Physics](https://github.com/ext-sakamoro/ALICE-Physics) | v0.2.0 | Deterministic 128-bit Physics Engine | I64F64, CORDIC, XPBD, GJK/EPA, BVH | AGPL-3.0 |

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

## Use Cases

### IoT / Edge Computing
- Smart sensors (temperature, humidity, pressure)
- Industrial monitoring (vibration, flow rate)
- Agriculture (soil moisture, weather stations)

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
