//! LoadBalancer bridges — ALICE-LoadBalancer ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting load-balancer operational data (extracted as primitives)
//! to the ALICE ecosystem. No external crate types are imported; all fields
//! use primitive types derived from serialised load-balancer state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LoadBalancer → DB (state snapshot persistence) ──────────────

/// Load-balancer state snapshot for ALICE-DB persistence.
pub struct LoadBalancerDbRecord {
    /// Content hash over lb_id, backend_count, and snapshot_ts.
    pub content_hash: u64,
    /// Opaque load-balancer identifier hash.
    pub lb_id_hash: u64,
    /// Number of registered backend instances.
    pub backend_count: u32,
    /// Total active connections across all backends.
    pub active_connections: u64,
    /// Observed requests per second at snapshot time.
    pub requests_per_sec: f64,
    /// Interval between health checks in milliseconds.
    pub health_check_interval_ms: u32,
    /// Hash of the algorithm name (e.g. "round_robin", "least_conn").
    pub algorithm_name_hash: u64,
    /// Number of backends currently marked as healthy.
    pub healthy_backend_count: u32,
    /// Unix timestamp in seconds when this snapshot was taken.
    pub snapshot_ts: u64,
}

/// Build a DB persistence record from load-balancer state.
#[inline]
#[must_use]
pub fn lb_to_db_record(
    lb_id: &[u8],
    backend_count: u32,
    active_connections: u64,
    requests_per_sec: f64,
    health_check_interval_ms: u32,
    algorithm_name: &[u8],
    healthy_backend_count: u32,
    snapshot_ts: u64,
) -> LoadBalancerDbRecord {
    let lb_id_hash = fnv1a(lb_id);
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&lb_id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&backend_count.to_le_bytes());
    buf[12..20].copy_from_slice(&snapshot_ts.to_le_bytes());
    buf[20..24].copy_from_slice(&healthy_backend_count.to_le_bytes());
    LoadBalancerDbRecord {
        content_hash: fnv1a(&buf),
        lb_id_hash,
        backend_count,
        active_connections,
        requests_per_sec,
        health_check_interval_ms,
        algorithm_name_hash: fnv1a(algorithm_name),
        healthy_backend_count,
        snapshot_ts,
    }
}

// ── Bridge 2: LoadBalancer → Cache (routing table caching) ────────────────

/// Cached routing table entry for ALICE-Cache.
pub struct LoadBalancerCacheEntry {
    /// Content hash over lb_id_hash and backend_count.
    pub content_hash: u64,
    /// Hashed load-balancer identifier used as cache key.
    pub lb_id_hash: u64,
    /// Number of backends in the cached routing table.
    pub backend_count: u32,
    /// Number of healthy backends.
    pub healthy_backend_count: u32,
    /// Algorithm name hash.
    pub algorithm_name_hash: u64,
    /// TTL in milliseconds (branchless: shorter when unhealthy ratio is high).
    pub ttl_ms: u32,
    /// Unix timestamp when this entry was cached.
    pub cached_at: u64,
}

/// Build a cache entry for a load-balancer routing table.
///
/// TTL is 5000 ms by default; reduced to 500 ms when the healthy backend
/// ratio falls below 0.5 (degraded cluster needs faster refresh).
#[inline]
#[must_use]
pub fn lb_to_cache_entry(
    lb_id: &[u8],
    backend_count: u32,
    healthy_backend_count: u32,
    algorithm_name: &[u8],
    cached_at: u64,
) -> LoadBalancerCacheEntry {
    let lb_id_hash = fnv1a(lb_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&lb_id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&backend_count.to_le_bytes());
    // Branchless TTL: 5000 - degraded * 4500
    let healthy_ratio_low = (healthy_backend_count * 2 < backend_count.max(1)) as u32;
    let ttl_ms = 5000 - healthy_ratio_low * 4500;
    LoadBalancerCacheEntry {
        content_hash: fnv1a(&buf),
        lb_id_hash,
        backend_count,
        healthy_backend_count,
        algorithm_name_hash: fnv1a(algorithm_name),
        ttl_ms,
        cached_at,
    }
}

