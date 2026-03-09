//! ETL bridges — ETL ↔ DB, Cache, Analytics, Monitor, Notify
//!
//! 5 bridges connecting ETL pipeline data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: ETL → DB (pipeline run persistence) ────────────────────────

/// ETL pipeline run record for ALICE-DB persistence.
pub struct EtlDbRecord {
    /// Content hash over pipeline run fields.
    pub content_hash: u64,
    /// FNV-1a hash of the pipeline identifier.
    pub pipeline_hash: u64,
    /// Total records processed in this run.
    pub record_count: u64,
    /// Number of transformation steps applied.
    pub transform_count: u32,
    /// FNV-1a hash of the data source identifier.
    pub source_hash: u64,
    /// Run timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize an ETL pipeline run record for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn etl_to_db_record(
    pipeline_hash: u64,
    record_count: u64,
    transform_count: u32,
    source_hash: u64,
    timestamp_ms: u64,
) -> EtlDbRecord {
    let mut key = [0u8; 36];
    key[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    key[8..16].copy_from_slice(&record_count.to_le_bytes());
    key[16..20].copy_from_slice(&transform_count.to_le_bytes());
    key[20..28].copy_from_slice(&source_hash.to_le_bytes());
    key[28..36].copy_from_slice(&timestamp_ms.to_le_bytes());
    EtlDbRecord {
        content_hash: fnv1a(&key),
        pipeline_hash,
        record_count,
        transform_count,
        source_hash,
        timestamp_ms,
    }
}

// ── Bridge 2: ETL → Cache (pipeline state caching) ───────────────────────

/// ETL pipeline state cache entry for ALICE-Cache.
pub struct EtlCacheEntry {
    /// Content hash over pipeline + state fields.
    pub content_hash: u64,
    /// FNV-1a hash of the pipeline identifier.
    pub pipeline_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of records in the cached state.
    pub record_count: u64,
    /// Number of pipeline stages in the cached run.
    pub stage_count: u32,
}

/// Build an ETL pipeline state cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 30 s when record_count > 1_000_000 (bulk run).
#[inline]
#[must_use]
pub fn etl_to_cache_entry(
    pipeline_hash: u64,
    record_count: u64,
    stage_count: u32,
) -> EtlCacheEntry {
    // Branchless bulk-run TTL: 300 s normal, 30 s when record_count > 1M.
    let bulk = (record_count > 1_000_000) as u32;
    let ttl_secs = 300_u32 - bulk * 270_u32;
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    key[8..16].copy_from_slice(&record_count.to_le_bytes());
    key[16..20].copy_from_slice(&stage_count.to_le_bytes());
    EtlCacheEntry {
        content_hash: fnv1a(&key),
        pipeline_hash,
        ttl_secs,
        record_count,
        stage_count,
    }
}

// ── Bridge 3: ETL → Analytics (throughput metrics) ───────────────────────

/// ETL throughput analytics event for ALICE-Analytics.
pub struct EtlAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total records processed in the reporting window.
    pub record_count: u64,
    /// Total errors encountered.
    pub error_count: u64,
    /// Average throughput in records per second.
    pub throughput_rps: u64,
    /// Total pipeline run duration in milliseconds.
    pub duration_ms: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an ETL analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn etl_to_analytics_event(
    record_count: u64,
    error_count: u64,
    throughput_rps: u64,
    duration_ms: u64,
    timestamp_ms: u64,
) -> EtlAnalyticsEvent {
    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&record_count.to_le_bytes());
    key[8..16].copy_from_slice(&error_count.to_le_bytes());
    key[16..24].copy_from_slice(&throughput_rps.to_le_bytes());
    key[24..32].copy_from_slice(&duration_ms.to_le_bytes());
    key[32..40].copy_from_slice(&timestamp_ms.to_le_bytes());
    EtlAnalyticsEvent {
        content_hash: fnv1a(&key),
        record_count,
        error_count,
        throughput_rps,
        duration_ms,
        timestamp_ms,
    }
}

// ── Bridge 4: ETL → Monitor (pipeline run status) ────────────────────────

