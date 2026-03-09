//! TimeSeries bridges — TimeSeries ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting time series metric data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: TimeSeries → DB (metric series persistence) ────────────────

/// Time series metric record for ALICE-DB persistence.
pub struct TimeSeriesDbRecord {
    /// Content hash over metric series fields.
    pub content_hash: u64,
    /// FNV-1a hash of the metric name and label set.
    pub metric_hash: u64,
    /// Total number of data points stored for this metric.
    pub data_point_count: u64,
    /// Data retention period in days.
    pub retention_days: u32,
    /// Storage resolution in milliseconds (interval between samples).
    pub resolution_ms: u32,
    /// Number of label tags associated with this metric.
    pub tag_count: u16,
}

/// Serialize a time series metric record for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn time_series_to_db_record(
    metric_hash: u64,
    data_point_count: u64,
    retention_days: u32,
    resolution_ms: u32,
    tag_count: u16,
) -> TimeSeriesDbRecord {
    let mut key = [0u8; 26];
    key[0..8].copy_from_slice(&metric_hash.to_le_bytes());
    key[8..16].copy_from_slice(&data_point_count.to_le_bytes());
    key[16..20].copy_from_slice(&retention_days.to_le_bytes());
    key[20..24].copy_from_slice(&resolution_ms.to_le_bytes());
    key[24..26].copy_from_slice(&tag_count.to_le_bytes());
    TimeSeriesDbRecord {
        content_hash: fnv1a(&key),
        metric_hash,
        data_point_count,
        retention_days,
        resolution_ms,
        tag_count,
    }
}

// ── Bridge 2: TimeSeries → Cache (metric window caching) ─────────────────

/// Time series metric window cache entry for ALICE-Cache.
pub struct TimeSeriesCacheEntry {
    /// Content hash over metric + window fields.
    pub content_hash: u64,
    /// FNV-1a hash of the metric name and label set.
    pub metric_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of data points in the cached window.
    pub data_point_count: u64,
    /// Query window size in milliseconds.
    pub window_ms: u64,
}

/// Build a time series metric window cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 10 s when window_ms < 60_000 (sub-minute window, high churn).
#[inline]
#[must_use]
pub fn time_series_to_cache_entry(
    metric_hash: u64,
    data_point_count: u64,
    window_ms: u64,
) -> TimeSeriesCacheEntry {
    // Branchless sub-minute TTL: 300 s normal, 10 s when window_ms < 60 s.
    let sub_minute = (window_ms < 60_000) as u32;
    let ttl_secs = 300_u32 - sub_minute * 290_u32;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&metric_hash.to_le_bytes());
    key[8..16].copy_from_slice(&data_point_count.to_le_bytes());
    key[16..24].copy_from_slice(&window_ms.to_le_bytes());
    TimeSeriesCacheEntry {
        content_hash: fnv1a(&key),
        metric_hash,
        ttl_secs,
        data_point_count,
        window_ms,
    }
}

// ── Bridge 3: TimeSeries → Analytics (query performance metrics) ──────────

/// Time series query analytics event for ALICE-Analytics.
pub struct TimeSeriesAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total queries executed in the reporting window.
    pub query_count: u64,
    /// Total data points scanned by queries.
    pub scan_points: u64,
    /// Average query execution time in microseconds.
    pub query_time_us: u64,
    /// Cache hit rate in basis points (10_000 = 100.00%).
    pub cache_hit_bps: u16,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a time series analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn time_series_to_analytics_event(
    query_count: u64,
    scan_points: u64,
    query_time_us: u64,
    cache_hit_bps: u16,
    timestamp_ms: u64,
) -> TimeSeriesAnalyticsEvent {
    let mut key = [0u8; 34];
    key[0..8].copy_from_slice(&query_count.to_le_bytes());
    key[8..16].copy_from_slice(&scan_points.to_le_bytes());
    key[16..24].copy_from_slice(&query_time_us.to_le_bytes());
    key[24..26].copy_from_slice(&cache_hit_bps.to_le_bytes());
    key[26..34].copy_from_slice(&timestamp_ms.to_le_bytes());
    TimeSeriesAnalyticsEvent {
        content_hash: fnv1a(&key),
        query_count,
        scan_points,
        query_time_us,
        cache_hit_bps,
        timestamp_ms,
    }
}

// ── Bridge 4: TimeSeries → Monitor (ingestion health status) ─────────────