// ── Bridge 3: LoadBalancer → Analytics (traffic metrics ingestion) ─────────

/// Load-balancer traffic metrics event for ALICE-Analytics ingestion.
pub struct LoadBalancerAnalyticsEvent {
    /// Content hash over lb_id_hash and snapshot_ts.
    pub content_hash: u64,
    /// Hashed load-balancer identifier.
    pub lb_id_hash: u64,
    /// Number of backend instances.
    pub backend_count: u32,
    /// Total active connections.
    pub active_connections: u64,
    /// Requests per second.
    pub requests_per_sec: f64,
    /// Mean connections per healthy backend.
    pub mean_connections_per_backend: f64,
    /// Ratio of healthy backends (0.0–1.0).
    pub healthy_ratio: f64,
    /// Unix timestamp in seconds.
    pub snapshot_ts: u64,
}

/// Build an analytics event from load-balancer traffic statistics.
#[inline]
#[must_use]
pub fn lb_to_analytics_event(
    lb_id: &[u8],
    backend_count: u32,
    healthy_backend_count: u32,
    active_connections: u64,
    requests_per_sec: f64,
    snapshot_ts: u64,
) -> LoadBalancerAnalyticsEvent {
    let lb_id_hash = fnv1a(lb_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&lb_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&snapshot_ts.to_le_bytes());
    let mean_connections_per_backend =
        active_connections as f64 / healthy_backend_count.max(1) as f64;
    let healthy_ratio = healthy_backend_count as f64 / backend_count.max(1) as f64;
    LoadBalancerAnalyticsEvent {
        content_hash: fnv1a(&buf),
        lb_id_hash,
        backend_count,
        active_connections,
        requests_per_sec,
        mean_connections_per_backend,
        healthy_ratio,
        snapshot_ts,
    }
}

// ── Bridge 4: LoadBalancer → Monitor (health alert) ───────────────────────

/// Load-balancer health alert for ALICE-Monitor.
pub struct LoadBalancerMonitorAlert {
    /// Content hash over lb_id_hash and alert_code.
    pub content_hash: u64,
    /// Hashed load-balancer identifier.
    pub lb_id_hash: u64,
    /// Alert code: 1 = backend down, 2 = overload, 3 = algorithm changed.
    pub alert_code: u8,
    /// Number of healthy backends at alert time.
    pub healthy_backend_count: u32,
    /// Total backend count.
    pub backend_count: u32,
    /// Active connections at alert time.
    pub active_connections: u64,
    /// Requests per second at alert time.
    pub requests_per_sec: f64,
    /// Unix timestamp in seconds when the alert was raised.
    pub raised_at: u64,
}

/// Build a monitor health alert from load-balancer state.
#[inline]
#[must_use]
pub fn lb_to_monitor_alert(
    lb_id: &[u8],
    alert_code: u8,
    healthy_backend_count: u32,
    backend_count: u32,
    active_connections: u64,
    requests_per_sec: f64,
    raised_at: u64,
) -> LoadBalancerMonitorAlert {
    let lb_id_hash = fnv1a(lb_id);
    let mut buf = [0u8; 9];
    buf[0..8].copy_from_slice(&lb_id_hash.to_le_bytes());
    buf[8] = alert_code;
    LoadBalancerMonitorAlert {
        content_hash: fnv1a(&buf),
        lb_id_hash,
        alert_code,
        healthy_backend_count,
        backend_count,
        active_connections,
        requests_per_sec,
        raised_at,
    }
}

// ── Bridge 5: LoadBalancer → API (status response payload) ────────────────

/// Load-balancer status API response payload for ALICE-API serialisation.
pub struct LoadBalancerApiPayload {
    /// Content hash over lb_id_hash and backend_count.
    pub content_hash: u64,
    /// Hashed load-balancer identifier.
    pub lb_id_hash: u64,
    /// Number of registered backends.
    pub backend_count: u32,
    /// Number of healthy backends.
    pub healthy_backend_count: u32,
    /// Active connection count.
    pub active_connections: u64,
    /// Requests per second.
    pub requests_per_sec: f64,
    /// Algorithm name hash.
    pub algorithm_name_hash: u64,
    /// Health check interval in milliseconds.
    pub health_check_interval_ms: u32,
    /// Whether the response was served from cache.
    pub from_cache: bool,
    /// Response latency in microseconds.
    pub latency_us: u64,
}