/// ETL pipeline run status for ALICE-Monitor.
pub struct EtlMonitorStatus {
    /// Content hash over pipeline run status fields.
    pub content_hash: u64,
    /// FNV-1a hash of the pipeline identifier.
    pub pipeline_hash: u64,
    /// Number of pipeline stages.
    pub stage_count: u32,
    /// Whether the pipeline is currently running.
    pub is_running: bool,
    /// Current pipeline progress percentage (0–100).
    pub progress_pct: u8,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an ETL pipeline run status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn etl_to_monitor_status(
    pipeline_hash: u64,
    stage_count: u32,
    is_running: bool,
    progress_pct: u8,
    timestamp_ms: u64,
) -> EtlMonitorStatus {
    let mut key = [0u8; 22];
    key[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    key[8..12].copy_from_slice(&stage_count.to_le_bytes());
    key[12] = is_running as u8;
    key[13] = progress_pct;
    key[14..22].copy_from_slice(&timestamp_ms.to_le_bytes());
    EtlMonitorStatus {
        content_hash: fnv1a(&key),
        pipeline_hash,
        stage_count,
        is_running,
        progress_pct,
        timestamp_ms,
    }
}

// ── Bridge 5: ETL → Notify (pipeline alert) ──────────────────────────────

/// ETL pipeline alert payload for ALICE-Notify.
pub struct EtlNotifyAlert {
    /// Content hash over severity + pipeline + error + timestamp.
    pub content_hash: u64,
    /// Severity level: 0=info, 1=warning, 2=critical.
    pub severity: u8,
    /// FNV-1a hash of the pipeline identifier.
    pub pipeline_hash: u64,
    /// Number of errors that triggered the alert.
    pub error_count: u64,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an ETL pipeline alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn etl_to_notify_alert(
    severity: u8,
    pipeline_hash: u64,
    error_count: u64,
    timestamp_ms: u64,
) -> EtlNotifyAlert {
    let mut key = [0u8; 25];
    key[0] = severity;
    key[1..9].copy_from_slice(&pipeline_hash.to_le_bytes());
    key[9..17].copy_from_slice(&error_count.to_le_bytes());
    key[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    EtlNotifyAlert {
        content_hash: fnv1a(&key),
        severity,
        pipeline_hash,
        error_count,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const PIPELINE_HASH: u64 = 0x1234_ABCD_5678_EF90;
    const SOURCE_HASH: u64 = 0xFEDC_BA98_7654_3210;

    #[test]
    fn test_etl_to_db_record_hash_nonzero() {
        let rec = etl_to_db_record(PIPELINE_HASH, 500_000, 5, SOURCE_HASH, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_etl_to_db_record_fields() {
        let rec = etl_to_db_record(PIPELINE_HASH, 100_000, 3, SOURCE_HASH, 1_700_000_000_000);
        assert_eq!(rec.pipeline_hash, PIPELINE_HASH);
        assert_eq!(rec.record_count, 100_000);
        assert_eq!(rec.transform_count, 3);
        assert_eq!(rec.source_hash, SOURCE_HASH);
        assert_eq!(rec.timestamp_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_etl_to_cache_entry_normal_ttl() {
        let entry = etl_to_cache_entry(PIPELINE_HASH, 1_000, 4);
        assert_eq!(entry.ttl_secs, 300);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_etl_to_cache_entry_bulk_ttl() {
        // record_count > 1M → reduced TTL = 30 s.
        let entry = etl_to_cache_entry(PIPELINE_HASH, 1_000_001, 4);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_etl_to_analytics_event_fields() {
        let ev = etl_to_analytics_event(2_000_000, 50, 10_000, 200_000, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.record_count, 2_000_000);
        assert_eq!(ev.error_count, 50);
        assert_eq!(ev.throughput_rps, 10_000);
        assert_eq!(ev.duration_ms, 200_000);
    }

    #[test]
    fn test_etl_to_analytics_event_determinism() {
        let a = etl_to_analytics_event(1, 0, 100, 1_000, 0);
        let b = etl_to_analytics_event(1, 0, 100, 1_000, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_etl_to_monitor_status_running() {
        let s = etl_to_monitor_status(PIPELINE_HASH, 6, true, 42, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(s.is_running);
        assert_eq!(s.progress_pct, 42);
    }

    #[test]
    fn test_etl_to_notify_alert_fields() {
        let alert = etl_to_notify_alert(2, PIPELINE_HASH, 1_000, 1_700_000_000_000);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.pipeline_hash, PIPELINE_HASH);
        assert_eq!(alert.error_count, 1_000);
    }

    #[test]
    fn test_etl_to_notify_alert_determinism() {
        let a = etl_to_notify_alert(1, PIPELINE_HASH, 5, 42);
        let b = etl_to_notify_alert(1, PIPELINE_HASH, 5, 42);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
