//! CI bridges — CI ↔ DB, Cache, Analytics, Monitor, Notify
//!
//! 5 bridges connecting CI pipeline data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: CI → DB (pipeline record persistence) ─────────────────────

/// Pipeline record for ALICE-DB persistence.
pub struct CiDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Pipeline identifier hash.
    pub pipeline_hash: u64,
    /// Total build count.
    pub build_count: u64,
    /// Successful build count.
    pub pass_count: u64,
    /// Failed build count.
    pub fail_count: u64,
    /// Average build duration in milliseconds.
    pub avg_duration_ms: u64,
}

/// Serialize CI pipeline data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn ci_to_db_record(
    pipeline_hash: u64,
    build_count: u64,
    pass_count: u64,
    fail_count: u64,
    avg_duration_ms: u64,
) -> CiDbRecord {
    // buf: pipeline_hash(8) + build_count(8) + pass_count(8) + fail_count(8) + avg_duration_ms(8) = 40
    let mut buf = [0u8; 40];
    buf[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&build_count.to_le_bytes());
    buf[16..24].copy_from_slice(&pass_count.to_le_bytes());
    buf[24..32].copy_from_slice(&fail_count.to_le_bytes());
    buf[32..40].copy_from_slice(&avg_duration_ms.to_le_bytes());
    CiDbRecord {
        content_hash: fnv1a(&buf),
        pipeline_hash,
        build_count,
        pass_count,
        fail_count,
        avg_duration_ms,
    }
}

// ── Bridge 2: CI → Cache (artifact cache entry) ──────────────────────────

/// Artifact cache entry for ALICE-Cache.
pub struct CiCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Pipeline identifier hash.
    pub pipeline_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Total artifact size in bytes.
    pub artifact_bytes: u64,
    /// Build number.
    pub build_number: u64,
}

/// Build CI artifact cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn ci_to_cache_entry(
    pipeline_hash: u64,
    ttl_secs: u32,
    artifact_bytes: u64,
    build_number: u64,
) -> CiCacheEntry {
    // buf: pipeline_hash(8) + artifact_bytes(8) + build_number(8) = 24
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&artifact_bytes.to_le_bytes());
    buf[16..24].copy_from_slice(&build_number.to_le_bytes());
    CiCacheEntry {
        content_hash: fnv1a(&buf),
        pipeline_hash,
        ttl_secs,
        artifact_bytes,
        build_number,
    }
}

// ── Bridge 3: CI → Analytics (build event metrics) ───────────────────────

/// Build analytics event for ALICE-Analytics ingestion.
pub struct CiAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Cumulative build count.
    pub build_count: u64,
    /// Success rate in basis points (0–10000).
    pub success_rate_bps: u16,
    /// Average build duration in milliseconds.
    pub avg_duration_ms: u64,
    /// Total test count executed.
    pub test_count: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build CI analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn ci_to_analytics_event(
    build_count: u64,
    success_rate_bps: u16,
    avg_duration_ms: u64,
    test_count: u32,
    timestamp_ms: u64,
) -> CiAnalyticsEvent {
    // buf: build_count(8) + success_rate_bps(2) + avg_duration_ms(8) + test_count(4) + timestamp_ms(8) = 30
    let mut buf = [0u8; 30];
    buf[0..8].copy_from_slice(&build_count.to_le_bytes());
    buf[8..10].copy_from_slice(&success_rate_bps.to_le_bytes());
    buf[10..18].copy_from_slice(&avg_duration_ms.to_le_bytes());
    buf[18..22].copy_from_slice(&test_count.to_le_bytes());
    buf[22..30].copy_from_slice(&timestamp_ms.to_le_bytes());
    CiAnalyticsEvent {
        content_hash: fnv1a(&buf),
        build_count,
        success_rate_bps,
        avg_duration_ms,
        test_count,
        timestamp_ms,
    }
}

// ── Bridge 4: CI → Monitor (pipeline health status) ──────────────────────

/// Pipeline health status for ALICE-Monitor.
pub struct CiMonitorStatus {
    /// Content hash.
    pub content_hash: u64,
    /// Pipeline identifier hash.
    pub pipeline_hash: u64,
    /// Number of builds waiting in queue.
    pub queue_depth: u32,
    /// Number of builds currently running.
    pub running_count: u32,
    /// Whether the pipeline is healthy.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build CI monitor status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn ci_to_monitor_status(
    pipeline_hash: u64,
    queue_depth: u32,
    running_count: u32,
    is_healthy: bool,
    timestamp_ms: u64,
) -> CiMonitorStatus {
    // buf: pipeline_hash(8) + queue_depth(4) + running_count(4) + is_healthy(1) + timestamp_ms(8) = 25
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&queue_depth.to_le_bytes());
    buf[12..16].copy_from_slice(&running_count.to_le_bytes());
    buf[16] = is_healthy as u8;
    buf[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    CiMonitorStatus {
        content_hash: fnv1a(&buf),
        pipeline_hash,
        queue_depth,
        running_count,
        is_healthy,
        timestamp_ms,
    }
}

