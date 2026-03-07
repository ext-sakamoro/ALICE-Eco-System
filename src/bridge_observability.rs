//! Observability bridges — ALICE-Observability ↔ DB, Analytics, Cache, CDN, Edge
//!
//! 5 bridges connecting distributed tracing, alerting, and SLI data to
//! the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Observability → DB (span storage) ───────────────────────────

/// Span storage record for ALICE-DB.
///
/// Flattens a distributed trace span into a row suitable for time-series
/// queries and waterfall rendering by the tracing UI.
pub struct ObservabilityDbSpanRecord {
    /// FNV-1a hash over trace_id, span_id, and name.
    pub content_hash: u64,
    /// Trace identifier hash (64-bit projection of 128-bit trace ID).
    pub trace_id_hash: u64,
    /// Span identifier hash.
    pub span_id_hash: u64,
    /// Span start timestamp in milliseconds.
    pub start_ms: u64,
    /// Span duration in milliseconds (end_ms - start_ms; saturating at 0).
    pub duration_ms: u64,
    /// True when the span status is Error.
    pub is_error: bool,
    /// Number of attributes attached to the span.
    pub attribute_count: u32,
}

/// Serialize a span for ALICE-DB storage.
#[inline]
#[must_use]
pub fn observability_to_db_span_record(
    trace_id: &str,
    span_id: &str,
    name: &str,
    start_ms: u64,
    end_ms: u64,
    is_error: bool,
    attribute_count: u32,
) -> ObservabilityDbSpanRecord {
    let trace_id_hash = fnv1a(trace_id.as_bytes());
    let span_id_hash = fnv1a(span_id.as_bytes());
    let mut data = [0u8; 24];
    data[0..8].copy_from_slice(&trace_id_hash.to_le_bytes());
    data[8..16].copy_from_slice(&span_id_hash.to_le_bytes());
    data[16..24].copy_from_slice(&fnv1a(name.as_bytes()).to_le_bytes());
    let duration_ms = end_ms.saturating_sub(start_ms);
    ObservabilityDbSpanRecord {
        content_hash: fnv1a(&data),
        trace_id_hash,
        span_id_hash,
        start_ms,
        duration_ms,
        is_error,
        attribute_count,
    }
}

// ── Bridge 2: Observability → Analytics (SLI metrics) ────────────────────

/// SLI metrics payload for ALICE-Analytics.
///
/// Feeds SLI good/total counts and error budget data into the analytics
/// pipeline so SLO burn-rate alerts and trend analysis can be computed.
pub struct ObservabilityAnalyticsSliPayload {
    /// FNV-1a hash over SLI name and window.
    pub content_hash: u64,
    /// SLI name hash for analytics stream routing.
    pub sli_name_hash: u64,
    /// Number of good events in the window.
    pub good_events: u64,
    /// Total events in the window.
    pub total_events: u64,
    /// SLI ratio (good / total); 1.0 when total is zero.
    pub sli_ratio: f64,
    /// Error budget remaining as a fraction of the target (may be negative).
    pub error_budget_remaining: f64,
    /// Measurement window in milliseconds.
    pub window_ms: u64,
}

/// Build an SLI metrics payload for ALICE-Analytics.
///
/// `sli_target` is the desired availability fraction, e.g. 0.999.
/// Division is guarded with a reciprocal to avoid a bare `/` in the hot path.
#[inline]
#[must_use]
pub fn observability_to_analytics_sli(
    sli_name: &str,
    good_events: u64,
    total_events: u64,
    sli_target: f64,
    window_ms: u64,
) -> ObservabilityAnalyticsSliPayload {
    let sli_name_hash = fnv1a(sli_name.as_bytes());
    let rcp_total = 1.0 / total_events.max(1) as f64;
    let sli_ratio = good_events as f64 * rcp_total;
    // error_budget_remaining = (sli_ratio - target) / (1.0 - target)
    let error_fraction = 1.0 - sli_target;
    let rcp_error = 1.0 / error_fraction.max(f64::EPSILON);
    let error_budget_remaining = (sli_ratio - sli_target) * rcp_error;
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&sli_name_hash.to_le_bytes());
    data[8..16].copy_from_slice(&window_ms.to_le_bytes());
    ObservabilityAnalyticsSliPayload {
        content_hash: fnv1a(&data),
        sli_name_hash,
        good_events,
        total_events,
        sli_ratio,
        error_budget_remaining,
        window_ms,
    }
}

// ── Bridge 3: Observability → Cache (span cache) ──────────────────────────

