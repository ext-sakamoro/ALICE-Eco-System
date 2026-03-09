//! Hypervisor bridges — ALICE-Hypervisor ↔ DB, Cache, Analytics, Monitor, Auth
//!
//! 5 bridges connecting hypervisor host and VM data (extracted as primitives)
//! to the ALICE ecosystem. No external crate types are imported; all fields
//! use primitive types derived from serialised hypervisor state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Hypervisor → DB (host snapshot persistence) ─────────────────

/// Hypervisor host snapshot for ALICE-DB persistence.
pub struct HypervisorDbRecord {
    /// Content hash over host_hash, vm_count, and timestamp_ms.
    pub content_hash: u64,
    /// Number of virtual machines currently running on the host.
    pub vm_count: u32,
    /// Total virtual CPU count across all VMs.
    pub vcpu_total: u32,
    /// Total memory allocated to VMs in megabytes.
    pub memory_total_mb: u64,
    /// Opaque host identifier hash.
    pub host_hash: u64,
    /// Unix timestamp in milliseconds when the snapshot was captured.
    pub timestamp_ms: u64,
}

/// Build a DB persistence record from extracted hypervisor host data.
#[inline]
#[must_use]
pub fn hypervisor_to_db_record(
    host_id: &[u8],
    vm_count: u32,
    vcpu_total: u32,
    memory_total_mb: u64,
    timestamp_ms: u64,
) -> HypervisorDbRecord {
    let host_hash = fnv1a(host_id);
    let mut buf = [0u8; 20];
    buf[0..8].copy_from_slice(&host_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&vm_count.to_le_bytes());
    buf[12..20].copy_from_slice(&timestamp_ms.to_le_bytes());
    HypervisorDbRecord {
        content_hash: fnv1a(&buf),
        vm_count,
        vcpu_total,
        memory_total_mb,
        host_hash,
        timestamp_ms,
    }
}

// ── Bridge 2: Hypervisor → Cache (live host state caching) ────────────────

/// Cached hypervisor host state entry for ALICE-Cache.
pub struct HypervisorCacheEntry {
    /// Content hash over host_hash and vm_count.
    pub content_hash: u64,
    /// Hashed host identifier used as cache key.
    pub host_hash: u64,
    /// TTL in seconds for this cache entry.
    pub ttl_secs: u32,
    /// Current VM count on the host.
    pub vm_count: u32,
    /// Serialised snapshot size in bytes.
    pub snapshot_bytes: u64,
}

/// Build a cache entry for a hypervisor host's live state.
///
/// TTL is 60 s by default; reduced to 10 s when `vm_count` exceeds 128
/// to ensure high-density hosts stay fresh.
#[inline]
#[must_use]
pub fn hypervisor_to_cache_entry(
    host_id: &[u8],
    vm_count: u32,
    snapshot_bytes: u64,
) -> HypervisorCacheEntry {
    let host_hash = fnv1a(host_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&host_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&vm_count.to_le_bytes());
    let dense = (vm_count > 128) as u32;
    let ttl_secs = 60 - dense * 50;
    HypervisorCacheEntry {
        content_hash: fnv1a(&buf),
        host_hash,
        ttl_secs,
        vm_count,
        snapshot_bytes,
    }
}

// ── Bridge 3: Hypervisor → Analytics (resource metrics ingestion) ──────────

/// Hypervisor resource metrics event for ALICE-Analytics ingestion.
pub struct HypervisorAnalyticsEvent {
    /// Content hash over host_hash and timestamp_ms.
    pub content_hash: u64,
    /// Number of virtual machines on the host at event time.
    pub vm_count: u32,
    /// CPU utilisation as a percentage (0–100).
    pub cpu_usage_pct: u8,
    /// Memory utilisation as a percentage (0–100).
    pub memory_usage_pct: u8,
    /// Total I/O operations since the last event.
    pub io_ops: u64,
    /// Unix timestamp in milliseconds when the event was recorded.
    pub timestamp_ms: u64,
}

