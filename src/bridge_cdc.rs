//! CDC bridges — CDC ↔ DB, Cache, Analytics, Queue, Monitor
//!
//! 5 bridges connecting change data capture streams to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: CDC → DB (change record persistence) ───────────────────────

/// CDC change record for ALICE-DB persistence.
pub struct CdcDbRecord {
    /// Content hash over table + change counts.
    pub content_hash: u64,
    /// FNV-1a hash of the table identifier.
    pub table_hash: u64,
    /// Total number of change events captured.
    pub change_count: u64,
    /// Number of INSERT operations captured.
    pub insert_count: u64,
    /// Number of UPDATE operations captured.
    pub update_count: u64,
    /// Number of DELETE operations captured.
    pub delete_count: u64,
}

/// Serialize a CDC change record for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn cdc_to_db_record(
    table_hash: u64,
    change_count: u64,
    insert_count: u64,
    update_count: u64,
    delete_count: u64,
) -> CdcDbRecord {
    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&table_hash.to_le_bytes());
    key[8..16].copy_from_slice(&change_count.to_le_bytes());
    key[16..24].copy_from_slice(&insert_count.to_le_bytes());
    key[24..32].copy_from_slice(&update_count.to_le_bytes());
    key[32..40].copy_from_slice(&delete_count.to_le_bytes());
    CdcDbRecord {
        content_hash: fnv1a(&key),
        table_hash,
        change_count,
        insert_count,
        update_count,
        delete_count,
    }
}

// ── Bridge 2: CDC → Cache (replication position caching) ─────────────────

/// CDC replication position cache entry for ALICE-Cache.
pub struct CdcCacheEntry {
    /// Content hash over table + position fields.
    pub content_hash: u64,
    /// FNV-1a hash of the table identifier.
    pub table_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Log sequence number of the last processed change.
    pub lsn: u64,
    /// Number of changes since the cached LSN.
    pub change_count: u64,
}

/// Build a CDC replication position cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 10 s when change_count > 10_000 (high-churn table).
#[inline]
#[must_use]
pub fn cdc_to_cache_entry(table_hash: u64, lsn: u64, change_count: u64) -> CdcCacheEntry {
    // Branchless high-churn TTL: 60 s normal, 10 s when change_count > 10K.
    let high_churn = (change_count > 10_000) as u32;
    let ttl_secs = 60_u32 - high_churn * 50_u32;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&table_hash.to_le_bytes());
    key[8..16].copy_from_slice(&lsn.to_le_bytes());
    key[16..24].copy_from_slice(&change_count.to_le_bytes());
    CdcCacheEntry {
        content_hash: fnv1a(&key),
        table_hash,
        ttl_secs,
        lsn,
        change_count,
    }
}

// ── Bridge 3: CDC → Analytics (replication lag metrics) ──────────────────

/// CDC replication analytics event for ALICE-Analytics.
pub struct CdcAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total changes processed in the reporting window.
    pub change_count: u64,
    /// Current replication lag in milliseconds.
    pub lag_ms: u64,
    /// Change throughput in records per second.
    pub throughput_rps: u64,
    /// Number of replication errors in the window.
    pub error_count: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a CDC analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn cdc_to_analytics_event(
    change_count: u64,
    lag_ms: u64,
    throughput_rps: u64,
    error_count: u32,
    timestamp_ms: u64,
) -> CdcAnalyticsEvent {
    let mut key = [0u8; 36];
    key[0..8].copy_from_slice(&change_count.to_le_bytes());
    key[8..16].copy_from_slice(&lag_ms.to_le_bytes());
    key[16..24].copy_from_slice(&throughput_rps.to_le_bytes());
    key[24..28].copy_from_slice(&error_count.to_le_bytes());
    key[28..36].copy_from_slice(&timestamp_ms.to_le_bytes());
    CdcAnalyticsEvent {
        content_hash: fnv1a(&key),
        change_count,
        lag_ms,
        throughput_rps,
        error_count,
        timestamp_ms,
    }
}

// ── Bridge 4: CDC → Queue (change event enqueue) ─────────────────────────

/// CDC change event queue entry for ALICE-Queue.
pub struct CdcQueueEntry {
    /// Content hash over table + operation + LSN.
    pub content_hash: u64,
    /// FNV-1a hash of the table identifier.
    pub table_hash: u64,
    /// DML operation code: 0=INSERT, 1=UPDATE, 2=DELETE, 3=TRUNCATE.
    pub operation: u8,
    /// Log sequence number of this change event.
    pub lsn: u64,
    /// Serialised payload size in bytes.
    pub payload_bytes: u64,
}

