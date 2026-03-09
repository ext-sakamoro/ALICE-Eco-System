//! ServiceMesh bridges — ServiceMesh ↔ DB, Cache, Analytics, Monitor, Notify
//!
//! 5 bridges connecting service mesh configuration and metrics to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: ServiceMesh → DB (mesh config persistence) ─────────────────

/// Service mesh configuration record for ALICE-DB persistence.
pub struct ServiceMeshDbRecord {
    /// Content hash over mesh fields.
    pub content_hash: u64,
    /// Number of services registered in the mesh.
    pub service_count: u32,
    /// Number of routing rules configured.
    pub route_count: u32,
    /// FNV-1a hash of the mesh configuration identifier.
    pub mesh_hash: u64,
    /// Configuration version number.
    pub version: u32,
    /// Record timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize a service mesh configuration record for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn service_mesh_to_db_record(
    service_count: u32,
    route_count: u32,
    mesh_hash: u64,
    version: u32,
    timestamp_ms: u64,
) -> ServiceMeshDbRecord {
    let mut key = [0u8; 32];
    key[0..4].copy_from_slice(&service_count.to_le_bytes());
    key[4..8].copy_from_slice(&route_count.to_le_bytes());
    key[8..16].copy_from_slice(&mesh_hash.to_le_bytes());
    key[16..20].copy_from_slice(&version.to_le_bytes());
    key[20..28].copy_from_slice(&timestamp_ms.to_le_bytes());
    key[28..32].copy_from_slice(&route_count.to_le_bytes());
    ServiceMeshDbRecord {
        content_hash: fnv1a(&key),
        service_count,
        route_count,
        mesh_hash,
        version,
        timestamp_ms,
    }
}

// ── Bridge 2: ServiceMesh → Cache (mesh config caching) ──────────────────

/// Service mesh configuration cache entry for ALICE-Cache.
pub struct ServiceMeshCacheEntry {
    /// Content hash over mesh + config fields.
    pub content_hash: u64,
    /// FNV-1a hash of the mesh configuration identifier.
    pub mesh_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of services in the cached configuration.
    pub service_count: u32,
    /// Configuration data size in bytes.
    pub config_bytes: u64,
}

/// Build a service mesh configuration cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 30 s when config_bytes > 1 MiB (large config).
#[inline]
#[must_use]
pub fn service_mesh_to_cache_entry(
    mesh_hash: u64,
    service_count: u32,
    config_bytes: u64,
) -> ServiceMeshCacheEntry {
    // Branchless large-config TTL: 300 s normal, 30 s when config_bytes > 1 MiB.
    let large = (config_bytes > 1_048_576) as u32;
    let ttl_secs = 300_u32 - large * 270_u32;
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&mesh_hash.to_le_bytes());
    key[8..12].copy_from_slice(&service_count.to_le_bytes());
    key[12..20].copy_from_slice(&config_bytes.to_le_bytes());
    ServiceMeshCacheEntry {
        content_hash: fnv1a(&key),
        mesh_hash,
        ttl_secs,
        service_count,
        config_bytes,
    }
}

// ── Bridge 3: ServiceMesh → Analytics (traffic metrics) ──────────────────

/// Service mesh traffic analytics event for ALICE-Analytics.
pub struct ServiceMeshAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total inter-service requests in the reporting window.
    pub request_count: u64,
    /// Total error responses in the reporting window.
    pub error_count: u64,
    /// P99 request latency in microseconds.
    pub p99_latency_us: u64,
    /// Success rate in basis points (10_000 = 100.00%).
    pub success_rate_bps: u16,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a service mesh analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn service_mesh_to_analytics_event(
    request_count: u64,
    error_count: u64,
    p99_latency_us: u64,
    success_rate_bps: u16,
    timestamp_ms: u64,
) -> ServiceMeshAnalyticsEvent {
    let mut key = [0u8; 34];
    key[0..8].copy_from_slice(&request_count.to_le_bytes());
    key[8..16].copy_from_slice(&error_count.to_le_bytes());
    key[16..24].copy_from_slice(&p99_latency_us.to_le_bytes());
    key[24..26].copy_from_slice(&success_rate_bps.to_le_bytes());
    key[26..34].copy_from_slice(&timestamp_ms.to_le_bytes());
    ServiceMeshAnalyticsEvent {
        content_hash: fnv1a(&key),
        request_count,
        error_count,
        p99_latency_us,
        success_rate_bps,
        timestamp_ms,
    }
}

// ── Bridge 4: ServiceMesh → Monitor (mesh health status) ─────────────────