/// Build an analytics ingestion event from hypervisor resource metrics.
#[inline]
#[must_use]
pub fn hypervisor_to_analytics_event(
    host_id: &[u8],
    vm_count: u32,
    cpu_usage_pct: u8,
    memory_usage_pct: u8,
    io_ops: u64,
    timestamp_ms: u64,
) -> HypervisorAnalyticsEvent {
    let host_hash = fnv1a(host_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&host_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    HypervisorAnalyticsEvent {
        content_hash: fnv1a(&buf),
        vm_count,
        cpu_usage_pct,
        memory_usage_pct,
        io_ops,
        timestamp_ms,
    }
}

// ── Bridge 4: Hypervisor → Monitor (host health status) ───────────────────

/// Hypervisor host health status for ALICE-Monitor.
pub struct HypervisorMonitorStatus {
    /// Content hash over host_hash and timestamp_ms.
    pub content_hash: u64,
    /// Hashed host identifier.
    pub host_hash: u64,
    /// Number of running VMs on the host.
    pub vm_count: u32,
    /// Memory overcommit ratio multiplied by 100 (e.g., 150 = 1.50×).
    pub overcommit_ratio_x100: u32,
    /// Whether the host is currently considered healthy.
    pub is_healthy: bool,
    /// Unix timestamp in milliseconds of the status report.
    pub timestamp_ms: u64,
}

/// Build a monitor health status from hypervisor host state.
#[inline]
#[must_use]
pub fn hypervisor_to_monitor_status(
    host_id: &[u8],
    vm_count: u32,
    overcommit_ratio_x100: u32,
    is_healthy: bool,
    timestamp_ms: u64,
) -> HypervisorMonitorStatus {
    let host_hash = fnv1a(host_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&host_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    HypervisorMonitorStatus {
        content_hash: fnv1a(&buf),
        host_hash,
        vm_count,
        overcommit_ratio_x100,
        is_healthy,
        timestamp_ms,
    }
}

// ── Bridge 5: Hypervisor → Auth (tenant VM access link) ───────────────────

/// Hypervisor tenant authorisation link for ALICE-Auth.
pub struct HypervisorAuthLink {
    /// Content hash over host_hash and tenant_hash.
    pub content_hash: u64,
    /// Hashed host identifier.
    pub host_hash: u64,
    /// Hashed tenant identifier.
    pub tenant_hash: u64,
    /// Maximum number of VMs this tenant may run on the host.
    pub vm_limit: u32,
    /// Permission bitmask for the tenant on this host.
    pub permission: u8,
}

/// Build an auth link from hypervisor host and tenant identifiers.
#[inline]
#[must_use]
pub fn hypervisor_to_auth_link(
    host_id: &[u8],
    tenant_id: &[u8],
    vm_limit: u32,
    permission: u8,
) -> HypervisorAuthLink {
    let host_hash = fnv1a(host_id);
    let tenant_hash = fnv1a(tenant_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&host_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&tenant_hash.to_le_bytes());
    HypervisorAuthLink {
        content_hash: fnv1a(&buf),
        host_hash,
        tenant_hash,
        vm_limit,
        permission,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = hypervisor_to_db_record(b"host-01", 16, 64, 131_072, 1_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.host_hash, 0);
    }

    #[test]
    fn db_record_fields_preserved() {
        let rec = hypervisor_to_db_record(b"h", 8, 32, 65_536, 42_000);
        assert_eq!(rec.vm_count, 8);
        assert_eq!(rec.vcpu_total, 32);
        assert_eq!(rec.memory_total_mb, 65_536);
        assert_eq!(rec.timestamp_ms, 42_000);
    }

    #[test]
    fn db_record_hash_deterministic() {
        let a = hypervisor_to_db_record(b"hx", 4, 16, 8_192, 0);
        let b = hypervisor_to_db_record(b"hx", 4, 16, 8_192, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_sparse_host_long_ttl() {
        let entry = hypervisor_to_cache_entry(b"h1", 64, 1_048_576);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn cache_entry_dense_host_short_ttl() {
        let entry = hypervisor_to_cache_entry(b"h2", 200, 2_097_152);
        assert_eq!(entry.ttl_secs, 10);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_fields_and_hash() {
        let ev = hypervisor_to_analytics_event(b"host-99", 32, 75, 60, 500_000, 8_000_000);
        assert_eq!(ev.vm_count, 32);
        assert_eq!(ev.cpu_usage_pct, 75);
        assert_eq!(ev.memory_usage_pct, 60);
        assert_eq!(ev.io_ops, 500_000);
        assert_ne!(ev.content_hash, 0);
    }

    // ── Monitor status tests ──────────────────────────────────────────────

    #[test]
    fn monitor_status_healthy_flag_preserved() {
        let st = hypervisor_to_monitor_status(b"hm", 10, 150, true, 3_000);
        assert!(st.is_healthy);
        assert_eq!(st.overcommit_ratio_x100, 150);
        assert_ne!(st.content_hash, 0);
    }

    // ── Auth link tests ───────────────────────────────────────────────────

    #[test]
    fn auth_link_hashes_differ_for_different_tenants() {
        let a = hypervisor_to_auth_link(b"host", b"tenant-a", 10, 0xFF);
        let b = hypervisor_to_auth_link(b"host", b"tenant-b", 10, 0xFF);
        assert_ne!(a.content_hash, b.content_hash);
        assert_ne!(a.tenant_hash, b.tenant_hash);
    }

    #[test]
    fn auth_link_fields_preserved() {
        let link = hypervisor_to_auth_link(b"h", b"t", 5, 0x03);
        assert_eq!(link.vm_limit, 5);
        assert_eq!(link.permission, 0x03);
        assert_ne!(link.content_hash, 0);
    }
}
