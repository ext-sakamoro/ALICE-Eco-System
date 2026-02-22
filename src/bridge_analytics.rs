//! Analytics bridges — ALICE-Analytics ↔ DB, Cache, CDN, ML, Search, View, Edge
//!
//! 7 bridges connecting streaming analytics sketches to the ALICE ecosystem.

use alice_analytics::prelude::*;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Analytics → DB (metric snapshot persistence) ──────────────

/// Metric snapshot record for ALICE-DB persistence.
pub struct AnalyticsDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// P50 latency (quantile).
    pub p50: f64,
    /// P99 latency (quantile).
    pub p99: f64,
    /// Total observation count.
    pub count: u64,
}

/// Serialize analytics snapshot for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn analytics_to_db_record(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsDbRecord {
    let card = hll.cardinality();
    let count = dd.count();
    let data = [card.to_le_bytes().as_slice(), &count.to_le_bytes()].concat();
    AnalyticsDbRecord {
        content_hash: fnv1a(&data),
        cardinality: card,
        p50: dd.quantile(0.50),
        p99: dd.quantile(0.99),
        count,
    }
}

// ── Bridge 2: Analytics → Cache (sketch caching) ────────────────────────

/// Sketch cache entry for ALICE-Cache.
pub struct AnalyticsCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// Observation count.
    pub count: u64,
}

