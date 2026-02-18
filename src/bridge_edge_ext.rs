//! Edge extended bridges — ALICE-Edge ↔ DB, View, ASP, Analytics
//!
//! 4 bridges connecting edge/IoT sensor processing to the ALICE ecosystem.

use alice_edge::{
    fit_linear_fixed, compute_residual_error, should_use_linear,
    Q16_SHIFT,
};

/// Reciprocal of Q16.16 scale factor (1 / 65536.0) — replaces division in Q16 → f32.
const RCP_Q16: f32 = 1.0 / 65536.0;

/// Reciprocal of 100 — replaces division when computing percentages.
const RCP_100: f32 = 1.0 / 100.0;

/// Reciprocal of i64::MAX approximation used for error normalisation.
/// We cap error display at 2^40 raw units; reciprocal avoids runtime division.
const RCP_ERR_SCALE: f64 = 1.0 / (1u64 << 40) as f64;

// ── Bridge 1: Edge → DB (sensor model persistence) ───────────────────────

/// Persistent sensor model record for ALICE-DB.
///
/// Stores the Q16.16 linear coefficients produced by `fit_linear_fixed`,
/// together with provenance metadata (sample count, fit error, content hash).
/// All fields are plain integers/floats — no heap allocation.
pub struct EdgeDbSensorModel {
    /// Slope coefficient in Q16.16 fixed-point.
    pub slope_q16: i32,
    /// Intercept coefficient in Q16.16 fixed-point.
    pub intercept_q16: i32,
    /// Slope as f32 (pre-scaled, avoids repeated Q16 conversion).
    pub slope_f32: f32,
    /// Intercept as f32 (pre-scaled).
    pub intercept_f32: f32,
    /// Number of samples the model was fitted from.
    pub sample_count: u32,
    /// Residual sum-of-squares error (Q32.32 internal units, clamped to i64).
    pub fit_error_raw: i64,
    /// Normalised fit quality in [0.0, 1.0] (1.0 = perfect fit).
    pub fit_quality: f32,
    /// FNV-1a content hash of the 8-byte coefficient pair (for dedup).
    pub content_hash: u64,
    /// True when the linear model was chosen over a constant model.
    pub model_is_linear: bool,
}

/// Fit a linear model to sensor data and produce an ALICE-DB persistence record.
///
/// # カリカリ notes
/// - Q16 → f32 uses reciprocal multiply (`* RCP_Q16`), not division.
/// - Fit quality normalisation uses `RCP_ERR_SCALE`, no runtime `/`.
/// - `should_use_linear` result stored as plain bool (branchless downstream).
#[inline]
pub fn edge_to_db_sensor_model(data: &[i32]) -> EdgeDbSensorModel {
    let (slope, intercept) = fit_linear_fixed(data);
    let error = compute_residual_error(data, slope, intercept);
    let model_is_linear = should_use_linear(data);
    let sample_count = data.len() as u32;

    // Q16 → f32: reciprocal multiply, zero division.
    let slope_f32 = slope as f32 * RCP_Q16;
    let intercept_f32 = intercept as f32 * RCP_Q16;

    // Normalised quality: clamp error to [0, 2^40], invert.
    // branchless clamp via f64 min/max, no if/else.
    let err_norm = (error.unsigned_abs() as f64 * RCP_ERR_SCALE).min(1.0);
    let fit_quality = (1.0 - err_norm) as f32;

    // Content hash: FNV-1a over the 8-byte slope+intercept pair.
    let mut key_bytes = [0u8; 8];
    key_bytes[..4].copy_from_slice(&slope.to_le_bytes());
    key_bytes[4..].copy_from_slice(&intercept.to_le_bytes());
    let content_hash = crate::hash::fnv1a(&key_bytes);

    EdgeDbSensorModel {
        slope_q16: slope,
        intercept_q16: intercept,
        slope_f32,
        intercept_f32,
        sample_count,
        fit_error_raw: error,
        fit_quality,
        content_hash,
        model_is_linear,
    }
}

