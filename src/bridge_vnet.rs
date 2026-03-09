//! VNet bridges — VNet ↔ DB, Cache, Analytics, Edge, Monitor
//!
//! 5 bridges connecting virtual network topology data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: VNet → DB (network topology persistence) ───────────────────

/// Virtual network topology record for ALICE-DB persistence.
pub struct VnetDbRecord {
    /// Content hash over topology fields.
    pub content_hash: u64,
    /// Number of network nodes in the VNet.
    pub node_count: u32,
    /// Number of network links configured.
    pub link_count: u32,
    /// FNV-1a hash of the subnet identifier.
    pub subnet_hash: u64,
    /// Aggregate bandwidth in megabits per second.
    pub bandwidth_mbps: u32,
    /// Record timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize a virtual network topology record for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn vnet_to_db_record(
    node_count: u32,
    link_count: u32,
    subnet_hash: u64,
    bandwidth_mbps: u32,
    timestamp_ms: u64,
) -> VnetDbRecord {
    let mut key = [0u8; 32];
    key[0..4].copy_from_slice(&node_count.to_le_bytes());
    key[4..8].copy_from_slice(&link_count.to_le_bytes());
    key[8..16].copy_from_slice(&subnet_hash.to_le_bytes());
    key[16..20].copy_from_slice(&bandwidth_mbps.to_le_bytes());
    key[20..28].copy_from_slice(&timestamp_ms.to_le_bytes());
    key[28..32].copy_from_slice(&node_count.to_le_bytes());
    VnetDbRecord {
        content_hash: fnv1a(&key),
        node_count,
        link_count,
        subnet_hash,
        bandwidth_mbps,
        timestamp_ms,
    }
}

// ── Bridge 2: VNet → Cache (subnet routing cache) ────────────────────────

/// Subnet routing cache entry for ALICE-Cache.
pub struct VnetCacheEntry {
    /// Content hash over subnet + routing fields.
    pub content_hash: u64,
    /// FNV-1a hash of the subnet identifier.
    pub subnet_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of nodes in the cached subnet.
    pub node_count: u32,
    /// Number of routing entries in the cache.
    pub route_count: u32,
}

/// Build a subnet routing cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 60 s when route_count > 500 (large routing table).
#[inline]
#[must_use]
pub fn vnet_to_cache_entry(subnet_hash: u64, node_count: u32, route_count: u32) -> VnetCacheEntry {
    // Branchless large-table TTL: 600 s normal, 60 s when route_count > 500.
    let large = (route_count > 500) as u32;
    let ttl_secs = 600_u32 - large * 540_u32;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&subnet_hash.to_le_bytes());
    key[8..12].copy_from_slice(&node_count.to_le_bytes());
    key[12..16].copy_from_slice(&route_count.to_le_bytes());
    VnetCacheEntry {
        content_hash: fnv1a(&key),
        subnet_hash,
        ttl_secs,
        node_count,
        route_count,
    }
}

// ── Bridge 3: VNet → Analytics (traffic flow metrics) ────────────────────

/// VNet traffic flow analytics event for ALICE-Analytics.
pub struct VnetAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total packets processed in the reporting window.
    pub packet_count: u64,
    /// Total bytes transferred in the reporting window.
    pub byte_count: u64,
    /// Packet drop rate in basis points (10_000 = 100.00%).
    pub drop_rate_bps: u16,
    /// Average round-trip latency in microseconds.
    pub latency_us: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a VNet analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn vnet_to_analytics_event(
    packet_count: u64,
    byte_count: u64,
    drop_rate_bps: u16,
    latency_us: u64,
    timestamp_ms: u64,
) -> VnetAnalyticsEvent {
    let mut key = [0u8; 34];
    key[0..8].copy_from_slice(&packet_count.to_le_bytes());
    key[8..16].copy_from_slice(&byte_count.to_le_bytes());
    key[16..18].copy_from_slice(&drop_rate_bps.to_le_bytes());
    key[18..26].copy_from_slice(&latency_us.to_le_bytes());
    key[26..34].copy_from_slice(&timestamp_ms.to_le_bytes());
    VnetAnalyticsEvent {
        content_hash: fnv1a(&key),
        packet_count,
        byte_count,
        drop_rate_bps,
        latency_us,
        timestamp_ms,
    }
}

// ── Bridge 4: VNet → Edge (edge node telemetry) ───────────────────────────

/// VNet edge node telemetry record for ALICE-Edge.
pub struct VnetEdgeTelemetry {
    /// Content hash over edge node metrics.
    pub content_hash: u64,
    /// Number of active nodes at the edge.
    pub node_count: u32,
    /// Current throughput in megabits per second.
    pub throughput_mbps: u32,
    /// Number of transmission errors recorded.
    pub error_count: u32,
    /// Network utilisation percentage (0–100).
    pub utilization_pct: u8,
}

