//! Lakehouse bridges — Lakehouse ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting data lakehouse metadata to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Lakehouse → DB (table catalog persistence) ─────────────────

/// Lakehouse table catalog record for ALICE-DB persistence.
pub struct LakehouseDbRecord {
    /// Content hash over catalog fields.
    pub content_hash: u64,
    /// Number of tables in the lakehouse.
    pub table_count: u32,
    /// Total number of data partitions across all tables.
    pub partition_count: u64,
    /// Total storage consumed in bytes.
    pub total_bytes: u64,
    /// FNV-1a hash of the storage format identifier (e.g., parquet, delta).
    pub format_hash: u64,
    /// Catalog schema version number.
    pub version: u32,
}

/// Serialize a lakehouse table catalog record for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn lakehouse_to_db_record(
    table_count: u32,
    partition_count: u64,
    total_bytes: u64,
    format_hash: u64,
    version: u32,
) -> LakehouseDbRecord {
    let mut key = [0u8; 36];
    key[0..4].copy_from_slice(&table_count.to_le_bytes());
    key[4..12].copy_from_slice(&partition_count.to_le_bytes());
    key[12..20].copy_from_slice(&total_bytes.to_le_bytes());
    key[20..28].copy_from_slice(&format_hash.to_le_bytes());
    key[28..32].copy_from_slice(&version.to_le_bytes());
    key[32..36].copy_from_slice(&table_count.to_le_bytes());
    LakehouseDbRecord {
        content_hash: fnv1a(&key),
        table_count,
        partition_count,
        total_bytes,
        format_hash,
        version,
    }
}

// ── Bridge 2: Lakehouse → Cache (table metadata caching) ─────────────────

/// Lakehouse table metadata cache entry for ALICE-Cache.
pub struct LakehouseCacheEntry {
    /// Content hash over table + partition fields.
    pub content_hash: u64,
    /// FNV-1a hash of the table identifier.
    pub table_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of rows in the cached table scan result.
    pub row_count: u64,
    /// Number of partitions in the cached table.
    pub partition_count: u64,
}

/// Build a lakehouse table metadata cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 60 s when partition_count > 10_000 (highly partitioned table).
#[inline]
#[must_use]
pub fn lakehouse_to_cache_entry(
    table_hash: u64,
    row_count: u64,
    partition_count: u64,
) -> LakehouseCacheEntry {
    // Branchless high-partition TTL: 600 s normal, 60 s when partition_count > 10K.
    let high_part = (partition_count > 10_000) as u32;
    let ttl_secs = 600_u32 - high_part * 540_u32;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&table_hash.to_le_bytes());
    key[8..16].copy_from_slice(&row_count.to_le_bytes());
    key[16..24].copy_from_slice(&partition_count.to_le_bytes());
    LakehouseCacheEntry {
        content_hash: fnv1a(&key),
        table_hash,
        ttl_secs,
        row_count,
        partition_count,
    }
}

// ── Bridge 3: Lakehouse → Analytics (query performance metrics) ───────────

/// Lakehouse query analytics event for ALICE-Analytics.
pub struct LakehouseAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total queries executed in the reporting window.
    pub query_count: u64,
    /// Total bytes scanned by queries in the window.
    pub scan_bytes: u64,
    /// Average query execution time in milliseconds.
    pub query_time_ms: u64,
    /// Cache hit rate in basis points (10_000 = 100.00%).
    pub cache_hit_bps: u16,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a lakehouse analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn lakehouse_to_analytics_event(
    query_count: u64,
    scan_bytes: u64,
    query_time_ms: u64,
    cache_hit_bps: u16,
    timestamp_ms: u64,
) -> LakehouseAnalyticsEvent {
    let mut key = [0u8; 34];
    key[0..8].copy_from_slice(&query_count.to_le_bytes());
    key[8..16].copy_from_slice(&scan_bytes.to_le_bytes());
    key[16..24].copy_from_slice(&query_time_ms.to_le_bytes());
    key[24..26].copy_from_slice(&cache_hit_bps.to_le_bytes());
    key[26..34].copy_from_slice(&timestamp_ms.to_le_bytes());
    LakehouseAnalyticsEvent {
        content_hash: fnv1a(&key),
        query_count,
        scan_bytes,
        query_time_ms,
        cache_hit_bps,
        timestamp_ms,
    }
}

// ── Bridge 4: Lakehouse → Monitor (storage health status) ────────────────