/// Cache analytics sketch for ALICE-Cache.
#[inline]
#[must_use]
pub fn analytics_to_cache_entry(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsCacheEntry {
    let card = hll.cardinality();
    let count = dd.count();
    let data = card.to_le_bytes();
    AnalyticsCacheEntry {
        content_hash: fnv1a(&data),
        cardinality: card,
        count,
    }
}

// ── Bridge 3: Analytics → CDN (aggregated report delivery) ──────────────

/// Aggregated report for ALICE-CDN delivery.
pub struct AnalyticsCdnReport {
    /// Content hash.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// P50.
    pub p50: f64,
    /// P95.
    pub p95: f64,
    /// P99.
    pub p99: f64,
    /// MIME type.
    pub content_type: &'static str,
}

/// Package analytics report for ALICE-CDN delivery.
#[inline]
#[must_use]
pub fn analytics_to_cdn_report(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsCdnReport {
    let card = hll.cardinality();
    let p50 = dd.quantile(0.50);
    let p99 = dd.quantile(0.99);
    let data = [
        card.to_le_bytes().as_slice(),
        &p50.to_le_bytes(),
        &p99.to_le_bytes(),
    ]
    .concat();
    AnalyticsCdnReport {
        content_hash: fnv1a(&data),
        cardinality: card,
        p50,
        p95: dd.quantile(0.95),
        p99,
        content_type: "application/x-alice-analytics",
    }
}

// ── Bridge 4: Analytics → ML (feature extraction for anomaly detection) ──

/// ML feature vector from analytics for anomaly detection.
pub struct AnalyticsMlFeatures {
    /// Cardinality (normalized).
    pub cardinality: f64,
    /// Mean value.
    pub mean: f64,
    /// P50.
    pub p50: f64,
    /// P99.
    pub p99: f64,
    /// Count.
    pub count: u64,
    /// Feature vector for ML input.
    pub feature_vec: Vec<f64>,
}

/// Extract ML features from analytics for ALICE-ML anomaly detection.
#[inline]
#[must_use]
pub fn analytics_to_ml_features(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsMlFeatures {
    let card = hll.cardinality();
    let mean = dd.mean();
    let p50 = dd.quantile(0.50);
    let p99 = dd.quantile(0.99);
    let count = dd.count();
    AnalyticsMlFeatures {
        cardinality: card,
        mean,
        p50,
        p99,
        count,
        feature_vec: vec![card, mean, p50, p99, count as f64],
    }
}

// ── Bridge 5: Analytics → Search (metric index) ─────────────────────────

/// Metric search index for ALICE-Search.
pub struct AnalyticsSearchIndex {
    /// Content hash for search key.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// Count.
    pub count: u64,
    /// Mean value.
    pub mean: f64,
}

/// Index analytics metrics for ALICE-Search.
#[inline]
#[must_use]
pub fn analytics_to_search_index(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsSearchIndex {
    let card = hll.cardinality();
    let data = card.to_le_bytes();
    AnalyticsSearchIndex {
        content_hash: fnv1a(&data),
        cardinality: card,
        count: dd.count(),
        mean: dd.mean(),
    }
}

// ── Bridge 6: Analytics → View (dashboard visualization) ─────────────────

/// Dashboard visualization config for ALICE-View.
pub struct AnalyticsViewConfig {
    /// Cardinality estimate.
    pub cardinality: f64,
    /// P50.
    pub p50: f64,
    /// P95.
    pub p95: f64,
    /// P99.
    pub p99: f64,
    /// Count.
    pub count: u64,
    /// Mean.
    pub mean: f64,
}

/// Configure dashboard visualization for ALICE-View.
#[inline]
#[must_use]
pub fn analytics_to_view_config(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsViewConfig {
    AnalyticsViewConfig {
        cardinality: hll.cardinality(),
        p50: dd.quantile(0.50),
        p95: dd.quantile(0.95),
        p99: dd.quantile(0.99),
        count: dd.count(),
        mean: dd.mean(),
    }
}

// ── Bridge 7: Analytics → Edge (lightweight telemetry) ───────────────────

/// Lightweight telemetry payload for ALICE-Edge.
pub struct AnalyticsEdgePayload {
    /// Content hash.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// Count.
    pub count: u64,
    /// Payload size (estimated bytes).
    pub estimated_bytes: usize,
}

/// Prepare lightweight telemetry for ALICE-Edge devices.
#[inline]
#[must_use]
pub fn analytics_to_edge_payload(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsEdgePayload {
    let card = hll.cardinality();
    let count = dd.count();
    let data = [card.to_le_bytes().as_slice(), &count.to_le_bytes()].concat();
    // Compact payload: 8 bytes cardinality + 8 bytes count = 16 bytes
    AnalyticsEdgePayload {
        content_hash: fnv1a(&data),
        cardinality: card,
        count,
        estimated_bytes: 16,
    }
}

// ── Bridge 8: API → Analytics (rate-limit metrics) ───────────────────────

/// Rate-limit window metrics from ALICE-API for Analytics ingestion.
pub struct ApiAnalyticsMetrics {
    /// Content hash over the window interval endpoints.
    pub content_hash: u64,
    /// Total requests observed in the window.
    pub total_requests: u64,
    /// Number of requests rejected by the rate limiter.
    pub rate_limited: u64,
    /// Average end-to-end request latency in microseconds.
    pub avg_latency_us: f64,
    /// Total error responses (status >= 500) in the window.
    pub error_count: u64,
    /// Window start timestamp in nanoseconds.
    pub window_start_ns: u64,
    /// Window end timestamp in nanoseconds.
    pub window_end_ns: u64,
}

/// Build API rate-limit window metrics for ALICE-Analytics ingestion.
///
/// `avg_latency_us` is derived from `total_latency_ns` using reciprocal
/// multiply — no bare `/` in the hot path.
#[inline]
#[must_use]
pub fn api_to_analytics_metrics(
    total_requests: u64,
    rate_limited: u64,
    total_latency_ns: u64,
    error_count: u64,
    window_start_ns: u64,
    window_end_ns: u64,
) -> ApiAnalyticsMetrics {
    const RCP_NS_TO_US: f64 = 1.0 / 1_000.0;
    // Reciprocal of request count — hoisted so all per-request averages share it.
    let rcp_requests = 1.0 / total_requests.max(1) as f64;
    let avg_latency_us = total_latency_ns as f64 * RCP_NS_TO_US * rcp_requests;

    // Hash over window endpoints so each window gets a unique key.
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&window_start_ns.to_le_bytes());
    key[8..16].copy_from_slice(&window_end_ns.to_le_bytes());
    ApiAnalyticsMetrics {
        content_hash: fnv1a(&key),
        total_requests,
        rate_limited,
        avg_latency_us,
        error_count,
        window_start_ns,
        window_end_ns,
    }
}

// ── Bridge 9: Motion → Analytics (trajectory metrics) ────────────────────

/// Trajectory planning metrics from ALICE-Motion for Analytics ingestion.
pub struct MotionAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Number of trajectories computed in the reporting period.
    pub trajectories_computed: u64,
    /// Average number of path segments per trajectory.
    pub avg_segments: f64,
    /// Average trajectory duration in milliseconds.
    pub avg_duration_ms: f64,
    /// Maximum jerk observed across all trajectories (mm/s³).
    pub max_jerk: f32,
    /// Average trajectory planning time in microseconds.
    pub planning_time_us: f64,
}

/// Build trajectory planning metrics for ALICE-Analytics ingestion.
///
/// All averages use reciprocal multiply against `trajectories_computed`
/// to avoid repeated division.
#[inline]
#[must_use]
pub fn motion_to_analytics_metrics(
    trajectories_computed: u64,
    total_segments: u64,
    total_duration_ms: f64,
    max_jerk: f32,
    total_planning_time_us: f64,
) -> MotionAnalyticsMetrics {
    // Reciprocal of count — shared across all average computations.
    let rcp_count = 1.0 / trajectories_computed.max(1) as f64;
    let avg_segments = total_segments as f64 * rcp_count;
    let avg_duration_ms = total_duration_ms * rcp_count;
    let planning_time_us = total_planning_time_us * rcp_count;

    // Hash over the core metrics.
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&trajectories_computed.to_le_bytes());
    key[8..16].copy_from_slice(&total_segments.to_le_bytes());
    key[16..20].copy_from_slice(&max_jerk.to_le_bytes());
    key[20..24].copy_from_slice(&(avg_duration_ms.to_bits() as u32).to_le_bytes());
    MotionAnalyticsMetrics {
        content_hash: fnv1a(&key),
        trajectories_computed,
        avg_segments,
        avg_duration_ms,
        max_jerk,
        planning_time_us,
    }
}

// ── Bridge 10: Edge → Analytics (sensor compression metrics) ─────────────

/// Sensor compression metrics from ALICE-Edge for Analytics ingestion.
pub struct EdgeAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total sensor samples processed in the reporting period.
    pub samples_processed: u64,
    /// Average compression ratio (raw bytes / transmitted bytes).
    pub avg_compression_ratio: f32,
    /// Number of distinct sensor channels active in the period.
    pub sensor_count: u32,
    /// Number of anomalies detected across all sensor channels.
    pub anomalies_detected: u64,
}

/// Build sensor compression metrics for ALICE-Analytics ingestion.
///
/// `avg_compression_ratio` uses reciprocal multiply against `sensor_count`
/// to sum per-channel ratios without repeated division.
#[inline]
#[must_use]
pub fn edge_to_analytics_metrics(
    samples_processed: u64,
    total_compression_ratio: f32,
    sensor_count: u32,
    anomalies_detected: u64,
) -> EdgeAnalyticsMetrics {
    // Reciprocal of sensor_count — one division hoisted outside the logical loop.
    const RCP_FALLBACK: f32 = 1.0;
    let rcp_sensors = if sensor_count > 0 {
        1.0 / sensor_count as f32
    } else {
        RCP_FALLBACK
    };
    let avg_compression_ratio = total_compression_ratio * rcp_sensors;

    // Hash over the primary metric tuple.
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&samples_processed.to_le_bytes());
    key[8..12].copy_from_slice(&sensor_count.to_le_bytes());
    key[12..20].copy_from_slice(&anomalies_detected.to_le_bytes());
    EdgeAnalyticsMetrics {
        content_hash: fnv1a(&key),
        samples_processed,
        avg_compression_ratio,
        sensor_count,
        anomalies_detected,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_sketches() -> (HyperLogLog12, DDSketch256) {
        let mut hll = HyperLogLog12::new();
        let mut dd = DDSketch256::new(0.01);
        for i in 0..100u64 {
            hll.insert_hash(FnvHasher::hash_u64(i));
            dd.insert(i as f64);
        }
        (hll, dd)
    }

    #[test]
    fn test_analytics_to_db_record() {
        let (hll, dd) = test_sketches();
        let rec = analytics_to_db_record(&hll, &dd);
        assert_ne!(rec.content_hash, 0);
        assert!(rec.cardinality > 0.0);
        assert_eq!(rec.count, 100);
    }

    #[test]
    fn test_analytics_to_cache_entry() {
        let (hll, dd) = test_sketches();
        let entry = analytics_to_cache_entry(&hll, &dd);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.count, 100);
    }

    #[test]
    fn test_analytics_to_cdn_report() {
        let (hll, dd) = test_sketches();
        let rpt = analytics_to_cdn_report(&hll, &dd);
        assert_ne!(rpt.content_hash, 0);
        assert_eq!(rpt.content_type, "application/x-alice-analytics");
    }

    #[test]
    fn test_analytics_to_ml_features() {
        let (hll, dd) = test_sketches();
        let f = analytics_to_ml_features(&hll, &dd);
        assert_eq!(f.feature_vec.len(), 5);
        assert_eq!(f.count, 100);
    }

    #[test]
    fn test_analytics_to_search_index() {
        let (hll, dd) = test_sketches();
        let idx = analytics_to_search_index(&hll, &dd);
        assert_ne!(idx.content_hash, 0);
        assert_eq!(idx.count, 100);
    }

    #[test]
    fn test_analytics_to_view_config() {
        let (hll, dd) = test_sketches();
        let cfg = analytics_to_view_config(&hll, &dd);
        assert!(cfg.cardinality > 0.0);
        assert_eq!(cfg.count, 100);
    }

    #[test]
    fn test_analytics_to_edge_payload() {
        let (hll, dd) = test_sketches();
        let payload = analytics_to_edge_payload(&hll, &dd);
        assert_ne!(payload.content_hash, 0);
        assert_eq!(payload.estimated_bytes, 16);
    }

    #[test]
    fn test_api_to_analytics_metrics() {
        // 1000 requests, 50 rate-limited, total latency 500 ms (500_000_000 ns),
        // 10 errors, window [1_000_000_000, 2_000_000_000].
        let m = api_to_analytics_metrics(1_000, 50, 500_000_000, 10, 1_000_000_000, 2_000_000_000);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.total_requests, 1_000);
        assert_eq!(m.rate_limited, 50);
        assert_eq!(m.error_count, 10);
        // avg_latency_us = (500_000_000 / 1000) / 1000 = 500 µs
        assert!(
            (m.avg_latency_us - 500.0).abs() < 0.01,
            "avg_latency_us = {}",
            m.avg_latency_us
        );
        assert_eq!(m.window_start_ns, 1_000_000_000);
        assert_eq!(m.window_end_ns, 2_000_000_000);
    }

    #[test]
    fn test_api_to_analytics_metrics_zero_requests() {
        // Zero-request window must not panic (denom saturated to 1).
        let m = api_to_analytics_metrics(0, 0, 0, 0, 0, 1_000_000);
        assert_eq!(m.total_requests, 0);
        assert_eq!(m.avg_latency_us, 0.0);
        // Hash over window endpoints must still be non-zero.
        assert_ne!(m.content_hash, 0);
    }

    #[test]
    fn test_motion_to_analytics_metrics() {
        // 100 trajectories, 500 total segments, 2000.0 ms total, max_jerk 5.0, 300 µs total planning.
        let m = motion_to_analytics_metrics(100, 500, 2_000.0, 5.0, 300.0);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.trajectories_computed, 100);
        // avg_segments = 500 / 100 = 5.0
        assert!(
            (m.avg_segments - 5.0).abs() < 0.001,
            "avg_segments = {}",
            m.avg_segments
        );
        // avg_duration_ms = 2000 / 100 = 20.0
        assert!(
            (m.avg_duration_ms - 20.0).abs() < 0.001,
            "avg_duration_ms = {}",
            m.avg_duration_ms
        );
        assert!((m.max_jerk - 5.0).abs() < 0.001);
        // planning_time_us = 300 / 100 = 3.0
        assert!(
            (m.planning_time_us - 3.0).abs() < 0.001,
            "planning_time_us = {}",
            m.planning_time_us
        );
    }

    #[test]
    fn test_motion_to_analytics_metrics_zero_trajectories() {
        // Zero-trajectory case must not panic (rcp_count uses max(1)).
        let m = motion_to_analytics_metrics(0, 0, 0.0, 0.0, 0.0);
        assert_eq!(m.trajectories_computed, 0);
        assert_eq!(m.avg_segments, 0.0);
        assert_eq!(m.avg_duration_ms, 0.0);
    }

    #[test]
    fn test_edge_to_analytics_metrics_bridge() {
        // 4 sensors, total compression ratio sum = 16.0 (avg 4.0x per sensor),
        // 8000 samples, 3 anomalies.
        let m = edge_to_analytics_metrics(8_000, 16.0, 4, 3);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.samples_processed, 8_000);
        assert_eq!(m.sensor_count, 4);
        assert_eq!(m.anomalies_detected, 3);
        // avg_compression_ratio = 16.0 / 4 = 4.0
        assert!(
            (m.avg_compression_ratio - 4.0).abs() < 0.001,
            "avg_compression_ratio = {}",
            m.avg_compression_ratio
        );
    }

    #[test]
    fn test_edge_to_analytics_metrics_zero_sensors() {
        // Zero-sensor case must not panic; ratio falls back to total * 1.0.
        let m = edge_to_analytics_metrics(0, 0.0, 0, 0);
        assert_eq!(m.sensor_count, 0);
        assert_eq!(m.avg_compression_ratio, 0.0);
    }
}
