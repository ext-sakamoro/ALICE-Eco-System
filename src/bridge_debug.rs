//! Debug bridges — Debug ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting debug session data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Debug → DB (session record persistence) ────────────────────

/// Debug session record for ALICE-DB persistence.
pub struct DebugDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Debug session identifier hash.
    pub session_hash: u64,
    /// Number of breakpoints set in the session.
    pub breakpoint_count: u32,
    /// Total execution steps taken.
    pub step_count: u64,
    /// Number of variables inspected.
    pub variable_count: u32,
    /// Session timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize debug session for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn debug_to_db_record(
    session_hash: u64,
    breakpoint_count: u32,
    step_count: u64,
    variable_count: u32,
    timestamp_ms: u64,
) -> DebugDbRecord {
    // buf: session_hash(8) + breakpoint_count(4) + step_count(8) + variable_count(4) + timestamp_ms(8) = 32
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&session_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&breakpoint_count.to_le_bytes());
    buf[12..20].copy_from_slice(&step_count.to_le_bytes());
    buf[20..24].copy_from_slice(&variable_count.to_le_bytes());
    buf[24..32].copy_from_slice(&timestamp_ms.to_le_bytes());
    DebugDbRecord {
        content_hash: fnv1a(&buf),
        session_hash,
        breakpoint_count,
        step_count,
        variable_count,
        timestamp_ms,
    }
}

// ── Bridge 2: Debug → Cache (session snapshot cache) ─────────────────────

/// Debug session snapshot cache entry for ALICE-Cache.
pub struct DebugCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Debug session identifier hash.
    pub session_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Snapshot size in bytes.
    pub snapshot_bytes: u64,
    /// Current call stack depth.
    pub stack_depth: u32,
}

/// Build debug session snapshot cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn debug_to_cache_entry(
    session_hash: u64,
    ttl_secs: u32,
    snapshot_bytes: u64,
    stack_depth: u32,
) -> DebugCacheEntry {
    // buf: session_hash(8) + snapshot_bytes(8) + stack_depth(4) = 20
    let mut buf = [0u8; 20];
    buf[0..8].copy_from_slice(&session_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&snapshot_bytes.to_le_bytes());
    buf[16..20].copy_from_slice(&stack_depth.to_le_bytes());
    DebugCacheEntry {
        content_hash: fnv1a(&buf),
        session_hash,
        ttl_secs,
        snapshot_bytes,
        stack_depth,
    }
}

// ── Bridge 3: Debug → Analytics (session analytics event) ────────────────

/// Debug analytics event for ALICE-Analytics ingestion.
pub struct DebugAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Cumulative session count.
    pub session_count: u64,
    /// Average session duration in milliseconds.
    pub avg_duration_ms: u64,
    /// Total crash count.
    pub crash_count: u32,
    /// Total exception count.
    pub exception_count: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build debug analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn debug_to_analytics_event(
    session_count: u64,
    avg_duration_ms: u64,
    crash_count: u32,
    exception_count: u32,
    timestamp_ms: u64,
) -> DebugAnalyticsEvent {
    // buf: session_count(8) + avg_duration_ms(8) + crash_count(4) + exception_count(4) + timestamp_ms(8) = 32
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&session_count.to_le_bytes());
    buf[8..16].copy_from_slice(&avg_duration_ms.to_le_bytes());
    buf[16..20].copy_from_slice(&crash_count.to_le_bytes());
    buf[20..24].copy_from_slice(&exception_count.to_le_bytes());
    buf[24..32].copy_from_slice(&timestamp_ms.to_le_bytes());
    DebugAnalyticsEvent {
        content_hash: fnv1a(&buf),
        session_count,
        avg_duration_ms,
        crash_count,
        exception_count,
        timestamp_ms,
    }
}

// ── Bridge 4: Debug → Monitor (session health status) ────────────────────

/// Debug session health status for ALICE-Monitor.
pub struct DebugMonitorStatus {
    /// Content hash.
    pub content_hash: u64,
    /// Number of active debug sessions.
    pub active_sessions: u32,
    /// Current memory usage in bytes.
    pub memory_usage_bytes: u64,
    /// CPU utilization percentage (0–100).
    pub cpu_pct: u8,
    /// Whether the debug service is healthy.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build debug monitor status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn debug_to_monitor_status(
    active_sessions: u32,
    memory_usage_bytes: u64,
    cpu_pct: u8,
    is_healthy: bool,
    timestamp_ms: u64,
) -> DebugMonitorStatus {
    // buf: active_sessions(4) + memory_usage_bytes(8) + cpu_pct(1) + is_healthy(1) + timestamp_ms(8) = 22
    let mut buf = [0u8; 22];
    buf[0..4].copy_from_slice(&active_sessions.to_le_bytes());
    buf[4..12].copy_from_slice(&memory_usage_bytes.to_le_bytes());
    buf[12] = cpu_pct;
    buf[13] = is_healthy as u8;
    buf[14..22].copy_from_slice(&timestamp_ms.to_le_bytes());
    DebugMonitorStatus {
        content_hash: fnv1a(&buf),
        active_sessions,
        memory_usage_bytes,
        cpu_pct,
        is_healthy,
        timestamp_ms,
    }
}

