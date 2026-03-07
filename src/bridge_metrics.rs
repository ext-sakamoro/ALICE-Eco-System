//! Metrics bridges — ALICE-Metrics ↔ DB, Cache, Analytics, CDN, Edge
//!
//! 5 bridges connecting the metrics layer to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Metrics → DB (metrics persistence) ─────────────────────────

/// Metrics persistence record for ALICE-DB.
///
/// Captures a point-in-time snapshot of a counter/gauge pair for durable
/// storage so historical dashboards can replay metric evolution over time.
pub struct MetricsDbRecord {
    /// FNV-1a hash over metric name and label set.
    pub content_hash: u64,
    /// Metric name (truncated to 64 bytes for the hash key).
    pub name_hash: u64,
    /// Current counter value at snapshot time.
    pub counter_value: u64,
    /// Current gauge value at snapshot time.
    pub gauge_value: f64,
    /// Number of histogram observations recorded.
    pub histogram_count: u64,
    /// Snapshot timestamp in milliseconds.
    pub snapshot_at_ms: u64,
}

/// Serialize a metrics snapshot for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn metrics_to_db_record(
    name: &str,
    labels: &str,
    counter_value: u64,
    gauge_value: f64,
    histogram_count: u64,
    snapshot_at_ms: u64,
) -> MetricsDbRecord {
    let name_hash = fnv1a(name.as_bytes());
    let mut data = [0u8; 24];
    data[0..8].copy_from_slice(&name_hash.to_le_bytes());
    data[8..16].copy_from_slice(&fnv1a(labels.as_bytes()).to_le_bytes());
    data[16..24].copy_from_slice(&snapshot_at_ms.to_le_bytes());
    MetricsDbRecord {
        content_hash: fnv1a(&data),
        name_hash,
        counter_value,
        gauge_value,
        histogram_count,
        snapshot_at_ms,
    }
}

// ── Bridge 2: Metrics → Cache (metrics snapshot cache) ───────────────────

/// Metrics snapshot cache entry for ALICE-Cache.
///
/// Short-lived cache entries allow dashboard queries to avoid hitting
/// the metrics store on every request.  TTL is computed branchlessly
/// from the metric freshness window.
pub struct MetricsCacheEntry {
    /// FNV-1a hash over name and label set — cache key.
    pub content_hash: u64,
    /// Counter value at cache time.
    pub counter_value: u64,
    /// Gauge value at cache time.
    pub gauge_value: f64,
    /// Cache TTL in seconds (branchless: longer for stable metrics).
    pub ttl_secs: u32,
    /// Entry size in bytes (estimated).
    pub entry_bytes: usize,
}

/// Build a metrics snapshot cache entry for ALICE-Cache.
///
/// `stable_metric`: when true the TTL is extended to 60 s; otherwise 10 s.
/// The selection is branchless using integer arithmetic.
#[inline]
#[must_use]
pub fn metrics_to_cache_entry(
    name: &str,
    labels: &str,
    counter_value: u64,
    gauge_value: f64,
    stable_metric: bool,
) -> MetricsCacheEntry {
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&fnv1a(name.as_bytes()).to_le_bytes());
    data[8..16].copy_from_slice(&fnv1a(labels.as_bytes()).to_le_bytes());
    let content_hash = fnv1a(&data);
    // Branchless TTL: stable → 60 s, volatile → 10 s.
    let stable = stable_metric as u32;
    let ttl_secs = 10u32 + stable * 50u32;
    MetricsCacheEntry {
        content_hash,
        counter_value,
        gauge_value,
        ttl_secs,
        entry_bytes: 48,
    }
}

// ── Bridge 3: Metrics → Analytics (metrics aggregation) ──────────────────