/// Service mesh health status for ALICE-Monitor.
pub struct ServiceMeshMonitorStatus {
    /// Content hash over health metrics.
    pub content_hash: u64,
    /// Total number of services in the mesh.
    pub service_count: u32,
    /// Number of services reporting healthy.
    pub healthy_count: u32,
    /// Number of services with open circuit breakers.
    pub circuit_open_count: u32,
    /// Whether the overall mesh is considered healthy.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a service mesh health status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn service_mesh_to_monitor_status(
    service_count: u32,
    healthy_count: u32,
    circuit_open_count: u32,
    is_healthy: bool,
    timestamp_ms: u64,
) -> ServiceMeshMonitorStatus {
    let mut key = [0u8; 21];
    key[0..4].copy_from_slice(&service_count.to_le_bytes());
    key[4..8].copy_from_slice(&healthy_count.to_le_bytes());
    key[8..12].copy_from_slice(&circuit_open_count.to_le_bytes());
    key[12] = is_healthy as u8;
    key[13..21].copy_from_slice(&timestamp_ms.to_le_bytes());
    ServiceMeshMonitorStatus {
        content_hash: fnv1a(&key),
        service_count,
        healthy_count,
        circuit_open_count,
        is_healthy,
        timestamp_ms,
    }
}

// ── Bridge 5: ServiceMesh → Notify (mesh alert) ───────────────────────────

/// Service mesh alert payload for ALICE-Notify.
pub struct ServiceMeshNotifyAlert {
    /// Content hash over severity + service + error + timestamp.
    pub content_hash: u64,
    /// Severity level: 0=info, 1=warning, 2=critical.
    pub severity: u8,
    /// FNV-1a hash of the affected service identifier.
    pub service_hash: u64,
    /// Number of errors that triggered the alert.
    pub error_count: u64,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a service mesh alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn service_mesh_to_notify_alert(
    severity: u8,
    service_hash: u64,
    error_count: u64,
    timestamp_ms: u64,
) -> ServiceMeshNotifyAlert {
    let mut key = [0u8; 25];
    key[0] = severity;
    key[1..9].copy_from_slice(&service_hash.to_le_bytes());
    key[9..17].copy_from_slice(&error_count.to_le_bytes());
    key[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    ServiceMeshNotifyAlert {
        content_hash: fnv1a(&key),
        severity,
        service_hash,
        error_count,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const MESH_HASH: u64 = 0x1122_3344_5566_7788;
    const SERVICE_HASH: u64 = 0xAABB_CCDD_EEFF_0011;

    #[test]
    fn test_service_mesh_to_db_record_hash_nonzero() {
        let rec = service_mesh_to_db_record(10, 50, MESH_HASH, 3, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_service_mesh_to_db_record_fields() {
        let rec = service_mesh_to_db_record(8, 32, MESH_HASH, 2, 1_700_000_000_000);
        assert_eq!(rec.service_count, 8);
        assert_eq!(rec.route_count, 32);
        assert_eq!(rec.mesh_hash, MESH_HASH);
        assert_eq!(rec.version, 2);
        assert_eq!(rec.timestamp_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_service_mesh_to_cache_entry_normal_ttl() {
        let entry = service_mesh_to_cache_entry(MESH_HASH, 5, 512);
        assert_eq!(entry.ttl_secs, 300);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_service_mesh_to_cache_entry_large_ttl() {
        // config_bytes > 1 MiB → reduced TTL = 30 s.
        let entry = service_mesh_to_cache_entry(MESH_HASH, 5, 2_000_000);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_service_mesh_to_analytics_event_fields() {
        let ev = service_mesh_to_analytics_event(50_000, 100, 2_500, 9_998, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.request_count, 50_000);
        assert_eq!(ev.error_count, 100);
        assert_eq!(ev.p99_latency_us, 2_500);
        assert_eq!(ev.success_rate_bps, 9_998);
    }

    #[test]
    fn test_service_mesh_to_analytics_event_determinism() {
        let a = service_mesh_to_analytics_event(1, 0, 100, 10_000, 0);
        let b = service_mesh_to_analytics_event(1, 0, 100, 10_000, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_service_mesh_to_monitor_status_healthy() {
        let s = service_mesh_to_monitor_status(10, 10, 0, true, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(s.is_healthy);
        assert_eq!(s.circuit_open_count, 0);
    }

    #[test]
    fn test_service_mesh_to_notify_alert_fields() {
        let alert = service_mesh_to_notify_alert(2, SERVICE_HASH, 500, 1_700_000_000_000);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.service_hash, SERVICE_HASH);
        assert_eq!(alert.error_count, 500);
    }

    #[test]
    fn test_service_mesh_to_notify_alert_determinism() {
        let a = service_mesh_to_notify_alert(1, SERVICE_HASH, 10, 99);
        let b = service_mesh_to_notify_alert(1, SERVICE_HASH, 10, 99);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