// ── Bridge 5: Debug → API (session payload) ──────────────────────────────

/// Debug session API payload for ALICE-API responses.
pub struct DebugApiPayload {
    /// Content hash.
    pub content_hash: u64,
    /// Debug session identifier hash.
    pub session_hash: u64,
    /// Number of breakpoints set.
    pub breakpoint_count: u32,
    /// Current call stack depth.
    pub stack_depth: u32,
    /// API schema version.
    pub schema_version: u16,
}

/// Build debug session API payload for ALICE-API.
#[inline]
#[must_use]
pub fn debug_to_api_payload(
    session_hash: u64,
    breakpoint_count: u32,
    stack_depth: u32,
    schema_version: u16,
) -> DebugApiPayload {
    // buf: session_hash(8) + breakpoint_count(4) + stack_depth(4) + schema_version(2) = 18
    let mut buf = [0u8; 18];
    buf[0..8].copy_from_slice(&session_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&breakpoint_count.to_le_bytes());
    buf[12..16].copy_from_slice(&stack_depth.to_le_bytes());
    buf[16..18].copy_from_slice(&schema_version.to_le_bytes());
    DebugApiPayload {
        content_hash: fnv1a(&buf),
        session_hash,
        breakpoint_count,
        stack_depth,
        schema_version,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_to_db_record_hash_nonzero() {
        let rec = debug_to_db_record(0xabcd_1234_5678_9012, 5, 1_000, 42, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_debug_to_db_record_fields() {
        let rec = debug_to_db_record(0x1111, 3, 500, 20, 99_999);
        assert_eq!(rec.session_hash, 0x1111);
        assert_eq!(rec.breakpoint_count, 3);
        assert_eq!(rec.step_count, 500);
        assert_eq!(rec.variable_count, 20);
        assert_eq!(rec.timestamp_ms, 99_999);
    }

    #[test]
    fn test_debug_to_db_record_determinism() {
        let a = debug_to_db_record(0x42, 1, 10, 5, 123);
        let b = debug_to_db_record(0x42, 1, 10, 5, 123);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_debug_to_cache_entry_hash_nonzero() {
        let entry = debug_to_cache_entry(0xbeef, 1_800, 65_536, 8);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_debug_to_cache_entry_fields() {
        let entry = debug_to_cache_entry(0x9999, 3_600, 1_024, 4);
        assert_eq!(entry.session_hash, 0x9999);
        assert_eq!(entry.ttl_secs, 3_600);
        assert_eq!(entry.snapshot_bytes, 1_024);
        assert_eq!(entry.stack_depth, 4);
    }

    #[test]
    fn test_debug_to_analytics_event_hash_nonzero() {
        let ev = debug_to_analytics_event(50, 30_000, 2, 15, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_debug_to_analytics_event_fields() {
        let ev = debug_to_analytics_event(10, 5_000, 1, 3, 77_777);
        assert_eq!(ev.session_count, 10);
        assert_eq!(ev.avg_duration_ms, 5_000);
        assert_eq!(ev.crash_count, 1);
        assert_eq!(ev.exception_count, 3);
        assert_eq!(ev.timestamp_ms, 77_777);
    }

    #[test]
    fn test_debug_to_monitor_status_healthy() {
        let st = debug_to_monitor_status(2, 134_217_728, 45, true, 1_700_000_000_000);
        assert_ne!(st.content_hash, 0);
        assert!(st.is_healthy);
        assert_eq!(st.active_sessions, 2);
        assert_eq!(st.cpu_pct, 45);
    }

    #[test]
    fn test_debug_to_monitor_status_unhealthy() {
        let st = debug_to_monitor_status(0, 0, 100, false, 1_700_000_000_001);
        assert!(!st.is_healthy);
        assert_eq!(st.cpu_pct, 100);
    }

    #[test]
    fn test_debug_to_api_payload_hash_nonzero() {
        let payload = debug_to_api_payload(0xcafe_babe, 7, 12, 1);
        assert_ne!(payload.content_hash, 0);
    }

    #[test]
    fn test_debug_to_api_payload_fields() {
        let payload = debug_to_api_payload(0x5555, 4, 6, 2);
        assert_eq!(payload.session_hash, 0x5555);
        assert_eq!(payload.breakpoint_count, 4);
        assert_eq!(payload.stack_depth, 6);
        assert_eq!(payload.schema_version, 2);
    }

    #[test]
    fn test_debug_to_api_payload_determinism() {
        let a = debug_to_api_payload(0xff, 1, 2, 3);
        let b = debug_to_api_payload(0xff, 1, 2, 3);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
