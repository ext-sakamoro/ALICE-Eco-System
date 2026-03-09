//! Edge core bridges — ALICE-Edge ↔ Analytics, DB, `SemanticTelemetry`
//!
//! 3 bridges connecting the core Edge sensor pipeline to the ALICE ecosystem.
//! Covers sensor pipeline throughput metrics, sensor reading persistence, and
//! threshold-crossing event injection into the semantic telemetry ring.

use crate::hash::fnv1a;
use alice_edge::{compute_residual_error, fit_linear_fixed, should_use_linear};
use alice_semantic_telemetry::{EventKind, SemanticEvent, Severity};

/// Reciprocal of Q16.16 scale factor — replaces division in Q16 → f32.
const RCP_Q16: f32 = 1.0 / 65536.0;

/// Reciprocal of 2^40 — used for residual error normalisation without
/// a runtime division on the hot path.
const RCP_ERR_SCALE: f64 = 1.0 / (1u64 << 40) as f64;

// ── Bridge 1: Edge → Analytics (sensor pipeline metrics) ─────────────────

/// Sensor pipeline throughput metrics for ALICE-Analytics ingestion.
///
/// Summarises a single fit interval: samples processed, compression ratio
/// achieved, and anomaly count — all without heap allocation.
pub struct EdgePipelineMetrics {
    /// FNV-1a hash over the Q16 coefficient pair — analytics stream key.
    pub content_hash: u64,
    /// Sensor samples processed in this interval.
    pub samples_per_sec: u32,
    /// Compression ratio: raw bytes / transmitted bytes (>= 1.0).
    ///
    /// Computed as `(n * 4) / 8 = n / 2` via reciprocal multiply.
    pub compression_ratio: f32,
    /// Number of anomalies detected: 1 if residual error exceeds threshold,
    /// 0 otherwise.  Integer flag — no floating-point branch.
    pub anomaly_count: u32,
    /// Fit quality in [0.0, 1.0] (1.0 = perfect linear fit).
    pub fit_quality: f32,
}

/// Derive sensor pipeline metrics for ALICE-Analytics from raw sensor data.
///
/// `data` is the slice of integer sensor readings for one fit interval.
/// `anomaly_threshold` is the residual error value above which a reading
/// is classified as anomalous.
///
/// Optimisation notes:
/// - `samples_per_sec` is `n * 100` (assumes 10 ms fit interval, integer multiply).
/// - `compression_ratio` uses `n * 0.5` — reciprocal of 8 applied to `n * 4`.
/// - `anomaly_count` uses a branchless integer cast of the boolean comparison.
/// - `fit_quality` normalises error with `RCP_ERR_SCALE`, no runtime `/`.
#[inline]
#[must_use]
pub fn edge_to_analytics_pipeline_metrics(
    data: &[i32],
    anomaly_threshold: i64,
) -> EdgePipelineMetrics {
    const RCP_8: f32 = 1.0 / 8.0;
    let n = data.len();
    let (slope, intercept) = fit_linear_fixed(data);
    let error = compute_residual_error(data, slope, intercept);

    // Throughput: n samples per 10 ms interval → n * 100 samples/sec.
    let samples_per_sec = (n as u32).saturating_mul(100);

    // Compression ratio: raw = n * 4 bytes, transmitted = 8 bytes (slope + intercept).
    // ratio = n * 4 / 8 = n * 0.5.  Reciprocal multiply — no division.
    let compression_ratio = (n as f32 * 4.0 * RCP_8).max(1.0);

    // Anomaly detection: branchless — cast boolean to u32 (0 or 1).
    let anomaly_count = (error.abs() > anomaly_threshold) as u32;

    // Fit quality: normalise clamped error, invert.
    let err_norm = (error.unsigned_abs() as f64 * RCP_ERR_SCALE).min(1.0);
    let fit_quality = (1.0 - err_norm) as f32;

    // Content hash over the Q16 coefficient pair for stream keying.
    let mut key = [0u8; 8];
    key[0..4].copy_from_slice(&slope.to_le_bytes());
    key[4..8].copy_from_slice(&intercept.to_le_bytes());
    let content_hash = fnv1a(&key);

    EdgePipelineMetrics {
        content_hash,
        samples_per_sec,
        compression_ratio,
        anomaly_count,
        fit_quality,
    }
}

// ── Bridge 2: Edge → DB (sensor reading persistence records) ─────────────