/// Span cache entry for ALICE-Cache.
///
/// Hot spans (recently active traces) are cached so that the trace UI can
/// render waterfall views without querying the DB on every page load.
/// TTL is computed branchlessly based on whether the span has finished.
pub struct ObservabilityCacheSpanEntry {
    /// FNV-1a hash over trace_id and span_id — cache key.
    pub content_hash: u64,
    /// Span duration in milliseconds (0 when still in-flight).
    pub duration_ms: u64,
    /// True when the span has completed (end_ms > start_ms).
    pub is_complete: bool,
    /// Cache TTL in seconds (branchless: longer for completed spans).
    pub ttl_secs: u32,
    /// True when the span carries an error status.
    pub is_error: bool,
}

/// Build a span cache entry for ALICE-Cache.
///
/// Completed spans get a 300 s TTL; in-flight spans get 30 s.
/// The TTL selection is branchless via integer arithmetic.
#[inline]
#[must_use]
pub fn observability_to_cache_span(
    trace_id: &str,
    span_id: &str,
    start_ms: u64,
    end_ms: u64,
    is_error: bool,
) -> ObservabilityCacheSpanEntry {
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&fnv1a(trace_id.as_bytes()).to_le_bytes());
    data[8..16].copy_from_slice(&fnv1a(span_id.as_bytes()).to_le_bytes());
    let is_complete = end_ms > start_ms;
    let duration_ms = end_ms.saturating_sub(start_ms);
    // Branchless TTL: complete → 300 s, in-flight → 30 s.
    let complete = is_complete as u32;
    let ttl_secs = 30u32 + complete * 270u32;
    ObservabilityCacheSpanEntry {
        content_hash: fnv1a(&data),
        duration_ms,
        is_complete,
        ttl_secs,
        is_error,
    }
}

// ── Bridge 4: Observability → CDN (dashboard delivery) ───────────────────

/// Observability dashboard package for ALICE-CDN delivery.
///
/// Bundles a rendered SLI/alert summary for CDN edge caching so that
/// global status pages load from the nearest PoP.
pub struct ObservabilityCdnDashboard {
    /// FNV-1a hash over the rendered payload.
    pub content_hash: u64,
    /// Number of active alert rules included.
    pub alert_count: u32,
    /// Number of SLI configs included.
    pub sli_count: u32,
    /// Number of currently firing alerts.
    pub firing_count: u32,
    /// Estimated payload size in bytes.
    pub payload_bytes: usize,
    /// MIME type for CDN content negotiation.
    pub content_type: &'static str,
    /// Cache-Control max-age in seconds.
    pub max_age_secs: u32,
}

/// Build an observability dashboard package for ALICE-CDN delivery.
#[inline]
#[must_use]
pub fn observability_to_cdn_dashboard(
    payload: &str,
    alert_count: u32,
    sli_count: u32,
    firing_count: u32,
    max_age_secs: u32,
) -> ObservabilityCdnDashboard {
    let payload_hash = fnv1a(payload.as_bytes());
    let mut data = [0u8; 20];
    data[0..8].copy_from_slice(&payload_hash.to_le_bytes());
    data[8..12].copy_from_slice(&alert_count.to_le_bytes());
    data[12..16].copy_from_slice(&sli_count.to_le_bytes());
    data[16..20].copy_from_slice(&firing_count.to_le_bytes());
    ObservabilityCdnDashboard {
        content_hash: fnv1a(&data),
        alert_count,
        sli_count,
        firing_count,
        payload_bytes: payload.len(),
        content_type: "application/x-alice-observability",
        max_age_secs,
    }
}

// ── Bridge 5: Observability → Edge (trace forwarding) ────────────────────

/// Compact trace forward payload for ALICE-Edge.
///
/// Edge nodes emit lightweight span summaries so that the central tracing
/// backend can correlate edge latency without receiving full attribute sets.
pub struct ObservabilityEdgeTraceForward {
    /// FNV-1a hash over trace_id and span summary.
    pub content_hash: u64,
    /// Trace identifier hash.
    pub trace_id_hash: u64,
    /// Span duration in milliseconds.
    pub duration_ms: u64,
    /// True when the forwarded span has an error status.
    pub is_error: bool,
    /// Estimated wire size in bytes.
    pub wire_bytes: usize,
}

