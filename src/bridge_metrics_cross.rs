//! Cross-domain bridges — ALICE-Metrics ↔ Observability, Analytics
//!
//! 5 bridges connecting metrics counters/gauges/histograms to
//! observability spans/logs and analytics distribution events.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Metrics counter → Observability span ─────────────────────

/// Observability span data derived from a Metrics counter.
///
/// Maps a counter's name, label set, and current value into an
/// Observability span so request-rate metrics can appear as trace
/// annotations without direct coupling between the two crates.
pub struct MetricsCounterSpan {
    /// FNV-1a hash over counter name + label key + value bytes.
    pub content_hash: u64,
    /// Hash of the counter name — used as trace_id seed.
    pub trace_id: u64,
    /// Hash of the label key — used as span_id seed.
    pub span_id: u64,
    /// Counter name copied for span operation field.
    pub operation: &'static str,
    /// Counter value at conversion time.
    pub counter_value: u64,
    /// Span status: 0=Ok, 1=Error (counter above threshold → Error).
    pub status: u8,
}

/// Convert a Metrics counter snapshot into an Observability span descriptor.
///
/// `error_threshold`: if the counter value exceeds this, the span
/// is marked as Error (status=1), otherwise Ok (status=0).
#[inline]
#[must_use]
pub fn metrics_counter_to_observability_span(
    counter_name: &'static str,
    labels: &str,
    counter_value: u64,
    error_threshold: u64,
) -> MetricsCounterSpan {
    let name_hash = fnv1a(counter_name.as_bytes());
    let label_hash = fnv1a(labels.as_bytes());
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&name_hash.to_le_bytes());
    key[8..16].copy_from_slice(&label_hash.to_le_bytes());
    key[16..24].copy_from_slice(&counter_value.to_le_bytes());

    let exceeded = (counter_value > error_threshold) as u8;
    let status = match exceeded {
        0 => 0u8, // Ok
        _ => 1u8, // Error
    };

    MetricsCounterSpan {
        content_hash: fnv1a(&key),
        trace_id: name_hash,
        span_id: label_hash,
        operation: counter_name,
        counter_value,
        status,
    }
}

// ── Bridge 2: Metrics histogram → Analytics distribution event ─────────

/// Analytics distribution event derived from a Metrics histogram.
///
/// Feeds histogram summary statistics (count, sum, mean) into the
/// Analytics DDSketch/HLL pipeline for quantile estimation and
/// cardinality tracking without direct Analytics dependency in Metrics.
pub struct MetricsHistogramAnalytics {
    /// FNV-1a hash over histogram name + total_count + sum bytes.
    pub content_hash: u64,
    /// Hash of the histogram name for pipeline routing.
    pub name_hash: u64,
    /// Total number of observations in the histogram.
    pub observation_count: u64,
    /// Sum of all observed values.
    pub observation_sum: f64,
    /// Estimated mean (sum / count).
    pub mean: f64,
    /// Estimated P50 percentile.
    pub p50: f64,
    /// Estimated P99 percentile.
    pub p99: f64,
}

/// Convert a Metrics histogram into an Analytics distribution event.
#[inline]
#[must_use]
pub fn metrics_histogram_to_analytics(
    name: &str,
    total_count: u64,
    sum: f64,
    p50: f64,
    p99: f64,
) -> MetricsHistogramAnalytics {
    let name_hash = fnv1a(name.as_bytes());
    let mean = if total_count == 0 { 0.0 } else { sum / total_count as f64 };
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&name_hash.to_le_bytes());
    key[8..16].copy_from_slice(&total_count.to_le_bytes());
    key[16..24].copy_from_slice(&sum.to_bits().to_le_bytes());

    MetricsHistogramAnalytics {
        content_hash: fnv1a(&key),
        name_hash,
        observation_count: total_count,
        observation_sum: sum,
        mean,
        p50,
        p99,
    }
}

// ── Bridge 3: Metrics gauge → Observability log entry ──────────────────

/// Observability log entry derived from a Metrics gauge.
///
/// Maps a gauge's current value into an Observability log so that
/// gauge changes (e.g. memory pressure, connection count) appear as
/// structured log events in the tracing pipeline.
pub struct MetricsGaugeLog {
    /// FNV-1a hash over gauge name + value bytes.
    pub content_hash: u64,
    /// Hash of the gauge name for log routing.
    pub name_hash: u64,
    /// Gauge value at conversion time.
    pub gauge_value: f64,
    /// Log severity: 0=Info, 1=Warning, 2=Critical.
    pub severity: u8,
    /// Gauge name for structured log message.
    pub gauge_name: &'static str,
}