/// Build a CDC change event queue entry for ALICE-Queue.
#[inline]
#[must_use]
pub fn cdc_to_queue_entry(
    table_hash: u64,
    operation: u8,
    lsn: u64,
    payload_bytes: u64,
) -> CdcQueueEntry {
    let mut key = [0u8; 25];
    key[0..8].copy_from_slice(&table_hash.to_le_bytes());
    key[8] = operation;
    key[9..17].copy_from_slice(&lsn.to_le_bytes());
    key[17..25].copy_from_slice(&payload_bytes.to_le_bytes());
    CdcQueueEntry {
        content_hash: fnv1a(&key),
        table_hash,
        operation,
        lsn,
        payload_bytes,
    }
}

// ── Bridge 5: CDC → Monitor (replication slot health) ────────────────────

/// CDC replication slot health status for ALICE-Monitor.
pub struct CdcMonitorStatus {
    /// Content hash over slot health fields.
    pub content_hash: u64,
    /// FNV-1a hash of the table identifier.
    pub table_hash: u64,
    /// Current replication lag in milliseconds.
    pub lag_ms: u64,
    /// Whether the replication slot is healthy.
    pub is_healthy: bool,
    /// FNV-1a hash of the replication slot name.
    pub slot_hash: u64,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a CDC replication slot health status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn cdc_to_monitor_status(
    table_hash: u64,
    lag_ms: u64,
    is_healthy: bool,
    slot_hash: u64,
    timestamp_ms: u64,
) -> CdcMonitorStatus {
    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&table_hash.to_le_bytes());
    key[8..16].copy_from_slice(&lag_ms.to_le_bytes());
    key[16] = is_healthy as u8;
    key[17..25].copy_from_slice(&slot_hash.to_le_bytes());
    key[25..33].copy_from_slice(&timestamp_ms.to_le_bytes());
    CdcMonitorStatus {
        content_hash: fnv1a(&key),
        table_hash,
        lag_ms,
        is_healthy,
        slot_hash,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE_HASH: u64 = 0xDEAD_C0DE_CAFE_BABE;
    const SLOT_HASH: u64 = 0x0102_0304_0506_0708;

    #[test]
    fn test_cdc_to_db_record_hash_nonzero() {
        let rec = cdc_to_db_record(TABLE_HASH, 10_000, 5_000, 3_000, 2_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_cdc_to_db_record_fields() {
        let rec = cdc_to_db_record(TABLE_HASH, 100, 60, 30, 10);
        assert_eq!(rec.table_hash, TABLE_HASH);
        assert_eq!(rec.change_count, 100);
        assert_eq!(rec.insert_count, 60);
        assert_eq!(rec.update_count, 30);
        assert_eq!(rec.delete_count, 10);
    }

    #[test]
    fn test_cdc_to_cache_entry_normal_ttl() {
        let entry = cdc_to_cache_entry(TABLE_HASH, 999_999_999, 100);
        assert_eq!(entry.ttl_secs, 60);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_cdc_to_cache_entry_high_churn_ttl() {
        // change_count > 10K → reduced TTL = 10 s.
        let entry = cdc_to_cache_entry(TABLE_HASH, 999_999_999, 10_001);
        assert_eq!(entry.ttl_secs, 10);
    }

    #[test]
    fn test_cdc_to_analytics_event_fields() {
        let ev = cdc_to_analytics_event(50_000, 250, 5_000, 2, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.change_count, 50_000);
        assert_eq!(ev.lag_ms, 250);
        assert_eq!(ev.throughput_rps, 5_000);
        assert_eq!(ev.error_count, 2);
    }

    #[test]
    fn test_cdc_to_analytics_event_determinism() {
        let a = cdc_to_analytics_event(1, 2, 3, 4, 5);
        let b = cdc_to_analytics_event(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_cdc_to_queue_entry_fields() {
        let entry = cdc_to_queue_entry(TABLE_HASH, 1, 123_456_789, 512);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.table_hash, TABLE_HASH);
        assert_eq!(entry.operation, 1);
        assert_eq!(entry.lsn, 123_456_789);
        assert_eq!(entry.payload_bytes, 512);
    }

    #[test]
    fn test_cdc_to_monitor_status_healthy() {
        let s = cdc_to_monitor_status(TABLE_HASH, 50, true, SLOT_HASH, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(s.is_healthy);
        assert_eq!(s.lag_ms, 50);
        assert_eq!(s.slot_hash, SLOT_HASH);
    }

    #[test]
    fn test_cdc_to_monitor_status_determinism() {
        let a = cdc_to_monitor_status(TABLE_HASH, 100, true, SLOT_HASH, 0);
        let b = cdc_to_monitor_status(TABLE_HASH, 100, true, SLOT_HASH, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