/// Time series ingestion health status for ALICE-Monitor.
pub struct TimeSeriesMonitorStatus {
    /// Content hash over ingestion health fields.
    pub content_hash: u64,
    /// Total number of active metrics tracked.
    pub metric_count: u64,
    /// Current ingestion rate in data points per second.
    pub ingestion_rate: u64,
    /// Current ingestion lag in milliseconds.
    pub lag_ms: u64,
    /// Whether the ingestion pipeline is considered healthy.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a time series ingestion health status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn time_series_to_monitor_status(
    metric_count: u64,
    ingestion_rate: u64,
    lag_ms: u64,
    is_healthy: bool,
    timestamp_ms: u64,
) -> TimeSeriesMonitorStatus {
    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&metric_count.to_le_bytes());
    key[8..16].copy_from_slice(&ingestion_rate.to_le_bytes());
    key[16..24].copy_from_slice(&lag_ms.to_le_bytes());
    key[24] = is_healthy as u8;
    key[25..33].copy_from_slice(&timestamp_ms.to_le_bytes());
    TimeSeriesMonitorStatus {
        content_hash: fnv1a(&key),
        metric_count,
        ingestion_rate,
        lag_ms,
        is_healthy,
        timestamp_ms,
    }
}

// ── Bridge 5: TimeSeries → API (metric summary payload) ──────────────────

/// Time series metric summary payload for ALICE-API responses.
pub struct TimeSeriesApiPayload {
    /// Content hash over summary fields.
    pub content_hash: u64,
    /// FNV-1a hash of the metric name and label set.
    pub metric_hash: u64,
    /// Total number of data points available.
    pub data_point_count: u64,
    /// Storage resolution in milliseconds.
    pub resolution_ms: u32,
    /// API response schema version.
    pub schema_version: u16,
}

/// Build a time series metric summary payload for ALICE-API.
#[inline]
#[must_use]
pub fn time_series_to_api_payload(
    metric_hash: u64,
    data_point_count: u64,
    resolution_ms: u32,
    schema_version: u16,
) -> TimeSeriesApiPayload {
    let mut key = [0u8; 22];
    key[0..8].copy_from_slice(&metric_hash.to_le_bytes());
    key[8..16].copy_from_slice(&data_point_count.to_le_bytes());
    key[16..20].copy_from_slice(&resolution_ms.to_le_bytes());
    key[20..22].copy_from_slice(&schema_version.to_le_bytes());
    TimeSeriesApiPayload {
        content_hash: fnv1a(&key),
        metric_hash,
        data_point_count,
        resolution_ms,
        schema_version,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const METRIC_HASH: u64 = 0x1357_2468_9ACE_BDF0;

    #[test]
    fn test_time_series_to_db_record_hash_nonzero() {
        let rec = time_series_to_db_record(METRIC_HASH, 1_000_000, 90, 1_000, 8);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_time_series_to_db_record_fields() {
        let rec = time_series_to_db_record(METRIC_HASH, 500_000, 30, 5_000, 4);
        assert_eq!(rec.metric_hash, METRIC_HASH);
        assert_eq!(rec.data_point_count, 500_000);
        assert_eq!(rec.retention_days, 30);
        assert_eq!(rec.resolution_ms, 5_000);
        assert_eq!(rec.tag_count, 4);
    }

    #[test]
    fn test_time_series_to_cache_entry_normal_ttl() {
        // window_ms >= 60_000 → normal TTL = 300 s.
        let entry = time_series_to_cache_entry(METRIC_HASH, 1_000, 60_000);
        assert_eq!(entry.ttl_secs, 300);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_time_series_to_cache_entry_sub_minute_ttl() {
        // window_ms < 60_000 → reduced TTL = 10 s.
        let entry = time_series_to_cache_entry(METRIC_HASH, 100, 59_999);
        assert_eq!(entry.ttl_secs, 10);
    }

    #[test]
    fn test_time_series_to_analytics_event_fields() {
        let ev = time_series_to_analytics_event(2_000, 5_000_000, 800, 8_500, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.query_count, 2_000);
        assert_eq!(ev.scan_points, 5_000_000);
        assert_eq!(ev.query_time_us, 800);
        assert_eq!(ev.cache_hit_bps, 8_500);
    }

    #[test]
    fn test_time_series_to_analytics_event_determinism() {
        let a = time_series_to_analytics_event(1, 2, 3, 4, 5);
        let b = time_series_to_analytics_event(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_time_series_to_monitor_status_healthy() {
        let s = time_series_to_monitor_status(500, 100_000, 50, true, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(s.is_healthy);
        assert_eq!(s.metric_count, 500);
        assert_eq!(s.ingestion_rate, 100_000);
        assert_eq!(s.lag_ms, 50);
    }

    #[test]
    fn test_time_series_to_api_payload_fields() {
        let payload = time_series_to_api_payload(METRIC_HASH, 999_999, 1_000, 3);
        assert_ne!(payload.content_hash, 0);
        assert_eq!(payload.metric_hash, METRIC_HASH);
        assert_eq!(payload.data_point_count, 999_999);
        assert_eq!(payload.resolution_ms, 1_000);
        assert_eq!(payload.schema_version, 3);
    }

    #[test]
    fn test_time_series_to_api_payload_determinism() {
        let a = time_series_to_api_payload(METRIC_HASH, 100, 500, 1);
        let b = time_series_to_api_payload(METRIC_HASH, 100, 500, 1);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