/// Convert a Metrics gauge value into an Observability log entry.
///
/// `warning_threshold` / `critical_threshold`: gauge value above these
/// levels sets severity to Warning (1) or Critical (2).
#[inline]
#[must_use]
pub fn metrics_gauge_to_observability_log(
    gauge_name: &'static str,
    gauge_value: f64,
    warning_threshold: f64,
    critical_threshold: f64,
) -> MetricsGaugeLog {
    let name_hash = fnv1a(gauge_name.as_bytes());
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&name_hash.to_le_bytes());
    key[8..16].copy_from_slice(&gauge_value.to_bits().to_le_bytes());

    // 明示的matchでseverityを算出（branchless pattern）
    let above_critical = (gauge_value >= critical_threshold) as u8;
    let above_warning = (gauge_value >= warning_threshold) as u8;
    // critical=2, warning=1, info=0
    let severity = above_warning + above_critical;

    MetricsGaugeLog {
        content_hash: fnv1a(&key),
        name_hash,
        gauge_value,
        severity,
        gauge_name,
    }
}

// ── Bridge 4: Observability span → Metrics counter increment ───────────

/// Metrics counter increment derived from an Observability span.
///
/// Maps a completed span back into a counter increment so the Metrics
/// layer can track request counts per span-operation and status.
pub struct ObservabilitySpanCounter {
    /// FNV-1a hash over operation + status + duration bytes.
    pub content_hash: u64,
    /// Hash of the span operation — used as counter name.
    pub operation_hash: u64,
    /// Span status mapped to u8: 0=Ok, 1=Error, 2=Timeout.
    pub status: u8,
    /// Span duration in microseconds.
    pub duration_us: u64,
    /// Counter increment value (always 1 per span).
    pub increment: u64,
}

/// Convert an Observability span into a Metrics counter increment.
#[inline]
#[must_use]
pub fn observability_span_to_metrics_counter(
    operation: &str,
    status: u8,
    duration_us: u64,
) -> ObservabilitySpanCounter {
    let operation_hash = fnv1a(operation.as_bytes());
    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&operation_hash.to_le_bytes());
    key[8] = status;
    key[9..17].copy_from_slice(&duration_us.to_le_bytes());

    ObservabilitySpanCounter {
        content_hash: fnv1a(&key),
        operation_hash,
        status,
        duration_us,
        increment: 1,
    }
}

// ── Bridge 5: Metrics snapshot → Cache ─────────────────────────────────

/// Cache entry for a cross-domain metrics snapshot.
///
/// Caches an aggregated metrics snapshot (counter + gauge + histogram
/// mean) with branchless TTL based on whether the snapshot is considered
/// stable (low variance) or volatile (high variance).
pub struct MetricsSnapshotCache {
    /// FNV-1a hash over counter + gauge + histogram_mean bytes.
    pub content_hash: u64,
    /// Counter value at snapshot time.
    pub counter_value: u64,
    /// Gauge value at snapshot time.
    pub gauge_value: f64,
    /// Histogram mean at snapshot time.
    pub histogram_mean: f64,
    /// Cache TTL in seconds (branchless: stable=120s, volatile=15s).
    pub ttl_secs: u32,
    /// Estimated cache entry size in bytes.
    pub entry_bytes: usize,
}