/// Lakehouse storage health status for ALICE-Monitor.
pub struct LakehouseMonitorStatus {
    /// Content hash over health metrics.
    pub content_hash: u64,
    /// Total number of tables in the lakehouse.
    pub table_count: u32,
    /// Number of tables with pending compaction.
    pub compaction_pending: u32,
    /// Total storage consumed in bytes.
    pub storage_bytes: u64,
    /// Whether the lakehouse is considered healthy.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a lakehouse storage health status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn lakehouse_to_monitor_status(
    table_count: u32,
    compaction_pending: u32,
    storage_bytes: u64,
    is_healthy: bool,
    timestamp_ms: u64,
) -> LakehouseMonitorStatus {
    let mut key = [0u8; 25];
    key[0..4].copy_from_slice(&table_count.to_le_bytes());
    key[4..8].copy_from_slice(&compaction_pending.to_le_bytes());
    key[8..16].copy_from_slice(&storage_bytes.to_le_bytes());
    key[16] = is_healthy as u8;
    key[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    LakehouseMonitorStatus {
        content_hash: fnv1a(&key),
        table_count,
        compaction_pending,
        storage_bytes,
        is_healthy,
        timestamp_ms,
    }
}

// ── Bridge 5: Lakehouse → API (catalog summary payload) ──────────────────

/// Lakehouse catalog summary payload for ALICE-API responses.
pub struct LakehouseApiPayload {
    /// Content hash over summary fields.
    pub content_hash: u64,
    /// Number of tables in the lakehouse.
    pub table_count: u32,
    /// Total storage consumed in bytes.
    pub total_bytes: u64,
    /// API response schema version.
    pub schema_version: u16,
    /// FNV-1a hash of the tenant identifier.
    pub tenant_hash: u64,
}

/// Build a lakehouse catalog summary payload for ALICE-API.
#[inline]
#[must_use]
pub fn lakehouse_to_api_payload(
    table_count: u32,
    total_bytes: u64,
    schema_version: u16,
    tenant_hash: u64,
) -> LakehouseApiPayload {
    let mut key = [0u8; 22];
    key[0..4].copy_from_slice(&table_count.to_le_bytes());
    key[4..12].copy_from_slice(&total_bytes.to_le_bytes());
    key[12..14].copy_from_slice(&schema_version.to_le_bytes());
    key[14..22].copy_from_slice(&tenant_hash.to_le_bytes());
    LakehouseApiPayload {
        content_hash: fnv1a(&key),
        table_count,
        total_bytes,
        schema_version,
        tenant_hash,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE_HASH: u64 = 0xFACE_FEED_DEAD_BEEF;
    const FORMAT_HASH: u64 = 0x1234_5678_9ABC_DEF0;
    const TENANT_HASH: u64 = 0xABCD_EF01_2345_6789;

    #[test]
    fn test_lakehouse_to_db_record_hash_nonzero() {
        let rec = lakehouse_to_db_record(50, 10_000, 5_000_000_000, FORMAT_HASH, 3);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_lakehouse_to_db_record_fields() {
        let rec = lakehouse_to_db_record(20, 2_000, 1_000_000_000, FORMAT_HASH, 1);
        assert_eq!(rec.table_count, 20);
        assert_eq!(rec.partition_count, 2_000);
        assert_eq!(rec.total_bytes, 1_000_000_000);
        assert_eq!(rec.format_hash, FORMAT_HASH);
        assert_eq!(rec.version, 1);
    }

    #[test]
    fn test_lakehouse_to_cache_entry_normal_ttl() {
        let entry = lakehouse_to_cache_entry(TABLE_HASH, 1_000_000, 500);
        assert_eq!(entry.ttl_secs, 600);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_lakehouse_to_cache_entry_high_partition_ttl() {
        // partition_count > 10K → reduced TTL = 60 s.
        let entry = lakehouse_to_cache_entry(TABLE_HASH, 1_000_000, 10_001);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_lakehouse_to_analytics_event_fields() {
        let ev = lakehouse_to_analytics_event(1_000, 500_000_000, 250, 7_500, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.query_count, 1_000);
        assert_eq!(ev.scan_bytes, 500_000_000);
        assert_eq!(ev.query_time_ms, 250);
        assert_eq!(ev.cache_hit_bps, 7_500);
    }

    #[test]
    fn test_lakehouse_to_analytics_event_determinism() {
        let a = lakehouse_to_analytics_event(1, 2, 3, 4, 5);
        let b = lakehouse_to_analytics_event(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_lakehouse_to_monitor_status_healthy() {
        let s = lakehouse_to_monitor_status(50, 0, 1_000_000_000, true, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(s.is_healthy);
        assert_eq!(s.compaction_pending, 0);
    }

    #[test]
    fn test_lakehouse_to_api_payload_fields() {
        let payload = lakehouse_to_api_payload(30, 2_000_000_000, 2, TENANT_HASH);
        assert_ne!(payload.content_hash, 0);
        assert_eq!(payload.table_count, 30);
        assert_eq!(payload.total_bytes, 2_000_000_000);
        assert_eq!(payload.schema_version, 2);
        assert_eq!(payload.tenant_hash, TENANT_HASH);
    }

    #[test]
    fn test_lakehouse_to_api_payload_determinism() {
        let a = lakehouse_to_api_payload(10, 100, 1, TENANT_HASH);
        let b = lakehouse_to_api_payload(10, 100, 1, TENANT_HASH);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