/// Build a VNet edge node telemetry record for ALICE-Edge.
#[inline]
#[must_use]
pub fn vnet_to_edge_telemetry(
    node_count: u32,
    throughput_mbps: u32,
    error_count: u32,
    utilization_pct: u8,
) -> VnetEdgeTelemetry {
    let mut key = [0u8; 13];
    key[0..4].copy_from_slice(&node_count.to_le_bytes());
    key[4..8].copy_from_slice(&throughput_mbps.to_le_bytes());
    key[8..12].copy_from_slice(&error_count.to_le_bytes());
    key[12] = utilization_pct;
    VnetEdgeTelemetry {
        content_hash: fnv1a(&key),
        node_count,
        throughput_mbps,
        error_count,
        utilization_pct,
    }
}

// ── Bridge 5: VNet → Monitor (subnet health status) ──────────────────────

/// VNet subnet health status for ALICE-Monitor.
pub struct VnetMonitorStatus {
    /// Content hash over health metrics.
    pub content_hash: u64,
    /// FNV-1a hash of the subnet identifier.
    pub subnet_hash: u64,
    /// Total number of nodes in the subnet.
    pub node_count: u32,
    /// Number of links currently down.
    pub link_down_count: u32,
    /// Whether the subnet is considered healthy.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a VNet subnet health status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn vnet_to_monitor_status(
    subnet_hash: u64,
    node_count: u32,
    link_down_count: u32,
    is_healthy: bool,
    timestamp_ms: u64,
) -> VnetMonitorStatus {
    let mut key = [0u8; 21];
    key[0..8].copy_from_slice(&subnet_hash.to_le_bytes());
    key[8..12].copy_from_slice(&node_count.to_le_bytes());
    key[12..16].copy_from_slice(&link_down_count.to_le_bytes());
    key[16] = is_healthy as u8;
    key[17..21].copy_from_slice(&timestamp_ms.to_le_bytes()[..4]);
    VnetMonitorStatus {
        content_hash: fnv1a(&key),
        subnet_hash,
        node_count,
        link_down_count,
        is_healthy,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SUBNET_HASH: u64 = 0xC0A8_0001_FFFF_0001;

    #[test]
    fn test_vnet_to_db_record_hash_nonzero() {
        let rec = vnet_to_db_record(16, 24, SUBNET_HASH, 1_000, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_vnet_to_db_record_fields() {
        let rec = vnet_to_db_record(8, 12, SUBNET_HASH, 500, 1_700_000_000_000);
        assert_eq!(rec.node_count, 8);
        assert_eq!(rec.link_count, 12);
        assert_eq!(rec.subnet_hash, SUBNET_HASH);
        assert_eq!(rec.bandwidth_mbps, 500);
        assert_eq!(rec.timestamp_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_vnet_to_cache_entry_normal_ttl() {
        let entry = vnet_to_cache_entry(SUBNET_HASH, 10, 100);
        assert_eq!(entry.ttl_secs, 600);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_vnet_to_cache_entry_large_table_ttl() {
        // route_count > 500 → reduced TTL = 60 s.
        let entry = vnet_to_cache_entry(SUBNET_HASH, 10, 501);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_vnet_to_analytics_event_fields() {
        let ev = vnet_to_analytics_event(1_000_000, 100_000_000, 5, 200, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.packet_count, 1_000_000);
        assert_eq!(ev.byte_count, 100_000_000);
        assert_eq!(ev.drop_rate_bps, 5);
        assert_eq!(ev.latency_us, 200);
    }

    #[test]
    fn test_vnet_to_analytics_event_determinism() {
        let a = vnet_to_analytics_event(1, 2, 3, 4, 5);
        let b = vnet_to_analytics_event(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_vnet_to_edge_telemetry_fields() {
        let tel = vnet_to_edge_telemetry(32, 10_000, 0, 45);
        assert_ne!(tel.content_hash, 0);
        assert_eq!(tel.node_count, 32);
        assert_eq!(tel.throughput_mbps, 10_000);
        assert_eq!(tel.error_count, 0);
        assert_eq!(tel.utilization_pct, 45);
    }

    #[test]
    fn test_vnet_to_monitor_status_healthy() {
        let s = vnet_to_monitor_status(SUBNET_HASH, 16, 0, true, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(s.is_healthy);
        assert_eq!(s.link_down_count, 0);
    }

    #[test]
    fn test_vnet_to_monitor_status_unhealthy() {
        let s = vnet_to_monitor_status(SUBNET_HASH, 16, 3, false, 1_700_000_000_000);
        assert!(!s.is_healthy);
        assert_eq!(s.link_down_count, 3);
    }
}