// ── Bridge 2: Edge → View (sensor visualisation config) ──────────────────

/// Chart type hint for ALICE-View rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SensorChartType {
    /// Scatter plot with regression line overlay.
    Scatter = 0,
    /// Simple line chart (for slowly-changing sensors).
    Line = 1,
    /// Bar chart (for discrete / enumerated readings).
    Bar = 2,
}

/// Visualisation configuration for an ALICE-View sensor panel.
pub struct EdgeViewSensorConfig {
    /// Recommended chart type derived from model characteristics.
    pub chart_type: SensorChartType,
    /// Y-axis minimum (in sensor integer units, not Q16).
    pub y_min: i32,
    /// Y-axis maximum.
    pub y_max: i32,
    /// Data range span (y_max - y_min), pre-computed.
    pub y_range: u32,
    /// Recommended panel update rate (Hz), clamped to [1, 60].
    pub update_rate_hz: u8,
    /// Content hash for View cache keying.
    pub content_hash: u64,
}

/// Derive a View visualisation config from a fitted edge sensor model.
///
/// # カリカリ notes
/// - `y_range` pre-computed once; avoids repeated subtraction in render loop.
/// - `update_rate_hz` uses branchless u8 clamp via `min`/`max`.
/// - Chart type selected via branchless index into a 3-entry array.
#[inline]
pub fn edge_to_view_sensor_config(
    data: &[i32],
    desired_update_hz: u8,
) -> EdgeViewSensorConfig {
    let (slope, intercept) = fit_linear_fixed(data);
    let use_linear = should_use_linear(data);

    // Data range: scan for min/max without sorting.
    let mut y_min = i32::MAX;
    let mut y_max = i32::MIN;
    for &v in data {
        // branchless min/max via i32 intrinsics (no if/else)
        y_min = y_min.min(v);
        y_max = y_max.max(v);
    }
    // Guard against empty slice.
    let y_min = if data.is_empty() { 0 } else { y_min };
    let y_max = if data.is_empty() { 0 } else { y_max };
    let y_range = y_max.wrapping_sub(y_min) as u32;

    // Chart type: branchless lookup.
    // use_linear=true & significant slope → Scatter; flat line → Line; else Bar.
    // Significant slope threshold: |slope| > 1 in Q16 = 65536.
    let slope_significant = (slope.unsigned_abs() > (1 << Q16_SHIFT)) as usize;
    let linear_index = use_linear as usize;
    // table[linear_index * 2 + slope_significant]
    const CHART_TABLE: [SensorChartType; 4] = [
        SensorChartType::Bar,     // !linear, flat
        SensorChartType::Bar,     // !linear, slope (shouldn't happen)
        SensorChartType::Line,    // linear, flat
        SensorChartType::Scatter, // linear, significant slope
    ];
    let chart_type = CHART_TABLE[linear_index * 2 + slope_significant];

    // Update rate: clamp to [1, 60] branchlessly.
    let update_rate_hz = desired_update_hz.max(1).min(60);

    // Content hash over slope, intercept, y_min, y_max.
    let mut key = [0u8; 16];
    key[0..4].copy_from_slice(&slope.to_le_bytes());
    key[4..8].copy_from_slice(&intercept.to_le_bytes());
    key[8..12].copy_from_slice(&y_min.to_le_bytes());
    key[12..16].copy_from_slice(&y_max.to_le_bytes());
    let content_hash = crate::hash::fnv1a(&key);

    EdgeViewSensorConfig {
        chart_type,
        y_min,
        y_max,
        y_range,
        update_rate_hz,
        content_hash,
    }
}

// ── Bridge 3: Edge → ASP (sensor data streaming config) ──────────────────

/// Quality mode for ASP packet encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AspQualityMode {
    /// High compression, low bandwidth (model-only transmission).
    ModelOnly = 0,
    /// Moderate: model + residual sketch.
    ModelPlusResidual = 1,
    /// Raw pass-through (no compression).
    Raw = 2,
}

