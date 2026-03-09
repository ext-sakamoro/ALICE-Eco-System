//! Logistics bridges — ALICE-Logistics ↔ DB, Cache, Analytics, API, Notify
//!
//! 5 bridges connecting logistics routing and supply-chain data (extracted as
//! primitives) to the ALICE ecosystem. No external crate types are imported;
//! all fields use primitive types derived from serialised logistics state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Logistics → DB (route snapshot persistence) ─────────────────

/// Route snapshot record for ALICE-DB persistence.
pub struct LogisticsDbRecord {
    /// Content hash over route_id, node_count, and edge_count.
    pub content_hash: u64,
    /// Opaque route identifier hash.
    pub route_id_hash: u64,
    /// Number of nodes (depots, waypoints, delivery stops) in the route graph.
    pub node_count: u32,
    /// Number of directed edges in the route graph.
    pub edge_count: u32,
    /// Total route cost (distance × weight factor, arbitrary units).
    pub route_cost: f64,
    /// Number of vehicles assigned to this route plan.
    pub vehicle_count: u32,
    /// Aggregated demand across all stops (e.g. kg or units).
    pub demand_total: f64,
    /// Unix timestamp (seconds) when this snapshot was recorded.
    pub snapshot_ts: u64,
}

/// Build a DB persistence record from extracted logistics route data.
#[inline]
#[must_use]
pub fn logistics_to_db_record(
    route_id: &[u8],
    node_count: u32,
    edge_count: u32,
    route_cost: f64,
    vehicle_count: u32,
    demand_total: f64,
    snapshot_ts: u64,
) -> LogisticsDbRecord {
    let route_id_hash = fnv1a(route_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&route_id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&node_count.to_le_bytes());
    buf[12..16].copy_from_slice(&edge_count.to_le_bytes());
    LogisticsDbRecord {
        content_hash: fnv1a(&buf),
        route_id_hash,
        node_count,
        edge_count,
        route_cost,
        vehicle_count,
        demand_total,
        snapshot_ts,
    }
}

// ── Bridge 2: Logistics → Cache (route plan caching) ──────────────────────

/// Cached route plan entry for ALICE-Cache.
pub struct LogisticsCacheEntry {
    /// Content hash over route_id_hash and vehicle_count.
    pub content_hash: u64,
    /// Hashed route identifier used as cache key.
    pub route_id_hash: u64,
    /// Number of vehicles in the cached plan.
    pub vehicle_count: u32,
    /// Total route cost of the cached plan.
    pub route_cost: f64,
    /// TTL in seconds (branchless: shorter for high-demand routes).
    pub ttl_secs: u32,
    /// Unix timestamp when this entry was cached.
    pub cached_at: u64,
}

/// Build a cache entry for a logistics route plan.
///
/// TTL is 1800 s by default; reduced to 300 s when `demand_total` exceeds
/// `demand_threshold` (high-demand routes change frequently).
#[inline]
#[must_use]
pub fn logistics_to_cache_entry(
    route_id: &[u8],
    vehicle_count: u32,
    route_cost: f64,
    demand_total: f64,
    demand_threshold: f64,
    cached_at: u64,
) -> LogisticsCacheEntry {
    let route_id_hash = fnv1a(route_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&route_id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&vehicle_count.to_le_bytes());
    // Branchless TTL: 1800 - high_demand * 1500
    let high_demand = (demand_total > demand_threshold) as u32;
    let ttl_secs = 1800 - high_demand * 1500;
    LogisticsCacheEntry {
        content_hash: fnv1a(&buf),
        route_id_hash,
        vehicle_count,
        route_cost,
        ttl_secs,
        cached_at,
    }
}

// ── Bridge 3: Logistics → Analytics (operational metrics ingestion) ────────

/// Logistics operational metrics event for ALICE-Analytics ingestion.
pub struct LogisticsAnalyticsEvent {
    /// Content hash over route_id_hash and snapshot_ts.
    pub content_hash: u64,
    /// Hashed route identifier.
    pub route_id_hash: u64,
    /// Number of nodes in the route graph.
    pub node_count: u32,
    /// Number of edges in the route graph.
    pub edge_count: u32,
    /// Total route cost.
    pub route_cost: f64,
    /// Number of vehicles.
    pub vehicle_count: u32,
    /// Aggregated demand.
    pub demand_total: f64,
    /// Mean cost per node (route_cost / node_count).
    pub mean_cost_per_node: f64,
    /// Solve duration in microseconds.
    pub solve_duration_us: u64,
}

/// Build an analytics event from logistics route solution data.
#[inline]
#[must_use]
pub fn logistics_to_analytics_event(
    route_id: &[u8],
    node_count: u32,
    edge_count: u32,
    route_cost: f64,
    vehicle_count: u32,
    demand_total: f64,
    solve_duration_us: u64,
) -> LogisticsAnalyticsEvent {
    let route_id_hash = fnv1a(route_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&route_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&solve_duration_us.to_le_bytes());
    let mean_cost_per_node = route_cost / node_count.max(1) as f64;
    LogisticsAnalyticsEvent {
        content_hash: fnv1a(&buf),
        route_id_hash,
        node_count,
        edge_count,
        route_cost,
        vehicle_count,
        demand_total,
        mean_cost_per_node,
        solve_duration_us,
    }
}