/// Metrics aggregation payload for ALICE-Analytics.
///
/// Feeds counter deltas and gauge samples into the analytics pipeline so
/// SLI/SLO computations and anomaly detection have fresh metric data.
pub struct MetricsAnalyticsPayload {
    /// FNV-1a hash over name and label set.
    pub content_hash: u64,
    /// Counter delta since the previous aggregation window.
    pub counter_delta: u64,
    /// Gauge value for the current window.
    pub gauge_value: f64,
    /// Summary observation count for the window.
    pub observation_count: u64,
    /// Summary sum for the window.
    pub observation_sum: f64,
    /// P50 quantile from the summary.
    pub p50: f64,
    /// P99 quantile from the summary.
    pub p99: f64,
}

/// Build a metrics aggregation payload for ALICE-Analytics.
#[inline]
#[must_use]
pub fn metrics_to_analytics_payload(
    name: &str,
    labels: &str,
    counter_delta: u64,
    gauge_value: f64,
    observation_count: u64,
    observation_sum: f64,
    p50: f64,
    p99: f64,
) -> MetricsAnalyticsPayload {
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&fnv1a(name.as_bytes()).to_le_bytes());
    data[8..16].copy_from_slice(&fnv1a(labels.as_bytes()).to_le_bytes());
    MetricsAnalyticsPayload {
        content_hash: fnv1a(&data),
        counter_delta,
        gauge_value,
        observation_count,
        observation_sum,
        p50,
        p99,
    }
}

// ── Bridge 4: Metrics → CDN (dashboard delivery) ─────────────────────────

/// Metrics dashboard package for ALICE-CDN delivery.
///
/// Serialized metric summaries are pushed to the CDN edge so that
/// global dashboards can render without round-tripping to the origin.
pub struct MetricsCdnPackage {
    /// FNV-1a hash over the Prometheus export payload.
    pub content_hash: u64,
    /// Prometheus text exposition (truncated hash for routing).
    pub export_hash: u64,
    /// Number of metric families included.
    pub family_count: u32,
    /// Estimated payload size in bytes.
    pub payload_bytes: usize,
    /// MIME type for CDN content negotiation.
    pub content_type: &'static str,
    /// Cache-Control max-age in seconds.
    pub max_age_secs: u32,
}

/// Package a Prometheus metrics export for ALICE-CDN delivery.
#[inline]
#[must_use]
pub fn metrics_to_cdn_package(
    prometheus_export: &str,
    family_count: u32,
    max_age_secs: u32,
) -> MetricsCdnPackage {
    let export_bytes = prometheus_export.as_bytes();
    let export_hash = fnv1a(export_bytes);
    let mut data = [0u8; 12];
    data[0..8].copy_from_slice(&export_hash.to_le_bytes());
    data[8..12].copy_from_slice(&family_count.to_le_bytes());
    MetricsCdnPackage {
        content_hash: fnv1a(&data),
        export_hash,
        family_count,
        payload_bytes: export_bytes.len(),
        content_type: "text/plain; version=0.0.4; charset=utf-8",
        max_age_secs,
    }
}

// ── Bridge 5: Metrics → Edge (metrics forwarding) ────────────────────────

/// Compact metrics forward payload for ALICE-Edge.
///
/// Edge nodes receive a stripped-down binary metric summary to minimise
/// bandwidth on constrained links.  Only the most actionable values are
/// forwarded; full Prometheus text is retained at the origin.
pub struct MetricsEdgeForward {
    /// FNV-1a hash over name and the forwarded values.
    pub content_hash: u64,
    /// Metric name hash for edge-side routing.
    pub name_hash: u64,
    /// Counter value (raw, not delta — edge reconciles on receipt).
    pub counter_value: u64,
    /// Gauge value.
    pub gauge_value: f64,
    /// Estimated payload bytes on the wire.
    pub wire_bytes: usize,
}

