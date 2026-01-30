# ALICE Ecosystem

**The Complete Edge-to-Cloud Data Pipeline with GPU Visualization**

> "Don't send data. Send the law."

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Sensor    │────▶│ ALICE-Edge  │────▶│  Network    │────▶│  ALICE-DB   │────▶│ ALICE-View  │
│  1000 pts   │     │  8 bytes    │     │  8 bytes    │     │   Query     │     │    GPU      │
│  (4000 B)   │     │  (500x)     │     │  (LoRaWAN)  │     │  Reconstruct│     │  Rendering  │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
```

## What is ALICE?

ALICE (**A**daptive **L**ightweight **I**ntelligent **C**ompression **E**ngine) is an ecosystem of libraries that work together to achieve extreme data compression by storing mathematical models instead of raw data.

| Component | Role | Feature |
|-----------|------|---------|
| [ALICE-Edge](https://github.com/ext-sakamoro/ALICE-Edge) | On-device model fitting | 500x compression |
| [ALICE-Streaming](https://github.com/ext-sakamoro/ALICE-Streaming-Protocol) | Network protocol | 100-1000x compression |
| [ALICE-DB](https://github.com/ext-sakamoro/ALICE-DB) | Model-based storage | 50-100x compression |
| [ALICE-Zip](https://github.com/ext-sakamoro/ALICE-Zip) | General compression | Variable ratio |
| [ALICE-View](https://github.com/ext-sakamoro/ALICE-View) | GPU visualization | Infinite zoom, X-Ray |

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

## Demo Output

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
                    ┌────────────────────────────────────────────┐
                    │              EDGE DEVICE                    │
                    │  ┌────────────┐    ┌────────────────────┐  │
    Raw Data ──────▶│  │   Sensor   │───▶│    ALICE-Edge      │──┼──▶ 8 bytes
    (discarded)     │  │   Buffer   │    │  fit_linear_fixed  │  │
                    │  └────────────┘    └────────────────────┘  │
                    └────────────────────────────────────────────┘
                                             │
                                             │ LoRaWAN / LTE-M / WiFi
                                             │ (8 bytes per transmission)
                                             ▼
                    ┌────────────────────────────────────────────┐
                    │              CLOUD / SERVER                 │
                    │  ┌────────────────────┐  ┌──────────────┐  │
    8 bytes ───────▶│  │   Deserialize      │─▶│   ALICE-DB   │  │
                    │  │   (slope,intercept)│  │   Storage    │  │
                    │  └────────────────────┘  └──────────────┘  │
                    │                                │            │
                    │                                ▼            │
                    │                         ┌──────────────┐   │
                    │                         │    Query     │   │
                    │                         │  Reconstruct │   │
                    │                         └──────────────┘   │
                    │                                │            │
                    │                                ▼            │
                    │                         ┌──────────────┐   │
                    │                         │ ALICE-View   │   │
                    │                         │   (wgpu)     │   │
                    │                         │  ∞ Zoom GPU  │   │
                    │                         └──────────────┘   │
                    └────────────────────────────────────────────┘
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