/// Build an API status response payload for a load-balancer query.
#[inline]
#[must_use]
pub fn lb_to_api_payload(
    lb_id: &[u8],
    backend_count: u32,
    healthy_backend_count: u32,
    active_connections: u64,
    requests_per_sec: f64,
    algorithm_name: &[u8],
    health_check_interval_ms: u32,
    from_cache: bool,
    latency_us: u64,
) -> LoadBalancerApiPayload {
    let lb_id_hash = fnv1a(lb_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&lb_id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&backend_count.to_le_bytes());
    LoadBalancerApiPayload {
        content_hash: fnv1a(&buf),
        lb_id_hash,
        backend_count,
        healthy_backend_count,
        active_connections,
        requests_per_sec,
        algorithm_name_hash: fnv1a(algorithm_name),
        health_check_interval_ms,
        from_cache,
        latency_us,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = lb_to_db_record(
            b"lb-prod-1",
            8,
            1200,
            5000.0,
            5000,
            b"round_robin",
            8,
            1_700_000_000,
        );
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.lb_id_hash, 0);
    }

    #[test]
    fn db_record_algorithm_hash_differs_by_name() {
        let rr = lb_to_db_record(b"lb", 4, 100, 1000.0, 5000, b"round_robin", 4, 0);
        let lc = lb_to_db_record(b"lb", 4, 100, 1000.0, 5000, b"least_conn", 4, 0);
        assert_ne!(rr.algorithm_name_hash, lc.algorithm_name_hash);
    }

    #[test]
    fn db_record_hash_deterministic() {
        let a = lb_to_db_record(b"x", 2, 50, 100.0, 1000, b"rr", 2, 999);
        let b = lb_to_db_record(b"x", 2, 50, 100.0, 1000, b"rr", 2, 999);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_healthy_cluster_long_ttl() {
        // 4 healthy out of 4 total → healthy_ratio_low = false
        let e = lb_to_cache_entry(b"lb1", 4, 4, b"rr", 0);
        assert_eq!(e.ttl_ms, 5000);
    }

    #[test]
    fn cache_entry_degraded_cluster_short_ttl() {
        // 1 healthy out of 4 total → 1*2 < 4 → degraded
        let e = lb_to_cache_entry(b"lb2", 4, 1, b"rr", 0);
        assert_eq!(e.ttl_ms, 500);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_healthy_ratio() {
        // 3 healthy out of 4 → 0.75
        let ev = lb_to_analytics_event(b"lb3", 4, 3, 600, 2000.0, 1_000);
        assert!((ev.healthy_ratio - 0.75).abs() < 1e-9);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn analytics_event_mean_connections_per_backend() {
        // 300 connections / 3 healthy = 100.0
        let ev = lb_to_analytics_event(b"lb4", 4, 3, 300, 500.0, 2_000);
        assert!((ev.mean_connections_per_backend - 100.0).abs() < 1e-9);
    }

    // ── Monitor alert tests ───────────────────────────────────────────────

    #[test]
    fn monitor_alert_fields_and_hash() {
        let alert = lb_to_monitor_alert(b"lb-x", 1, 2, 4, 800, 3000.0, 1_700_200_000);
        assert_eq!(alert.alert_code, 1);
        assert_eq!(alert.healthy_backend_count, 2);
        assert_ne!(alert.content_hash, 0);
    }

    // ── API payload tests ─────────────────────────────────────────────────

    #[test]
    fn api_payload_from_cache_and_hash() {
        let p = lb_to_api_payload(b"lb-api", 6, 5, 900, 1500.0, b"least_conn", 3000, true, 200);
        assert!(p.from_cache);
        assert_eq!(p.backend_count, 6);
        assert_ne!(p.content_hash, 0);
        assert_ne!(p.algorithm_name_hash, 0);
    }
}