/// Build a compact metrics forward payload for ALICE-Edge.
#[inline]
#[must_use]
pub fn metrics_to_edge_forward(
    name: &str,
    counter_value: u64,
    gauge_value: f64,
) -> MetricsEdgeForward {
    let name_hash = fnv1a(name.as_bytes());
    let mut data = [0u8; 24];
    data[0..8].copy_from_slice(&name_hash.to_le_bytes());
    data[8..16].copy_from_slice(&counter_value.to_le_bytes());
    data[16..24].copy_from_slice(&gauge_value.to_bits().to_le_bytes());
    MetricsEdgeForward {
        content_hash: fnv1a(&data),
        name_hash,
        counter_value,
        gauge_value,
        // 8 bytes name_hash + 8 bytes counter + 8 bytes gauge = 24 bytes.
        wire_bytes: 24,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_to_db_record_content_hash_nonzero() {
        let rec = metrics_to_db_record(
            "http_requests_total",
            "method=GET,status=200",
            1_000,
            0.0,
            500,
            1_700_000_000_000,
        );
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.name_hash, 0);
    }

    #[test]
    fn test_metrics_to_db_record_field_values() {
        let rec = metrics_to_db_record("cpu_usage", "host=a", 42, 7.5, 100, 9_999);
        assert_eq!(rec.counter_value, 42);
        assert!((rec.gauge_value - 7.5).abs() < 1e-9);
        assert_eq!(rec.histogram_count, 100);
        assert_eq!(rec.snapshot_at_ms, 9_999);
    }

    #[test]
    fn test_metrics_to_db_record_hash_determinism() {
        let a = metrics_to_db_record("latency_ms", "svc=api", 0, 1.0, 10, 1_000);
        let b = metrics_to_db_record("latency_ms", "svc=api", 0, 1.0, 10, 1_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_metrics_to_cache_entry_stable_ttl() {
        let entry = metrics_to_cache_entry("mem_usage", "host=b", 0, 512.0, true);
        assert_ne!(entry.content_hash, 0);
        // stable=true → ttl = 10 + 50 = 60
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_metrics_to_cache_entry_volatile_ttl() {
        let entry = metrics_to_cache_entry("error_rate", "svc=edge", 7, 0.03, false);
        // stable=false → ttl = 10 + 0 = 10
        assert_eq!(entry.ttl_secs, 10);
        assert_eq!(entry.counter_value, 7);
    }

    #[test]
    fn test_metrics_to_analytics_payload_fields() {
        let p = metrics_to_analytics_payload(
            "request_duration_seconds",
            "route=/api",
            200,
            0.5,
            1_000,
            450.0,
            0.25,
            0.98,
        );
        assert_ne!(p.content_hash, 0);
        assert_eq!(p.counter_delta, 200);
        assert_eq!(p.observation_count, 1_000);
        assert!((p.observation_sum - 450.0).abs() < 1e-9);
        assert!((p.p50 - 0.25).abs() < 1e-9);
        assert!((p.p99 - 0.98).abs() < 1e-9);
    }

    #[test]
    fn test_metrics_to_cdn_package_content_type() {
        let export = "# HELP requests total\nrequests_total 42\n";
        let pkg = metrics_to_cdn_package(export, 1, 30);
        assert_ne!(pkg.content_hash, 0);
        assert_eq!(pkg.content_type, "text/plain; version=0.0.4; charset=utf-8");
        assert_eq!(pkg.family_count, 1);
        assert_eq!(pkg.payload_bytes, export.len());
        assert_eq!(pkg.max_age_secs, 30);
    }

    #[test]
    fn test_metrics_to_edge_forward_wire_bytes() {
        let fwd = metrics_to_edge_forward("disk_io_bytes", 8_192, 1.0);
        assert_ne!(fwd.content_hash, 0);
        assert_ne!(fwd.name_hash, 0);
        assert_eq!(fwd.counter_value, 8_192);
        assert_eq!(fwd.wire_bytes, 24);
    }

    #[test]
    fn test_metrics_to_edge_forward_hash_determinism() {
        let a = metrics_to_edge_forward("net_rx_bytes", 1_024, 0.0);
        let b = metrics_to_edge_forward("net_rx_bytes", 1_024, 0.0);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.name_hash, b.name_hash);
    }
}