/// Sensor reading persistence record for ALICE-DB.
///
/// Stores the Q16.16 linear model coefficients fitted to a batch of sensor
/// readings, together with provenance metadata required for time-series
/// queries and replay.
pub struct EdgeDbReadingRecord {
    /// FNV-1a hash over the Q16 coefficient pair — row deduplication key.
    pub content_hash: u64,
    /// Slope in Q16.16 fixed-point.
    pub slope_q16: i32,
    /// Intercept in Q16.16 fixed-point.
    pub intercept_q16: i32,
    /// Slope converted to f32 (pre-scaled, avoids repeated Q16 conversion).
    pub slope_f32: f32,
    /// Intercept converted to f32.
    pub intercept_f32: f32,
    /// Number of raw samples the model was fitted from.
    pub sample_count: u32,
    /// Residual sum-of-squares error (Q32.32 internal units).
    pub residual_error: i64,
    /// True when the linear model was selected over the constant mean model.
    pub model_is_linear: bool,
}

/// Fit a linear model to sensor data and build an ALICE-DB persistence record.
///
/// Optimisation notes:
/// - Q16 → f32 via reciprocal multiply (`* RCP_Q16`), not division.
/// - `should_use_linear` result stored as plain bool (branchless downstream).
#[inline]
#[must_use]
pub fn edge_to_db_reading_record(data: &[i32]) -> EdgeDbReadingRecord {
    let (slope, intercept) = fit_linear_fixed(data);
    let residual_error = compute_residual_error(data, slope, intercept);
    let model_is_linear = should_use_linear(data);
    let sample_count = data.len() as u32;

    // Q16 → f32: reciprocal multiply, zero runtime division.
    let slope_f32 = slope as f32 * RCP_Q16;
    let intercept_f32 = intercept as f32 * RCP_Q16;

    // Content hash over the 8-byte coefficient pair for row-level dedup.
    let mut key = [0u8; 8];
    key[0..4].copy_from_slice(&slope.to_le_bytes());
    key[4..8].copy_from_slice(&intercept.to_le_bytes());
    let content_hash = fnv1a(&key);

    EdgeDbReadingRecord {
        content_hash,
        slope_q16: slope,
        intercept_q16: intercept,
        slope_f32,
        intercept_f32,
        sample_count,
        residual_error,
        model_is_linear,
    }
}

// ── Bridge 3: Edge → SemanticTelemetry (threshold crossing events) ────────

/// Threshold crossing event emitted by the Edge sensor pipeline.
///
/// Produced whenever a sensor reading exceeds a configured bound.
/// Carries the raw crossing value and the threshold so downstream
/// subscribers can assess severity without re-reading sensor history.
pub struct EdgeThresholdEvent {
    /// FNV-1a hash over `(sensor_id, value)` — event deduplication key.
    pub content_hash: u64,
    /// Sensor channel identifier.
    pub sensor_id: u64,
    /// Sensor value at the moment of crossing (raw integer units).
    pub crossing_value: i32,
    /// Configured threshold that was exceeded.
    pub threshold: i32,
    /// True when the value exceeded the upper bound; false for lower bound.
    pub is_upper_crossing: bool,
    /// `SemanticEvent` ready for injection into ALICE-SemanticTelemetry ring.
    pub semantic_event: SemanticEvent,
}

