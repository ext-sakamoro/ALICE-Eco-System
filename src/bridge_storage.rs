//! Storage bridges — Storage ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting object storage metadata to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Storage → DB (bucket metadata persistence) ─────────────────

/// Object storage bucket record for ALICE-DB persistence.
pub struct StorageDbRecord {
    /// Content hash over bucket fields.
    pub content_hash: u64,
    /// Number of buckets in this storage account.
    pub bucket_count: u32,
    /// Total number of objects stored.
    pub object_count: u64,
    /// Total storage consumed in bytes.
    pub total_bytes: u64,
    /// Storage class code: 0=standard, 1=infrequent_access, 2=archive.
    pub storage_class: u8,
    /// FNV-1a hash of the region identifier.
    pub region_hash: u64,
}

/// Serialize an object storage bucket record for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn storage_to_db_record(
    bucket_count: u32,
    object_count: u64,
    total_bytes: u64,
    storage_class: u8,
    region_hash: u64,
) -> StorageDbRecord {
    let mut key = [0u8; 29];
    key[0..4].copy_from_slice(&bucket_count.to_le_bytes());
    key[4..12].copy_from_slice(&object_count.to_le_bytes());
    key[12..20].copy_from_slice(&total_bytes.to_le_bytes());
    key[20] = storage_class;
    key[21..29].copy_from_slice(&region_hash.to_le_bytes());
    StorageDbRecord {
        content_hash: fnv1a(&key),
        bucket_count,
        object_count,
        total_bytes,
        storage_class,
        region_hash,
    }
}

// ── Bridge 2: Storage → Cache (object metadata caching) ──────────────────

/// Object metadata cache entry for ALICE-Cache.
pub struct StorageCacheEntry {
    /// Content hash over object + access fields.
    pub content_hash: u64,
    /// FNV-1a hash of the object key.
    pub object_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Object size in bytes.
    pub object_bytes: u64,
    /// Last access timestamp in milliseconds since epoch.
    pub last_access_ts: u64,
}

/// Build an object metadata cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 60 s when object_bytes > 100 MiB (large object).
#[inline]
#[must_use]
pub fn storage_to_cache_entry(
    object_hash: u64,
    object_bytes: u64,
    last_access_ts: u64,
) -> StorageCacheEntry {
    // Branchless large-object TTL: 3600 s normal, 60 s when object_bytes > 100 MiB.
    let large = (object_bytes > 104_857_600) as u32;
    let ttl_secs = 3_600_u32 - large * 3_540_u32;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&object_hash.to_le_bytes());
    key[8..16].copy_from_slice(&object_bytes.to_le_bytes());
    key[16..24].copy_from_slice(&last_access_ts.to_le_bytes());
    StorageCacheEntry {
        content_hash: fnv1a(&key),
        object_hash,
        ttl_secs,
        object_bytes,
        last_access_ts,
    }
}

// ── Bridge 3: Storage → Analytics (I/O metrics) ──────────────────────────

/// Object storage I/O analytics event for ALICE-Analytics.
pub struct StorageAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total read operations in the reporting window.
    pub read_ops: u64,
    /// Total write operations in the reporting window.
    pub write_ops: u64,
    /// Total bytes transferred (read + write).
    pub bytes_transferred: u64,
    /// Average operation latency in microseconds.
    pub latency_us: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an object storage I/O analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn storage_to_analytics_event(
    read_ops: u64,
    write_ops: u64,
    bytes_transferred: u64,
    latency_us: u64,
    timestamp_ms: u64,
) -> StorageAnalyticsEvent {
    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&read_ops.to_le_bytes());
    key[8..16].copy_from_slice(&write_ops.to_le_bytes());
    key[16..24].copy_from_slice(&bytes_transferred.to_le_bytes());
    key[24..32].copy_from_slice(&latency_us.to_le_bytes());
    key[32..40].copy_from_slice(&timestamp_ms.to_le_bytes());
    StorageAnalyticsEvent {
        content_hash: fnv1a(&key),
        read_ops,
        write_ops,
        bytes_transferred,
        latency_us,
        timestamp_ms,
    }
}

// ── Bridge 4: Storage → Monitor (storage health status) ──────────────────