/// Build a compact trace forward payload for ALICE-Edge.
#[inline]
#[must_use]
pub fn observability_to_edge_trace(
    trace_id: &str,
    span_id: &str,
    duration_ms: u64,
    is_error: bool,
) -> ObservabilityEdgeTraceForward {
    let trace_id_hash = fnv1a(trace_id.as_bytes());
    let span_id_hash = fnv1a(span_id.as_bytes());
    let mut data = [0u8; 24];
    data[0..8].copy_from_slice(&trace_id_hash.to_le_bytes());
    data[8..16].copy_from_slice(&span_id_hash.to_le_bytes());
    data[16..24].copy_from_slice(&duration_ms.to_le_bytes());
    ObservabilityEdgeTraceForward {
        content_hash: fnv1a(&data),
        trace_id_hash,
        duration_ms,
        is_error,
        // 8 trace_id + 8 span_id + 8 duration = 24 bytes.
        wire_bytes: 24,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observability_to_db_span_record_content_hash_nonzero() {
        let rec = observability_to_db_span_record(
            "trace-abc-123",
            "span-xyz-456",
            "http.request",
            1_000,
            1_050,
            false,
            3,
        );
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.trace_id_hash, 0);
        assert_ne!(rec.span_id_hash, 0);
    }

    #[test]
    fn test_observability_to_db_span_record_duration() {
        let rec = observability_to_db_span_record("t1", "s1", "db.query", 1_000, 1_200, false, 0);
        assert_eq!(rec.duration_ms, 200);
        assert!(!rec.is_error);
    }

    #[test]
    fn test_observability_to_db_span_record_end_before_start_saturates() {
        // end_ms < start_ms → duration saturates to 0.
        let rec = observability_to_db_span_record("t2", "s2", "rpc", 2_000, 1_000, true, 1);
        assert_eq!(rec.duration_ms, 0);
        assert!(rec.is_error);
    }

    #[test]
    fn test_observability_to_analytics_sli_ratio() {
        // 990 good out of 1000 total → ratio 0.99.
        let p = observability_to_analytics_sli("api-availability", 990, 1_000, 0.999, 86_400_000);
        assert_ne!(p.content_hash, 0);
        assert!(
            (p.sli_ratio - 0.99).abs() < 1e-9,
            "sli_ratio={}",
            p.sli_ratio
        );
        assert_eq!(p.good_events, 990);
        assert_eq!(p.total_events, 1_000);
    }

    #[test]
    fn test_observability_to_analytics_sli_zero_total_no_panic() {
        // Zero total must not panic — denominator saturated to 1.
        let p = observability_to_analytics_sli("empty-sli", 0, 0, 0.999, 3_600_000);
        assert_eq!(p.total_events, 0);
        assert!((p.sli_ratio - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_observability_to_cache_span_complete_ttl() {
        let entry = observability_to_cache_span("trace-1", "span-1", 1_000, 2_000, false);
        assert_ne!(entry.content_hash, 0);
        assert!(entry.is_complete);
        // complete → ttl = 30 + 270 = 300
        assert_eq!(entry.ttl_secs, 300);
        assert_eq!(entry.duration_ms, 1_000);
    }

    #[test]
    fn test_observability_to_cache_span_inflight_ttl() {
        // end_ms == start_ms → not complete.
        let entry = observability_to_cache_span("trace-2", "span-2", 5_000, 5_000, false);
        assert!(!entry.is_complete);
        // in-flight → ttl = 30
        assert_eq!(entry.ttl_secs, 30);
        assert_eq!(entry.duration_ms, 0);
    }

    #[test]
    fn test_observability_to_cdn_dashboard_fields() {
        let payload = r#"{"slis":1,"alerts":2,"firing":0}"#;
        let dash = observability_to_cdn_dashboard(payload, 2, 1, 0, 60);
        assert_ne!(dash.content_hash, 0);
        assert_eq!(dash.alert_count, 2);
        assert_eq!(dash.sli_count, 1);
        assert_eq!(dash.firing_count, 0);
        assert_eq!(dash.payload_bytes, payload.len());
        assert_eq!(dash.content_type, "application/x-alice-observability");
        assert_eq!(dash.max_age_secs, 60);
    }

    #[test]
    fn test_observability_to_edge_trace_wire_bytes() {
        let fwd = observability_to_edge_trace("trace-edge-1", "span-edge-1", 42, false);
        assert_ne!(fwd.content_hash, 0);
        assert_ne!(fwd.trace_id_hash, 0);
        assert_eq!(fwd.duration_ms, 42);
        assert!(!fwd.is_error);
        assert_eq!(fwd.wire_bytes, 24);
    }
}
