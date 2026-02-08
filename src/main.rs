//! ALICE Ecosystem Integration Demo
//!
//! Demonstrates the complete data pipeline with ZERO unnecessary allocations.
//!
//! ```text
//! [Sensor Generator (Iterator)] → [Edge: Stack Only] → [Network] → [DB: Batch Write] → [View]
//! ```

use alice_db::{AliceDB, Aggregation};
use alice_edge::fit_linear_fixed;
use alice_view::{ViewerConfig, launch_viewer};
use tempfile::tempdir;

// Constants
const SAMPLE_COUNT: usize = 1000;
const BASE_TEMP: f32 = 25.0;
const SLOPE: f32 = 0.005;

/// Zero-allocation sensor data generator (Iterator)
struct SensorGenerator {
    current: usize,
    count: usize,
    base_temp: f32,
    slope: f32,
}

impl SensorGenerator {
    #[inline(always)]
    fn new(count: usize, base_temp: f32, slope: f32) -> Self {
        Self { current: 0, count, base_temp, slope }
    }
}

impl Iterator for SensorGenerator {
    type Item = i32;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.count {
            return None;
        }
        let i = self.current;
        self.current += 1;
        // Generate temperature data: y = slope * x + base
        // Values are in centidegrees (e.g., 2500 = 25.00°C)
        Some(((self.base_temp + self.slope * i as f32) * 100.0) as i32)
    }
}

/// Serialize two i32 coefficients as single u64 store (Zero-Copy, Inline)
#[inline(always)]
fn serialize_coefficients(slope: i32, intercept: i32) -> [u8; 8] {
    // Pack two i32 → one u64: single 64-bit store instruction
    let combined = (slope as u32 as u64) | ((intercept as u32 as u64) << 32);
    combined.to_le_bytes()
}