/// Object storage health status for ALICE-Monitor.
pub struct StorageMonitorStatus {
    /// Content hash over health metrics.
    pub content_hash: u64,
    /// Total number of buckets.
    pub bucket_count: u32,
    /// Storage usage percentage (0–100).
    pub usage_pct: u8,
    /// Number of I/O errors recorded.
    pub error_count: u32,
    /// Whether the storage service is considered healthy.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an object storage health status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn storage_to_monitor_status(
    bucket_count: u32,
    usage_pct: u8,
    error_count: u32,
    is_healthy: bool,
    timestamp_ms: u64,
) -> StorageMonitorStatus {
    let mut key = [0u8; 22];
    key[0..4].copy_from_slice(&bucket_count.to_le_bytes());
    key[4] = usage_pct;
    key[5..9].copy_from_slice(&error_count.to_le_bytes());
    key[9] = is_healthy as u8;
    key[10..18].copy_from_slice(&timestamp_ms.to_le_bytes());
    key[18..22].copy_from_slice(&bucket_count.to_le_bytes());
    StorageMonitorStatus {
        content_hash: fnv1a(&key),
        bucket_count,
        usage_pct,
        error_count,
        is_healthy,
        timestamp_ms,
    }
}

// ── Bridge 5: Storage → API (storage summary payload) ────────────────────

/// Object storage summary payload for ALICE-API responses.
pub struct StorageApiPayload {
    /// Content hash over summary fields.
    pub content_hash: u64,
    /// Number of buckets in the storage account.
    pub bucket_count: u32,
    /// Total storage consumed in bytes.
    pub total_bytes: u64,
    /// Total number of objects stored.
    pub object_count: u64,
    /// API response schema version.
    pub schema_version: u16,
}

/// Build an object storage summary payload for ALICE-API.
#[inline]
#[must_use]
pub fn storage_to_api_payload(
    bucket_count: u32,
    total_bytes: u64,
    object_count: u64,
    schema_version: u16,
) -> StorageApiPayload {
    let mut key = [0u8; 22];
    key[0..4].copy_from_slice(&bucket_count.to_le_bytes());
    key[4..12].copy_from_slice(&total_bytes.to_le_bytes());
    key[12..20].copy_from_slice(&object_count.to_le_bytes());
    key[20..22].copy_from_slice(&schema_version.to_le_bytes());
    StorageApiPayload {
        content_hash: fnv1a(&key),
        bucket_count,
        total_bytes,
        object_count,
        schema_version,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const OBJECT_HASH: u64 = 0xBEEF_DEAD_C0DE_CAFE;
    const REGION_HASH: u64 = 0x4455_6677_8899_AABB;

    #[test]
    fn test_storage_to_db_record_hash_nonzero() {
        let rec = storage_to_db_record(10, 1_000_000, 50_000_000_000, 0, REGION_HASH);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_storage_to_db_record_fields() {
        let rec = storage_to_db_record(5, 500_000, 10_000_000_000, 1, REGION_HASH);
        assert_eq!(rec.bucket_count, 5);
        assert_eq!(rec.object_count, 500_000);
        assert_eq!(rec.total_bytes, 10_000_000_000);
        assert_eq!(rec.storage_class, 1);
        assert_eq!(rec.region_hash, REGION_HASH);
    }

    #[test]
    fn test_storage_to_cache_entry_normal_ttl() {
        let entry = storage_to_cache_entry(OBJECT_HASH, 1_024, 1_700_000_000_000);
        assert_eq!(entry.ttl_secs, 3_600);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_storage_to_cache_entry_large_object_ttl() {
        // object_bytes > 100 MiB → reduced TTL = 60 s.
        let entry = storage_to_cache_entry(OBJECT_HASH, 104_857_601, 1_700_000_000_000);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_storage_to_analytics_event_fields() {
        let ev = storage_to_analytics_event(5_000, 2_000, 1_000_000_000, 500, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.read_ops, 5_000);
        assert_eq!(ev.write_ops, 2_000);
        assert_eq!(ev.bytes_transferred, 1_000_000_000);
        assert_eq!(ev.latency_us, 500);
    }

    #[test]
    fn test_storage_to_analytics_event_determinism() {
        let a = storage_to_analytics_event(1, 2, 3, 4, 5);
        let b = storage_to_analytics_event(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_storage_to_monitor_status_healthy() {
        let s = storage_to_monitor_status(10, 45, 0, true, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(s.is_healthy);
        assert_eq!(s.usage_pct, 45);
        assert_eq!(s.error_count, 0);
    }

    #[test]
    fn test_storage_to_api_payload_fields() {
        let payload = storage_to_api_payload(8, 5_000_000_000, 200_000, 1);
        assert_ne!(payload.content_hash, 0);
        assert_eq!(payload.bucket_count, 8);
        assert_eq!(payload.total_bytes, 5_000_000_000);
        assert_eq!(payload.object_count, 200_000);
        assert_eq!(payload.schema_version, 1);
    }

    #[test]
    fn test_storage_to_api_payload_determinism() {
        let a = storage_to_api_payload(3, 100, 10, 2);
        let b = storage_to_api_payload(3, 100, 10, 2);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