// ── Bridge 4: Logistics → API (route response payload) ────────────────────

/// Logistics route API response payload for ALICE-API serialisation.
pub struct LogisticsApiPayload {
    /// Content hash over route_id_hash and vehicle_count.
    pub content_hash: u64,
    /// Hashed route identifier.
    pub route_id_hash: u64,
    /// Number of vehicles in the returned plan.
    pub vehicle_count: u32,
    /// Total route cost.
    pub route_cost: f64,
    /// Aggregated demand covered.
    pub demand_total: f64,
    /// Whether the response was served from cache.
    pub from_cache: bool,
    /// Response latency in microseconds.
    pub latency_us: u64,
}

/// Build an API response payload for a logistics route request.
#[inline]
#[must_use]
pub fn logistics_to_api_payload(
    route_id: &[u8],
    vehicle_count: u32,
    route_cost: f64,
    demand_total: f64,
    from_cache: bool,
    latency_us: u64,
) -> LogisticsApiPayload {
    let route_id_hash = fnv1a(route_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&route_id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&vehicle_count.to_le_bytes());
    LogisticsApiPayload {
        content_hash: fnv1a(&buf),
        route_id_hash,
        vehicle_count,
        route_cost,
        demand_total,
        from_cache,
        latency_us,
    }
}

// ── Bridge 5: Logistics → Notify (alert on SLA breach) ───────────────────

/// Logistics SLA breach notification for ALICE-Notify.
pub struct LogisticsNotifyAlert {
    /// Content hash over route_id_hash and alert_code.
    pub content_hash: u64,
    /// Hashed route identifier.
    pub route_id_hash: u64,
    /// Alert code: 1 = cost overrun, 2 = capacity exceeded, 3 = deadline missed.
    pub alert_code: u8,
    /// Observed value that triggered the alert (cost, demand, or delay seconds).
    pub observed_value: f64,
    /// Threshold value that was breached.
    pub threshold_value: f64,
    /// Number of vehicles affected.
    pub vehicle_count: u32,
    /// Unix timestamp when the alert was raised.
    pub raised_at: u64,
}

/// Build a notification alert for a logistics SLA breach.
#[inline]
#[must_use]
pub fn logistics_to_notify_alert(
    route_id: &[u8],
    alert_code: u8,
    observed_value: f64,
    threshold_value: f64,
    vehicle_count: u32,
    raised_at: u64,
) -> LogisticsNotifyAlert {
    let route_id_hash = fnv1a(route_id);
    let mut buf = [0u8; 9];
    buf[0..8].copy_from_slice(&route_id_hash.to_le_bytes());
    buf[8] = alert_code;
    LogisticsNotifyAlert {
        content_hash: fnv1a(&buf),
        route_id_hash,
        alert_code,
        observed_value,
        threshold_value,
        vehicle_count,
        raised_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = logistics_to_db_record(b"route-1", 10, 15, 123.4, 3, 500.0, 1_700_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn db_record_fields_preserved() {
        let rec = logistics_to_db_record(b"route-A", 8, 12, 99.9, 2, 300.5, 42_000);
        assert_eq!(rec.node_count, 8);
        assert_eq!(rec.edge_count, 12);
        assert_eq!(rec.vehicle_count, 2);
        assert!((rec.demand_total - 300.5).abs() < f64::EPSILON);
        assert_eq!(rec.snapshot_ts, 42_000);
    }

    #[test]
    fn db_record_hash_deterministic() {
        let a = logistics_to_db_record(b"rt", 5, 7, 10.0, 1, 50.0, 0);
        let b = logistics_to_db_record(b"rt", 5, 7, 10.0, 1, 50.0, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_low_demand_long_ttl() {
        let entry = logistics_to_cache_entry(b"route-1", 2, 50.0, 100.0, 200.0, 0);
        assert_eq!(entry.ttl_secs, 1800);
    }

    #[test]
    fn cache_entry_high_demand_short_ttl() {
        let entry = logistics_to_cache_entry(b"route-2", 4, 80.0, 500.0, 200.0, 0);
        assert_eq!(entry.ttl_secs, 300);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_mean_cost_per_node() {
        // 100.0 cost / 4 nodes = 25.0
        let ev = logistics_to_analytics_event(b"r", 4, 6, 100.0, 2, 200.0, 5_000);
        assert!((ev.mean_cost_per_node - 25.0).abs() < 1e-9);
        assert_ne!(ev.content_hash, 0);
    }

    // ── API payload tests ─────────────────────────────────────────────────

    #[test]
    fn api_payload_from_cache_flag() {
        let p = logistics_to_api_payload(b"r2", 3, 77.7, 400.0, true, 120);
        assert!(p.from_cache);
        assert_eq!(p.vehicle_count, 3);
        assert_ne!(p.content_hash, 0);
    }

    // ── Notify alert tests ────────────────────────────────────────────────

    #[test]
    fn notify_alert_fields_and_hash() {
        let alert = logistics_to_notify_alert(b"route-X", 1, 150.0, 100.0, 5, 1_700_100_000);
        assert_eq!(alert.alert_code, 1);
        assert!((alert.observed_value - 150.0).abs() < f64::EPSILON);
        assert!((alert.threshold_value - 100.0).abs() < f64::EPSILON);
        assert_ne!(alert.content_hash, 0);
        assert_ne!(alert.route_id_hash, 0);
    }
}