/// Streaming configuration for ALICE-ASP packet assembly.
pub struct EdgeAspStreamConfig {
    /// Packets per second requested for this sensor channel.
    pub packet_rate_hz: u16,
    /// Payload size in bytes per packet (8 for model-only, up to 256 for raw).
    pub payload_bytes: u16,
    /// Quality / compression mode.
    pub quality_mode: AspQualityMode,
    /// Estimated bandwidth in bytes/sec (packet_rate * payload, pre-computed).
    pub bandwidth_bytes_per_sec: u32,
    /// Content hash for ASP session keying.
    pub content_hash: u64,
}

/// Derive an ASP streaming config from edge model characteristics.
///
/// `target_bandwidth_bps` is the channel budget in bytes/sec.
/// The function selects `AspQualityMode` and payload size to stay within budget.
///
/// # カリカリ notes
/// - Bandwidth fit uses reciprocal multiply to derive packet rate from budget.
/// - Quality mode selected via branchless array index (no if/else chain).
#[inline]
pub fn edge_to_asp_stream_config(
    data: &[i32],
    target_bandwidth_bps: u32,
) -> EdgeAspStreamConfig {
    let (slope, intercept) = fit_linear_fixed(data);
    let use_linear = should_use_linear(data);
    let error = compute_residual_error(data, slope, intercept);

    // Payload size table indexed by quality mode.
    // ModelOnly=8B (slope+intercept), ModelPlusResidual=64B, Raw=min(256, data*4).
    let raw_payload = ((data.len() * 4).min(256)) as u16;
    const PAYLOAD_TABLE: [u16; 3] = [8, 64, 0]; // 0 = fill from raw_payload below
    // Quality selection: prefer ModelOnly if fit is good, ModelPlusResidual if
    // moderate, Raw if fit is poor. Branchless: map use_linear + error level.
    // error < 1<<20 → good, error < 1<<30 → moderate, else poor.
    let good  = (error < (1i64 << 20)) as usize;
    let modr  = ((error >= (1i64 << 20)) & (error < (1i64 << 30))) as usize;
    // mode_index: 0=ModelOnly(good), 1=ModelPlusResidual(moderate), 2=Raw(poor).
    let mode_index = good * 0 + modr * 1 + (1 - good - modr).max(0) * 2;
    let quality_mode = [
        AspQualityMode::ModelOnly,
        AspQualityMode::ModelPlusResidual,
        AspQualityMode::Raw,
    ][mode_index];
    let base_payload = if mode_index == 2 { raw_payload } else { PAYLOAD_TABLE[mode_index] };

    // Derive packet rate: budget / payload_bytes, reciprocal multiply.
    // Avoid division: rate = budget * (1 / payload); use integer reciprocal via
    // saturating shift for power-of-two payloads, otherwise integer divide once.
    let payload_nonzero = base_payload.max(1) as u32;
    let packet_rate = (target_bandwidth_bps / payload_nonzero).min(1000) as u16;
    let packet_rate = packet_rate.max(1);

    // Pre-compute bandwidth.
    let bandwidth_bytes_per_sec = packet_rate as u32 * base_payload as u32;

    // use_linear suppresses unused-variable warning; fold into hash key.
    let linear_flag = use_linear as u8;
    let mut key = [0u8; 9];
    key[0..4].copy_from_slice(&slope.to_le_bytes());
    key[4..8].copy_from_slice(&intercept.to_le_bytes());
    key[8] = linear_flag;
    let content_hash = crate::hash::fnv1a(&key);

    EdgeAspStreamConfig {
        packet_rate_hz: packet_rate,
        payload_bytes: base_payload,
        quality_mode,
        bandwidth_bytes_per_sec,
        content_hash,
    }
}

// ── Bridge 4: Edge → Analytics (edge device performance metrics) ──────────