// ── Bridge 5: CI → Notify (pipeline failure alert) ───────────────────────

/// Pipeline failure alert for ALICE-Notify.
pub struct CiNotifyAlert {
    /// Content hash.
    pub content_hash: u64,
    /// Alert severity level (0 = info, 1 = warn, 2 = critical).
    pub severity: u8,
    /// Pipeline identifier hash.
    pub pipeline_hash: u64,
    /// Consecutive failure count.
    pub fail_count: u64,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build CI failure alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn ci_to_notify_alert(
    severity: u8,
    pipeline_hash: u64,
    fail_count: u64,
    timestamp_ms: u64,
) -> CiNotifyAlert {
    // buf: severity(1) + pipeline_hash(8) + fail_count(8) + timestamp_ms(8) = 25
    let mut buf = [0u8; 25];
    buf[0] = severity;
    buf[1..9].copy_from_slice(&pipeline_hash.to_le_bytes());
    buf[9..17].copy_from_slice(&fail_count.to_le_bytes());
    buf[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    CiNotifyAlert {
        content_hash: fnv1a(&buf),
        severity,
        pipeline_hash,
        fail_count,
        timestamp_ms,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ci_to_db_record_hash_nonzero() {
        let rec = ci_to_db_record(0xdead_beef_cafe_1234, 100, 90, 10, 3_500);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_ci_to_db_record_fields() {
        let rec = ci_to_db_record(0x1111, 50, 40, 10, 2_000);
        assert_eq!(rec.pipeline_hash, 0x1111);
        assert_eq!(rec.build_count, 50);
        assert_eq!(rec.pass_count, 40);
        assert_eq!(rec.fail_count, 10);
        assert_eq!(rec.avg_duration_ms, 2_000);
    }

    #[test]
    fn test_ci_to_db_record_determinism() {
        let a = ci_to_db_record(0x42, 10, 8, 2, 500);
        let b = ci_to_db_record(0x42, 10, 8, 2, 500);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_ci_to_cache_entry_hash_nonzero() {
        let entry = ci_to_cache_entry(0xabcd_1234, 3_600, 1_048_576, 42);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_ci_to_cache_entry_fields() {
        let entry = ci_to_cache_entry(0x9999, 7_200, 512, 7);
        assert_eq!(entry.pipeline_hash, 0x9999);
        assert_eq!(entry.ttl_secs, 7_200);
        assert_eq!(entry.artifact_bytes, 512);
        assert_eq!(entry.build_number, 7);
    }

    #[test]
    fn test_ci_to_analytics_event_hash_nonzero() {
        let ev = ci_to_analytics_event(200, 9_500, 4_000, 1_500, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_ci_to_analytics_event_fields() {
        let ev = ci_to_analytics_event(10, 8_000, 2_500, 300, 999_999);
        assert_eq!(ev.build_count, 10);
        assert_eq!(ev.success_rate_bps, 8_000);
        assert_eq!(ev.avg_duration_ms, 2_500);
        assert_eq!(ev.test_count, 300);
        assert_eq!(ev.timestamp_ms, 999_999);
    }

    #[test]
    fn test_ci_to_monitor_status_healthy() {
        let st = ci_to_monitor_status(0xface, 3, 2, true, 1_700_000_000_000);
        assert_ne!(st.content_hash, 0);
        assert!(st.is_healthy);
        assert_eq!(st.queue_depth, 3);
        assert_eq!(st.running_count, 2);
    }

    #[test]
    fn test_ci_to_monitor_status_unhealthy() {
        let st = ci_to_monitor_status(0xface, 20, 0, false, 1_700_000_000_001);
        assert!(!st.is_healthy);
        assert_eq!(st.queue_depth, 20);
    }

    #[test]
    fn test_ci_to_notify_alert_hash_nonzero() {
        let alert = ci_to_notify_alert(2, 0xbeef_cafe, 5, 1_700_000_000_000);
        assert_ne!(alert.content_hash, 0);
    }

    #[test]
    fn test_ci_to_notify_alert_fields() {
        let alert = ci_to_notify_alert(1, 0x1234, 3, 55_555);
        assert_eq!(alert.severity, 1);
        assert_eq!(alert.pipeline_hash, 0x1234);
        assert_eq!(alert.fail_count, 3);
        assert_eq!(alert.timestamp_ms, 55_555);
    }

    #[test]
    fn test_ci_to_notify_alert_determinism() {
        let a = ci_to_notify_alert(2, 0xffff, 10, 100);
        let b = ci_to_notify_alert(2, 0xffff, 10, 100);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