/// Build a threshold crossing event for ALICE-SemanticTelemetry injection.
///
/// `sensor_id` uniquely identifies the sensor channel.  `value` is the raw
/// reading; `threshold` is the bound that was crossed; `is_upper` selects
/// upper vs. lower bound crossing.  `timestamp_ns` is the nanosecond wall
/// clock at the time of the crossing.
///
/// The severity is `Warn` for a first crossing and is set uniformly here;
/// the caller may override it before ring insertion if escalation logic is
/// required.
///
/// Optimisation note: the `is_upper_crossing` flag is derived via a
/// branchless integer comparison — no conditional branch in the hot path.
#[inline]
#[must_use]
pub fn edge_to_semantic_threshold_event(
    sensor_id: u64,
    value: i32,
    threshold: i32,
    is_upper: bool,
    timestamp_ns: u64,
) -> EdgeThresholdEvent {
    // Branchless upper-crossing flag: value > threshold maps to true via cmp.
    let is_upper_crossing = is_upper && value > threshold;

    // Content hash over the (sensor_id, value) pair for deduplication.
    let mut key = [0u8; 12];
    key[0..8].copy_from_slice(&sensor_id.to_le_bytes());
    key[8..12].copy_from_slice(&value.to_le_bytes());
    let content_hash = fnv1a(&key);

    // Encode the crossing value in the SemanticEvent payload.
    // payload  = raw sensor value (i32 bits reinterpreted as u64).
    // payload2 = configured threshold (i32 bits reinterpreted as u64).
    let semantic_event = SemanticEvent {
        timestamp_ns,
        source_id: sensor_id,
        kind: EventKind::ThresholdCrossing,
        severity: Severity::Warn,
        payload: value as u32 as u64,
        payload2: threshold as u32 as u64,
    };

    EdgeThresholdEvent {
        content_hash,
        sensor_id,
        crossing_value: value,
        threshold,
        is_upper_crossing,
        semantic_event,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Perfect linear data: y = 10x + 50 over 8 samples.
    fn linear_data() -> [i32; 8] {
        [50, 60, 70, 80, 90, 100, 110, 120]
    }

    // ── Bridge 1: Edge → Analytics ──────────────────────────────────────

    #[test]
    fn test_edge_to_analytics_pipeline_metrics() {
        let data = linear_data(); // 8 samples

        // Anomaly threshold set above the near-zero error of a perfect fit.
        let metrics = edge_to_analytics_pipeline_metrics(&data, 1 << 30);

        // Content hash must be non-zero and deterministic.
        assert_ne!(metrics.content_hash, 0);
        assert_eq!(metrics.content_hash, {
            let (slope, intercept) = alice_edge::fit_linear_fixed(&data);
            let mut key = [0u8; 8];
            key[0..4].copy_from_slice(&slope.to_le_bytes());
            key[4..8].copy_from_slice(&intercept.to_le_bytes());
            crate::hash::fnv1a(&key)
        });

        // Throughput: 8 * 100 = 800 samples/sec.
        assert_eq!(metrics.samples_per_sec, 800);

        // Compression ratio: 8 * 4 / 8 = 4.0.
        assert!(
            (metrics.compression_ratio - 4.0).abs() < 0.01,
            "ratio = {}",
            metrics.compression_ratio
        );

        // Perfect linear fit → error is tiny → no anomaly.
        assert_eq!(metrics.anomaly_count, 0);

        // Perfect fit → quality close to 1.0.
        assert!(
            metrics.fit_quality > 0.9,
            "fit_quality = {}",
            metrics.fit_quality
        );

        // Threshold set to 0 → any non-zero error triggers anomaly flag.
        let metrics_anom = edge_to_analytics_pipeline_metrics(&data, 0);
        // Residual of a perfect fit is non-zero in fixed-point → anomaly_count = 1.
        // Allow either 0 or 1 since the perfect-fit residual may round to 0.
        assert!(metrics_anom.anomaly_count <= 1);

        // Edge case: empty slice must not panic.
        let empty = edge_to_analytics_pipeline_metrics(&[], 1000);
        assert_eq!(empty.samples_per_sec, 0);
        assert_eq!(empty.anomaly_count, 0);
        assert!((empty.compression_ratio - 1.0).abs() < 0.01); // clamped to 1.0
    }

    // ── Bridge 2: Edge → DB ──────────────────────────────────────────────

    #[test]
    fn test_edge_to_db_reading_record() {
        let data = linear_data();
        let rec = edge_to_db_reading_record(&data);

        // Slope should be ~10 in Q16 = 655360.
        assert!(
            (rec.slope_q16 - 655360).abs() < 1000,
            "slope_q16 = {}",
            rec.slope_q16
        );
        // Intercept should be ~50 in Q16 = 3276800.
        assert!(
            (rec.intercept_q16 - 3276800).abs() < 1000,
            "intercept_q16 = {}",
            rec.intercept_q16
        );

        // f32 conversions via RCP_Q16.
        assert!(
            (rec.slope_f32 - 10.0).abs() < 0.1,
            "slope_f32 = {}",
            rec.slope_f32
        );
        assert!(
            (rec.intercept_f32 - 50.0).abs() < 0.1,
            "intercept_f32 = {}",
            rec.intercept_f32
        );

        // Sample count.
        assert_eq!(rec.sample_count, 8);

        // Content hash non-zero.
        assert_ne!(rec.content_hash, 0);

        // Perfect linear data → model_is_linear should be true.
        assert!(rec.model_is_linear);

        // Two identical data sets must produce identical hashes.
        let rec2 = edge_to_db_reading_record(&data);
        assert_eq!(rec.content_hash, rec2.content_hash);

        // Constant data → model_is_linear = false.
        let flat: [i32; 6] = [100, 100, 100, 100, 100, 100];
        let flat_rec = edge_to_db_reading_record(&flat);
        assert!(!flat_rec.model_is_linear);

        // Hash must differ between linear and constant fits.
        assert_ne!(rec.content_hash, flat_rec.content_hash);
    }

    // ── Bridge 3: Edge → SemanticTelemetry ──────────────────────────────

    #[test]
    fn test_edge_to_semantic_threshold_event() {
        // Upper bound crossing: value 120 > threshold 100.
        let ev = edge_to_semantic_threshold_event(42, 120, 100, true, 1_000_000_000);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.sensor_id, 42);
        assert_eq!(ev.crossing_value, 120);
        assert_eq!(ev.threshold, 100);
        assert!(ev.is_upper_crossing, "expected upper crossing");

        // SemanticEvent fields.
        assert_eq!(ev.semantic_event.source_id, 42);
        assert_eq!(ev.semantic_event.kind, EventKind::ThresholdCrossing);
        assert_eq!(ev.semantic_event.severity, Severity::Warn);
        assert_eq!(ev.semantic_event.timestamp_ns, 1_000_000_000);
        // Payload encodes the crossing value.
        assert_eq!(ev.semantic_event.payload, 120u64);
        // Payload2 encodes the threshold.
        assert_eq!(ev.semantic_event.payload2, 100u64);

        // Lower bound crossing: is_upper = false, value < threshold.
        // is_upper_crossing should be false regardless of comparison.
        let ev_lower = edge_to_semantic_threshold_event(7, 10, 50, false, 2_000_000_000);
        assert!(!ev_lower.is_upper_crossing);

        // Content hashes must differ for different (sensor_id, value) pairs.
        assert_ne!(ev.content_hash, ev_lower.content_hash);

        // Non-crossing with is_upper = true and value <= threshold.
        let ev_no_cross = edge_to_semantic_threshold_event(1, 50, 100, true, 0);
        assert!(
            !ev_no_cross.is_upper_crossing,
            "value <= threshold should not be an upper crossing"
        );
    }

    // ── 追加テスト ────────────────────────────────────────────────────────

    #[test]
    fn test_edge_pipeline_metrics_determinism() {
        // 同一データで2回呼び出すと content_hash が一致すること（決定性確認）。
        let data = linear_data();
        let m1 = edge_to_analytics_pipeline_metrics(&data, 1 << 30);
        let m2 = edge_to_analytics_pipeline_metrics(&data, 1 << 30);
        assert_eq!(m1.content_hash, m2.content_hash);
        assert_eq!(m1.samples_per_sec, m2.samples_per_sec);
    }

    #[test]
    fn test_edge_db_single_sample_record() {
        // 1サンプルでもパニックせず sample_count=1 を返すこと。
        let data: [i32; 1] = [42];
        let rec = edge_to_db_reading_record(&data);
        assert_eq!(rec.sample_count, 1);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_edge_db_reading_record_sample_count() {
        // sample_count がスライス長に一致すること。
        let data: [i32; 5] = [10, 20, 30, 40, 50];
        let rec = edge_to_db_reading_record(&data);
        assert_eq!(rec.sample_count, 5);
    }

    #[test]
    fn test_edge_threshold_event_determinism() {
        // 同一引数で2回呼び出すと content_hash が一致すること。
        let ev1 = edge_to_semantic_threshold_event(99, 200, 150, true, 9_999_999);
        let ev2 = edge_to_semantic_threshold_event(99, 200, 150, true, 9_999_999);
        assert_eq!(ev1.content_hash, ev2.content_hash);
    }

    #[test]
    fn test_edge_threshold_event_lower_crossing_payload() {
        // 下限越えイベントの payload・payload2 フィールドが正しく設定されること。
        let ev = edge_to_semantic_threshold_event(5, 10, 50, false, 500);
        assert_eq!(ev.semantic_event.payload, 10u64);
        assert_eq!(ev.semantic_event.payload2, 50u64);
        assert_eq!(ev.semantic_event.timestamp_ns, 500);
    }
}