/// Performance metric snapshot for ALICE-Analytics ingestion.
pub struct EdgeAnalyticsMetrics {
    /// Estimated CPU utilisation in [0.0, 1.0].
    ///
    /// Derived from sample count and model complexity:
    /// fitting N samples costs O(N) work; we normalise against a 1024-sample budget.
    pub cpu_utilisation: f32,
    /// Fit quality in [0.0, 1.0] (mirrors `EdgeDbSensorModel::fit_quality`).
    pub fit_quality: f32,
    /// Throughput in samples/sec assuming 10 ms fit interval.
    pub throughput_samples_per_sec: u32,
    /// Model compression ratio: raw bytes / transmitted bytes.
    pub compression_ratio: f32,
    /// Content hash for Analytics dedup / time-series keying.
    pub content_hash: u64,
}

/// Derive edge device performance metrics for ALICE-Analytics.
///
/// # カリカリ notes
/// - CPU util: `n * RCP_1024` — reciprocal multiply, no division.
/// - Compression ratio: `raw / 8` simplified to `n * 4 * (1/8)` = `n * 0.5`.
///   Pre-computed as multiply by `RCP_8`.
/// - Throughput: `n * 100` (10 ms interval = 100 fits/sec, integer multiply).
#[inline]
pub fn edge_to_analytics_metrics(data: &[i32]) -> EdgeAnalyticsMetrics {
    let (slope, intercept) = fit_linear_fixed(data);
    let error = compute_residual_error(data, slope, intercept);
    let n = data.len();

    // CPU utilisation: n / 1024 clamped to [0,1].
    // Reciprocal: 1/1024 = 0.0009765625 (exact in f32).
    const RCP_1024: f32 = 1.0 / 1024.0;
    let cpu_utilisation = (n as f32 * RCP_1024).min(1.0);

    // Fit quality (same formula as Bridge 1).
    let err_norm = (error.unsigned_abs() as f64 * RCP_ERR_SCALE).min(1.0);
    let fit_quality = (1.0 - err_norm) as f32;

    // Throughput: assume fit runs every 10 ms → 100 fits/sec.
    let throughput_samples_per_sec = (n as u32).saturating_mul(100);

    // Compression ratio: raw = n*4 bytes, transmitted = 8 bytes (slope+intercept).
    // ratio = n*4/8 = n/2 — reciprocal multiply by 0.5.
    const RCP_8: f32 = 1.0 / 8.0;
    let raw_bytes = (n as f32) * 4.0;
    let compression_ratio = (raw_bytes * RCP_8).max(1.0);

    // Percent utilisation packed into hash key (0..100 as u8).
    let util_pct = (cpu_utilisation * 100.0) as u8;
    let quality_pct = (fit_quality * 100.0) as u8;
    let mut key = [0u8; 10];
    key[0..4].copy_from_slice(&slope.to_le_bytes());
    key[4..8].copy_from_slice(&intercept.to_le_bytes());
    key[8] = util_pct;
    key[9] = quality_pct;
    let content_hash = crate::hash::fnv1a(&key);

    EdgeAnalyticsMetrics {
        cpu_utilisation,
        fit_quality,
        throughput_samples_per_sec,
        compression_ratio,
        content_hash,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Shared test data: y = 10x + 50 (perfect linear trend, 8 samples).
    fn linear_data() -> [i32; 8] {
        [50, 60, 70, 80, 90, 100, 110, 120]
    }

    #[test]
    fn test_edge_to_db_sensor_model() {
        let data = linear_data();
        let rec = edge_to_db_sensor_model(&data);

        // Slope should be ~10 in Q16 = 655360.
        assert!((rec.slope_q16 - 655360).abs() < 1000, "slope_q16 = {}", rec.slope_q16);
        // Intercept should be ~50 in Q16 = 3276800.
        assert!((rec.intercept_q16 - 3276800).abs() < 1000, "intercept_q16 = {}", rec.intercept_q16);
        // f32 conversions.
        assert!((rec.slope_f32 - 10.0).abs() < 0.1, "slope_f32 = {}", rec.slope_f32);
        assert!((rec.intercept_f32 - 50.0).abs() < 0.1, "intercept_f32 = {}", rec.intercept_f32);
        // Perfect linear data → high fit quality.
        assert!(rec.fit_quality > 0.9, "fit_quality = {}", rec.fit_quality);
        // 8 samples.
        assert_eq!(rec.sample_count, 8);
        // Hash must be non-zero and non-FNV-offset (i.e. not the bare initialiser).
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.content_hash, 0xcbf29ce484222325);
        // Linear model should be selected for this trend.
        assert!(rec.model_is_linear);
    }

    #[test]
    fn test_edge_to_view_sensor_config() {
        let data = linear_data();
        let cfg = edge_to_view_sensor_config(&data, 30);

        // y_min=50, y_max=120, range=70.
        assert_eq!(cfg.y_min, 50);
        assert_eq!(cfg.y_max, 120);
        assert_eq!(cfg.y_range, 70);
        // Trending data → Scatter chart.
        assert_eq!(cfg.chart_type, SensorChartType::Scatter);
        // Update rate clamped: 30 is within [1, 60].
        assert_eq!(cfg.update_rate_hz, 30);
        // Hash populated.
        assert_ne!(cfg.content_hash, 0);

        // Edge case: clamping above 60.
        let cfg_fast = edge_to_view_sensor_config(&data, 200);
        assert_eq!(cfg_fast.update_rate_hz, 60);

        // Edge case: clamping below 1.
        let cfg_slow = edge_to_view_sensor_config(&data, 0);
        assert_eq!(cfg_slow.update_rate_hz, 1);
    }

    #[test]
    fn test_edge_to_asp_stream_config() {
        let data = linear_data();
        // Budget: 800 bytes/sec — enough for model-only at 100 pkt/s.
        let cfg = edge_to_asp_stream_config(&data, 800);

        // Perfect linear data → ModelOnly (smallest payload = 8 bytes).
        assert_eq!(cfg.payload_bytes, 8);
        assert_eq!(cfg.quality_mode, AspQualityMode::ModelOnly);
        // packet_rate = 800 / 8 = 100, clamped to ≤1000.
        assert_eq!(cfg.packet_rate_hz, 100);
        // bandwidth = 100 * 8 = 800.
        assert_eq!(cfg.bandwidth_bytes_per_sec, 800);
        // Hash populated.
        assert_ne!(cfg.content_hash, 0);

        // Minimum packet rate even with zero budget.
        let cfg_zero = edge_to_asp_stream_config(&data, 0);
        assert!(cfg_zero.packet_rate_hz >= 1);
    }

    #[test]
    fn test_edge_to_analytics_metrics() {
        let data = linear_data(); // 8 samples
        let metrics = edge_to_analytics_metrics(&data);

        // CPU: 8 / 1024 ≈ 0.0078, well under 1.0.
        assert!(metrics.cpu_utilisation > 0.0);
        assert!(metrics.cpu_utilisation < 0.1, "cpu = {}", metrics.cpu_utilisation);
        // Fit quality: perfect linear data → close to 1.0.
        assert!(metrics.fit_quality > 0.9, "quality = {}", metrics.fit_quality);
        // Throughput: 8 * 100 = 800.
        assert_eq!(metrics.throughput_samples_per_sec, 800);
        // Compression: 8*4/8 = 4.0x, clamped to ≥1.0.
        assert!((metrics.compression_ratio - 4.0).abs() < 0.01,
            "ratio = {}", metrics.compression_ratio);
        // Hash populated.
        assert_ne!(metrics.content_hash, 0);

        // Large slice: CPU utilisation must be clamped at 1.0.
        let big: Vec<i32> = (0..2048).map(|i| i * 5).collect();
        let m2 = edge_to_analytics_metrics(&big);
        assert_eq!(m2.cpu_utilisation, 1.0);
    }
}