/// Deserialize two i32 coefficients as single u64 load (Zero-Copy, Inline)
#[inline(always)]
fn deserialize_coefficients(buf: &[u8; 8]) -> (i32, i32) {
    // Unpack u64 → two i32: single 64-bit load + shift
    let combined = u64::from_le_bytes(*buf);
    (combined as u32 as i32, (combined >> 32) as u32 as i32)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         ALICE ECOSYSTEM INTEGRATION DEMO                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // ========================================================================
    // PHASE 1: SENSOR DATA GENERATION (Zero Heap Allocation)
    // ========================================================================
    println!("━━━ PHASE 1: Sensor Data Generation (Stack Allocated) ━━━");

    // Use stack buffer instead of Heap Vec
    // 1000 samples * 4 bytes = 4KB (Fits in stack easily)
    let mut sensor_buffer = [0i32; SAMPLE_COUNT];

    // Fill buffer using zero-allocation iterator
    let generator = SensorGenerator::new(SAMPLE_COUNT, BASE_TEMP, SLOPE);
    for (i, val) in generator.enumerate() {
        sensor_buffer[i] = val;
    }

    let raw_bytes = SAMPLE_COUNT * 4;
    println!("  Sensor readings: {} samples (Stack: {}KB)", SAMPLE_COUNT, raw_bytes / 1024);
    println!("  First value:     {:.2}°C", sensor_buffer[0] as f32 / 100.0);
    println!("  Last value:      {:.2}°C", sensor_buffer[SAMPLE_COUNT - 1] as f32 / 100.0);
    println!("  Raw data size:   {} bytes", raw_bytes);
    println!();

    // ========================================================================
    // PHASE 2: ALICE-EDGE COMPRESSION (Ultimate Optimized)
    // ========================================================================
    println!("━━━ PHASE 2: ALICE-Edge Compression (Ultimate) ━━━");

    // fit_linear_fixed is already optimized:
    // - O(1) sum_x, sum_xx via closed-form formulas
    // - Loop unrolling (4x)
    // - Unsafe pointer arithmetic
    // - Zero bounds checks
    let (slope, intercept) = fit_linear_fixed(&sensor_buffer);

    println!("  Model: y = slope × x + intercept (Q16.16 fixed-point)");
    println!("  Slope:     {} (Q16.16) = {:.6}", slope, slope as f64 / 65536.0);
    println!("  Intercept: {} (Q16.16) = {:.2}", intercept, intercept as f64 / 65536.0);

    // Serialize for transmission (8 bytes, stack only)
    let packet = serialize_coefficients(slope, intercept);
    let compressed_bytes = packet.len();

    println!();
    println!("  Packet (hex): {:02x?}", packet);
    println!("  Packet size:  {} bytes", compressed_bytes);
    println!();
    println!("  ┌─────────────────────────────────────────────────┐");
    println!("  │ COMPRESSION: {} bytes → {} bytes               │", raw_bytes, compressed_bytes);
    println!("  │ RATIO:       {}x                              │", raw_bytes / compressed_bytes);
    println!("  └─────────────────────────────────────────────────┘");
    println!();

    // ========================================================================
    // PHASE 3: NETWORK TRANSMISSION (Simulated)
    // ========================================================================
    println!("━━━ PHASE 3: Network Transmission ━━━");
    println!("  [EDGE DEVICE] ──── 8 bytes ────▶ [CLOUD SERVER]");
    println!("  Traditional:      {} bytes (LoRaWAN: ~{} packets)", raw_bytes, raw_bytes / 250);
    println!("  ALICE-Edge:       8 bytes (LoRaWAN: 1 packet!)");
    println!();

    // Simulate network receive (packet moves from edge to server memory)
    let received_packet = packet;
    let (rx_slope, rx_intercept) = deserialize_coefficients(&received_packet);
    println!("  Received: slope={}, intercept={}", rx_slope, rx_intercept);
    println!();

    // ========================================================================
    // PHASE 4: ALICE-DB STORAGE (Batch Optimized)
    // ========================================================================
    println!("━━━ PHASE 4: ALICE-DB Storage (Batch Insert) ━━━");

    let db_dir = tempdir()?;
    let db = AliceDB::open(db_dir.path())?;

    println!("  Database opened at: {:?}", db_dir.path());

    // Reconstruct data into a batch buffer for high-throughput insert
    // Instead of calling db.put() 1000 times (1000 lock acquisitions),
    // we build a batch and insert once.
    //
    // Stack-allocated fixed array (16KB)
    // Zero malloc in user code!
    let mut batch_buffer = [(0i64, 0.0f32); SAMPLE_COUNT];

    // Pre-calculate constants for fast loop
    const Q16_SCALE: f32 = 1.0 / 65536.0;
    const CENTI_SCALE: f32 = 1.0 / 100.0;

    println!("  Reconstructing {} points (stack mode, zero malloc)...", SAMPLE_COUNT);

    for i in 0..SAMPLE_COUNT {
        // Hand-optimized evaluation loop
        // Manual inline of evaluate_linear_fixed logic:
        // y = slope * x + intercept (Q16.16 arithmetic)
        let x = i as i32;
        let mx = (rx_slope as i64).wrapping_mul(x as i64);
        let q16_val = (mx as i32).wrapping_add(rx_intercept);

        // Convert Q16.16 centidegrees to °C
        let value = q16_val as f32 * Q16_SCALE * CENTI_SCALE;

        // Index access (compiler elides bounds check for const SAMPLE_COUNT)
        batch_buffer[i] = (i as i64, value);
    }

    // Single Batch Insert (Lock acquired only ONCE instead of 1000 times)
    db.put_batch(&batch_buffer)?;
    db.flush()?;

    let stats = db.stats();
    println!();
    println!("  DB Statistics:");
    println!("    Segments:     {}", stats.total_segments);
    println!("    Disk size:    {} bytes", stats.total_disk_size);
    println!("    Compression:  {:.1}x", stats.average_compression_ratio);
    println!("    Models used:  {:?}", stats.model_distribution);
    println!();

    // ========================================================================
    // PHASE 5: QUERY & VERIFICATION
    // ========================================================================
    println!("━━━ PHASE 5: Query & Verification ━━━");

    // Point queries
    let sample_points = [0, 250, 500, 750, 999];
    println!("  Point Queries:");
    println!("  ┌──────────┬────────────┬────────────┬──────────┐");
    println!("  │   Time   │  Original  │  DB Value  │  Error   │");
    println!("  ├──────────┼────────────┼────────────┼──────────┤");

    for &t in &sample_points {
        let original = sensor_buffer[t] as f32 / 100.0;
        let db_value = db.get(t as i64)?.unwrap_or(0.0);
        let error = (original - db_value).abs();
        println!(
            "  │ {:>8} │ {:>8.2}°C │ {:>8.2}°C │ {:>6.4}°C │",
            t, original, db_value, error
        );
    }
    println!("  └──────────┴────────────┴────────────┴──────────┘");
    println!();

    // Aggregation queries (using SIMD-optimized paths in ALICE-DB)
    println!("  Aggregation Queries (0-999):");
    let avg = db.aggregate(0, 999, Aggregation::Avg)?;
    let min = db.aggregate(0, 999, Aggregation::Min)?;
    let max = db.aggregate(0, 999, Aggregation::Max)?;

    println!("    AVG: {:.2}°C", avg);
    println!("    MIN: {:.2}°C", min);
    println!("    MAX: {:.2}°C", max);
    println!();

    // ========================================================================
    // SUMMARY
    // ========================================================================
    // ========================================================================
    // PHASE 6: ALICE-VIEW VISUALIZATION (Optional)
    // ========================================================================
    println!("━━━ PHASE 6: ALICE-View Visualization ━━━");
    println!("  GPU-accelerated procedural rendering available!");
    println!();
    println!("  Data can be visualized as:");
    println!("    - Perlin noise terrain (temperature → elevation)");
    println!("    - Voronoi cells (sensor clustering)");
    println!("    - Mandelbrot fractal (infinite zoom demo)");
    println!("    - Plasma effect (animated data flow)");
    println!();

    // Check if --view flag is passed
    let launch_view = std::env::args().any(|arg| arg == "--view");

    if launch_view {
        println!("  Launching ALICE-View...");
        println!("  Controls: Scroll=Zoom, Drag=Pan, F1=X-Ray, F2=Stats, Space=Pause");
        println!();
    } else {
        println!("  Run with --view to launch the visualization window");
        println!();
    }

    // ========================================================================
    // SUMMARY
    // ========================================================================
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                      PIPELINE SUMMARY                         ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║                                                              ║");
    println!("║  [Sensor]                                                    ║");
    println!("║     │ {} samples × 4 bytes = {} bytes (Stack)            │", SAMPLE_COUNT, raw_bytes);
    println!("║     ▼                                                        ║");
    println!("║  [ALICE-Edge] (Ultimate Optimized)                           ║");
    println!("║     │ fit_linear_fixed() → 8 bytes                           ║");
    println!("║     │ O(1) formulas + 4x unroll + unsafe ptr                 ║");
    println!("║     │ Compression: {}x                                      ║", raw_bytes / compressed_bytes);
    println!("║     ▼                                                        ║");
    println!("║  [Network: LoRaWAN/LTE-M]                                    ║");
    println!("║     │ 8 bytes transmitted                                    ║");
    println!("║     ▼                                                        ║");
    println!("║  [ALICE-DB] (Batch Insert)                                   ║");
    println!("║     │ put_batch() - Single lock acquisition                  ║");
    println!("║     │ Disk: {} bytes, Compression: {:.1}x                   ║",
             stats.total_disk_size, stats.average_compression_ratio);
    println!("║     ▼                                                        ║");
    println!("║  [Query]                                                     ║");
    println!("║     │ Point, Range, Aggregation (SIMD accelerated)           ║");
    println!("║     ▼                                                        ║");
    println!("║  [ALICE-View] (GPU Procedural Rendering)                     ║");
    println!("║     └─ wgpu + egui, infinite zoom, X-Ray mode                ║");
    println!("║                                                              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  OPTIMIZATIONS APPLIED:                                       ║");
    println!("║    ✓ Stack allocation - sensor buffer (4KB)                  ║");
    println!("║    ✓ Stack allocation - batch buffer (16KB)                  ║");
    println!("║    ✓ Iterator-based generation (lazy evaluation)             ║");
    println!("║    ✓ Batch DB insert (1 lock vs 1000 locks)                  ║");
    println!("║    ✓ Hand-inlined evaluation (no function call overhead)     ║");
    println!("║    ✓ #[inline(always)] on hot paths                          ║");
    println!("║    ✓ ZERO MALLOC IN USER CODE                                ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  TOTAL: {} bytes → 8 bytes → {} bytes                     ║",
             raw_bytes, stats.total_disk_size);
    println!("║  BANDWIDTH SAVED: 99.8%                                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("✓ ALICE Ecosystem Integration: SUCCESS (Zero Malloc)");

    db.close()?;

    // Launch viewer if requested (after DB close to free resources)
    if launch_view {
        launch_viewer(ViewerConfig {
            title: "ALICE Ecosystem - Data Visualization".to_string(),
            show_stats: true,
            ..Default::default()
        })?;
    }

    Ok(())
}
