//! Sandbox bridges — Sandbox ↔ DB, Cache, Analytics, Monitor, Auth
//!
//! 5 bridges connecting sandbox execution data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Sandbox → DB (execution record persistence) ────────────────

/// Sandbox execution record for ALICE-DB persistence.
pub struct SandboxDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Sandbox instance identifier hash.
    pub instance_hash: u64,
    /// Execution runtime in milliseconds.
    pub runtime_ms: u64,
    /// Memory limit in bytes.
    pub memory_limit_bytes: u64,
    /// CPU limit in millicores.
    pub cpu_limit_milli: u32,
    /// Process exit code.
    pub exit_code: i32,
}

/// Serialize sandbox execution data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn sandbox_to_db_record(
    instance_hash: u64,
    runtime_ms: u64,
    memory_limit_bytes: u64,
    cpu_limit_milli: u32,
    exit_code: i32,
) -> SandboxDbRecord {
    // buf: instance_hash(8) + runtime_ms(8) + memory_limit_bytes(8) + cpu_limit_milli(4) + exit_code(4) = 32
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&instance_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&runtime_ms.to_le_bytes());
    buf[16..24].copy_from_slice(&memory_limit_bytes.to_le_bytes());
    buf[24..28].copy_from_slice(&cpu_limit_milli.to_le_bytes());
    buf[28..32].copy_from_slice(&exit_code.to_le_bytes());
    SandboxDbRecord {
        content_hash: fnv1a(&buf),
        instance_hash,
        runtime_ms,
        memory_limit_bytes,
        cpu_limit_milli,
        exit_code,
    }
}

// ── Bridge 2: Sandbox → Cache (instance snapshot cache) ──────────────────

/// Sandbox instance snapshot cache entry for ALICE-Cache.
pub struct SandboxCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Sandbox instance identifier hash.
    pub instance_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Snapshot size in bytes.
    pub snapshot_bytes: u64,
    /// Instance state code (0 = idle, 1 = running, 2 = stopped).
    pub state: u8,
}

/// Build sandbox instance snapshot cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn sandbox_to_cache_entry(
    instance_hash: u64,
    ttl_secs: u32,
    snapshot_bytes: u64,
    state: u8,
) -> SandboxCacheEntry {
    // buf: instance_hash(8) + snapshot_bytes(8) + state(1) = 17
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&instance_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&snapshot_bytes.to_le_bytes());
    buf[16] = state;
    SandboxCacheEntry {
        content_hash: fnv1a(&buf),
        instance_hash,
        ttl_secs,
        snapshot_bytes,
        state,
    }
}

// ── Bridge 3: Sandbox → Analytics (execution analytics event) ────────────

/// Sandbox analytics event for ALICE-Analytics ingestion.
pub struct SandboxAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Cumulative execution count.
    pub exec_count: u64,
    /// Average runtime in milliseconds.
    pub avg_runtime_ms: u64,
    /// Out-of-memory kill count.
    pub oom_count: u32,
    /// Execution timeout count.
    pub timeout_count: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build sandbox analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn sandbox_to_analytics_event(
    exec_count: u64,
    avg_runtime_ms: u64,
    oom_count: u32,
    timeout_count: u32,
    timestamp_ms: u64,
) -> SandboxAnalyticsEvent {
    // buf: exec_count(8) + avg_runtime_ms(8) + oom_count(4) + timeout_count(4) + timestamp_ms(8) = 32
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&exec_count.to_le_bytes());
    buf[8..16].copy_from_slice(&avg_runtime_ms.to_le_bytes());
    buf[16..20].copy_from_slice(&oom_count.to_le_bytes());
    buf[20..24].copy_from_slice(&timeout_count.to_le_bytes());
    buf[24..32].copy_from_slice(&timestamp_ms.to_le_bytes());
    SandboxAnalyticsEvent {
        content_hash: fnv1a(&buf),
        exec_count,
        avg_runtime_ms,
        oom_count,
        timeout_count,
        timestamp_ms,
    }
}

// ── Bridge 4: Sandbox → Monitor (instance health status) ─────────────────

/// Sandbox instance health status for ALICE-Monitor.
pub struct SandboxMonitorStatus {
    /// Content hash.
    pub content_hash: u64,
    /// Number of active sandbox instances.
    pub active_instances: u32,
    /// Current memory usage in bytes.
    pub memory_usage_bytes: u64,
    /// CPU utilization percentage (0–100).
    pub cpu_usage_pct: u8,
    /// Whether the sandbox service is healthy.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build sandbox monitor status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn sandbox_to_monitor_status(
    active_instances: u32,
    memory_usage_bytes: u64,
    cpu_usage_pct: u8,
    is_healthy: bool,
    timestamp_ms: u64,
) -> SandboxMonitorStatus {
    // buf: active_instances(4) + memory_usage_bytes(8) + cpu_usage_pct(1) + is_healthy(1) + timestamp_ms(8) = 22
    let mut buf = [0u8; 22];
    buf[0..4].copy_from_slice(&active_instances.to_le_bytes());
    buf[4..12].copy_from_slice(&memory_usage_bytes.to_le_bytes());
    buf[12] = cpu_usage_pct;
    buf[13] = is_healthy as u8;
    buf[14..22].copy_from_slice(&timestamp_ms.to_le_bytes());
    SandboxMonitorStatus {
        content_hash: fnv1a(&buf),
        active_instances,
        memory_usage_bytes,
        cpu_usage_pct,
        is_healthy,
        timestamp_ms,
    }
}