/// Build a cache entry for a cross-domain metrics snapshot.
///
/// `stable`: when true, TTL is 120s; when false, TTL is 15s.
/// Computed branchlessly: `base - condition * delta`.
#[inline]
#[must_use]
pub fn metrics_snapshot_to_cache(
    counter_value: u64,
    gauge_value: f64,
    histogram_mean: f64,
    stable: bool,
) -> MetricsSnapshotCache {
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&counter_value.to_le_bytes());
    key[8..16].copy_from_slice(&gauge_value.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&histogram_mean.to_bits().to_le_bytes());

    // Branchless TTL: base=120, delta=105 → stable(1)=120, volatile(0)=15
    let stable_u32 = stable as u32;
    let ttl_secs = 120u32 - (1 - stable_u32) * 105u32;

    MetricsSnapshotCache {
        content_hash: fnv1a(&key),
        counter_value,
        gauge_value,
        histogram_mean,
        ttl_secs,
        entry_bytes: 40,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Bridge 1: counter → observability span

    #[test]
    fn test_counter_to_span_ok_status() {
        let span = metrics_counter_to_observability_span(
            "http_requests_total", "method=GET", 50, 100,
        );
        assert_ne!(span.content_hash, 0);
        assert_eq!(span.status, 0); // below threshold → Ok
        assert_eq!(span.counter_value, 50);
        assert_eq!(span.operation, "http_requests_total");
    }

    #[test]
    fn test_counter_to_span_error_status() {
        let span = metrics_counter_to_observability_span(
            "error_count", "svc=api", 200, 100,
        );
        assert_eq!(span.status, 1); // above threshold → Error
        assert_eq!(span.counter_value, 200);
    }

    #[test]
    fn test_counter_to_span_deterministic() {
        let a = metrics_counter_to_observability_span("rpc_total", "ep=foo", 10, 50);
        let b = metrics_counter_to_observability_span("rpc_total", "ep=foo", 10, 50);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.trace_id, b.trace_id);
    }

    // Bridge 2: histogram → analytics

    #[test]
    fn test_histogram_to_analytics_basic() {
        let ev = metrics_histogram_to_analytics("latency_ms", 100, 5000.0, 25.0, 95.0);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.observation_count, 100);
        assert!((ev.mean - 50.0).abs() < 1e-9);
        assert!((ev.p50 - 25.0).abs() < 1e-9);
        assert!((ev.p99 - 95.0).abs() < 1e-9);
    }

    #[test]
    fn test_histogram_to_analytics_zero_count() {
        let ev = metrics_histogram_to_analytics("empty_hist", 0, 0.0, 0.0, 0.0);
        assert!((ev.mean - 0.0).abs() < 1e-9);
        assert_eq!(ev.observation_count, 0);
    }

    // Bridge 3: gauge → observability log

    #[test]
    fn test_gauge_to_log_info() {
        let log = metrics_gauge_to_observability_log("mem_usage_mb", 50.0, 80.0, 95.0);
        assert_ne!(log.content_hash, 0);
        assert_eq!(log.severity, 0); // Info
        assert_eq!(log.gauge_name, "mem_usage_mb");
    }

    #[test]
    fn test_gauge_to_log_warning() {
        let log = metrics_gauge_to_observability_log("cpu_pct", 85.0, 80.0, 95.0);
        assert_eq!(log.severity, 1); // Warning
    }

    #[test]
    fn test_gauge_to_log_critical() {
        let log = metrics_gauge_to_observability_log("disk_pct", 98.0, 80.0, 95.0);
        assert_eq!(log.severity, 2); // Critical
    }

    // Bridge 4: observability span → metrics counter

    #[test]
    fn test_span_to_counter_basic() {
        let ctr = observability_span_to_metrics_counter("db_query", 0, 1500);
        assert_ne!(ctr.content_hash, 0);
        assert_eq!(ctr.status, 0);
        assert_eq!(ctr.duration_us, 1500);
        assert_eq!(ctr.increment, 1);
    }

    #[test]
    fn test_span_to_counter_error_status() {
        let ctr = observability_span_to_metrics_counter("auth_check", 1, 500);
        assert_eq!(ctr.status, 1);
        assert_eq!(ctr.increment, 1);
    }

    // Bridge 5: metrics snapshot → cache

    #[test]
    fn test_snapshot_cache_stable_ttl() {
        let entry = metrics_snapshot_to_cache(100, 3.14, 25.0, true);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 120); // stable → 120s
        assert_eq!(entry.counter_value, 100);
    }

    #[test]
    fn test_snapshot_cache_volatile_ttl() {
        let entry = metrics_snapshot_to_cache(0, 0.0, 0.0, false);
        assert_eq!(entry.ttl_secs, 15); // volatile → 15s
    }

    #[test]
    fn test_snapshot_cache_deterministic() {
        let a = metrics_snapshot_to_cache(42, 1.0, 2.0, true);
        let b = metrics_snapshot_to_cache(42, 1.0, 2.0, true);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