// ── Bridge 5: Sandbox → Auth (instance access link) ──────────────────────

/// Sandbox instance auth access link for ALICE-Auth.
pub struct SandboxAuthLink {
    /// Content hash.
    pub content_hash: u64,
    /// Sandbox instance identifier hash.
    pub instance_hash: u64,
    /// Principal (user/service) identifier hash.
    pub principal_hash: u64,
    /// Permission bitmask.
    pub permission: u8,
    /// Last access timestamp in seconds since epoch.
    pub last_access_ts: u64,
}

/// Build sandbox auth access link for ALICE-Auth.
#[inline]
#[must_use]
pub fn sandbox_to_auth_link(
    instance_hash: u64,
    principal_hash: u64,
    permission: u8,
    last_access_ts: u64,
) -> SandboxAuthLink {
    // buf: instance_hash(8) + principal_hash(8) + permission(1) + last_access_ts(8) = 25
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&instance_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&principal_hash.to_le_bytes());
    buf[16] = permission;
    buf[17..25].copy_from_slice(&last_access_ts.to_le_bytes());
    SandboxAuthLink {
        content_hash: fnv1a(&buf),
        instance_hash,
        principal_hash,
        permission,
        last_access_ts,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_to_db_record_hash_nonzero() {
        let rec = sandbox_to_db_record(0xdead_beef_cafe_0001, 1_500, 536_870_912, 500, 0);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_sandbox_to_db_record_fields() {
        let rec = sandbox_to_db_record(0x1234, 2_000, 268_435_456, 250, 1);
        assert_eq!(rec.instance_hash, 0x1234);
        assert_eq!(rec.runtime_ms, 2_000);
        assert_eq!(rec.memory_limit_bytes, 268_435_456);
        assert_eq!(rec.cpu_limit_milli, 250);
        assert_eq!(rec.exit_code, 1);
    }

    #[test]
    fn test_sandbox_to_db_record_exit_code_negative() {
        let rec = sandbox_to_db_record(0xffff, 100, 65_536, 100, -1);
        assert_eq!(rec.exit_code, -1);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_sandbox_to_cache_entry_hash_nonzero() {
        let entry = sandbox_to_cache_entry(0xabcd, 900, 1_048_576, 1);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_sandbox_to_cache_entry_fields() {
        let entry = sandbox_to_cache_entry(0x7777, 600, 512, 0);
        assert_eq!(entry.instance_hash, 0x7777);
        assert_eq!(entry.ttl_secs, 600);
        assert_eq!(entry.snapshot_bytes, 512);
        assert_eq!(entry.state, 0);
    }

    #[test]
    fn test_sandbox_to_analytics_event_hash_nonzero() {
        let ev = sandbox_to_analytics_event(1_000, 800, 5, 3, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_sandbox_to_analytics_event_fields() {
        let ev = sandbox_to_analytics_event(50, 1_200, 2, 1, 99_999);
        assert_eq!(ev.exec_count, 50);
        assert_eq!(ev.avg_runtime_ms, 1_200);
        assert_eq!(ev.oom_count, 2);
        assert_eq!(ev.timeout_count, 1);
        assert_eq!(ev.timestamp_ms, 99_999);
    }

    #[test]
    fn test_sandbox_to_monitor_status_healthy() {
        let st = sandbox_to_monitor_status(4, 1_073_741_824, 60, true, 1_700_000_000_000);
        assert_ne!(st.content_hash, 0);
        assert!(st.is_healthy);
        assert_eq!(st.active_instances, 4);
        assert_eq!(st.cpu_usage_pct, 60);
    }

    #[test]
    fn test_sandbox_to_monitor_status_unhealthy() {
        let st = sandbox_to_monitor_status(0, 0, 0, false, 1_700_000_000_001);
        assert!(!st.is_healthy);
    }

    #[test]
    fn test_sandbox_to_auth_link_hash_nonzero() {
        let link = sandbox_to_auth_link(0xbeef_cafe, 0x1234_5678, 0b0000_0111, 1_700_000_000);
        assert_ne!(link.content_hash, 0);
    }

    #[test]
    fn test_sandbox_to_auth_link_fields() {
        let link = sandbox_to_auth_link(0x1111, 0x2222, 3, 9_999_999);
        assert_eq!(link.instance_hash, 0x1111);
        assert_eq!(link.principal_hash, 0x2222);
        assert_eq!(link.permission, 3);
        assert_eq!(link.last_access_ts, 9_999_999);
    }

    #[test]
    fn test_sandbox_to_auth_link_determinism() {
        let a = sandbox_to_auth_link(0xaa, 0xbb, 1, 100);
        let b = sandbox_to_auth_link(0xaa, 0xbb, 1, 100);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
