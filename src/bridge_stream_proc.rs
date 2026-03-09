//! StreamProc bridges — StreamProc ↔ DB, Cache, Analytics, Queue, Monitor
//!
//! 5 bridges connecting stream processing pipeline data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: StreamProc → DB (pipeline snapshot persistence) ────────────

/// Stream processing pipeline snapshot record for ALICE-DB persistence.
pub struct StreamProcDbRecord {
    /// Content hash over pipeline snapshot fields.
    pub content_hash: u64,
    /// FNV-1a hash of the pipeline identifier.
    pub pipeline_hash: u64,
    /// Total records processed since pipeline start.
    pub record_count: u64,
    /// Sliding window size in milliseconds.
    pub window_ms: u32,
    /// Number of source partitions consumed.
    pub source_count: u32,
    /// Number of output sinks configured.
    pub sink_count: u32,
}

/// Serialize a stream processing pipeline snapshot for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn stream_proc_to_db_record(
    pipeline_hash: u64,
    record_count: u64,
    window_ms: u32,
    source_count: u32,
    sink_count: u32,
) -> StreamProcDbRecord {
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    key[8..16].copy_from_slice(&record_count.to_le_bytes());
    key[16..20].copy_from_slice(&window_ms.to_le_bytes());
    key[20..24].copy_from_slice(&source_count.to_le_bytes());
    key[24..28].copy_from_slice(&sink_count.to_le_bytes());
    key[28..32].copy_from_slice(&window_ms.to_le_bytes());
    StreamProcDbRecord {
        content_hash: fnv1a(&key),
        pipeline_hash,
        record_count,
        window_ms,
        source_count,
        sink_count,
    }
}

// ── Bridge 2: StreamProc → Cache (checkpoint state caching) ──────────────

/// Stream processing checkpoint state cache entry for ALICE-Cache.
pub struct StreamProcCacheEntry {
    /// Content hash over pipeline + checkpoint fields.
    pub content_hash: u64,
    /// FNV-1a hash of the pipeline identifier.
    pub pipeline_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Serialised operator state size in bytes.
    pub state_bytes: u64,
    /// Checkpoint sequence identifier.
    pub checkpoint_id: u64,
}

/// Build a stream processing checkpoint state cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 30 s when state_bytes > 10 MiB (large state).
#[inline]
#[must_use]
pub fn stream_proc_to_cache_entry(
    pipeline_hash: u64,
    state_bytes: u64,
    checkpoint_id: u64,
) -> StreamProcCacheEntry {
    // Branchless large-state TTL: 300 s normal, 30 s when state_bytes > 10 MiB.
    let large = (state_bytes > 10_485_760) as u32;
    let ttl_secs = 300_u32 - large * 270_u32;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    key[8..16].copy_from_slice(&state_bytes.to_le_bytes());
    key[16..24].copy_from_slice(&checkpoint_id.to_le_bytes());
    StreamProcCacheEntry {
        content_hash: fnv1a(&key),
        pipeline_hash,
        ttl_secs,
        state_bytes,
        checkpoint_id,
    }
}

// ── Bridge 3: StreamProc → Analytics (throughput metrics) ────────────────

/// Stream processing throughput analytics event for ALICE-Analytics.
pub struct StreamProcAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total records processed in the reporting window.
    pub record_count: u64,
    /// Average throughput in records per second.
    pub throughput_rps: u64,
    /// P99 end-to-end processing latency in microseconds.
    pub latency_p99_us: u64,
    /// Current backpressure level as percentage (0–100).
    pub backpressure_pct: u8,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a stream processing analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn stream_proc_to_analytics_event(
    record_count: u64,
    throughput_rps: u64,
    latency_p99_us: u64,
    backpressure_pct: u8,
    timestamp_ms: u64,
) -> StreamProcAnalyticsEvent {
    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&record_count.to_le_bytes());
    key[8..16].copy_from_slice(&throughput_rps.to_le_bytes());
    key[16..24].copy_from_slice(&latency_p99_us.to_le_bytes());
    key[24] = backpressure_pct;
    key[25..33].copy_from_slice(&timestamp_ms.to_le_bytes());
    StreamProcAnalyticsEvent {
        content_hash: fnv1a(&key),
        record_count,
        throughput_rps,
        latency_p99_us,
        backpressure_pct,
        timestamp_ms,
    }
}

// ── Bridge 4: StreamProc → Queue (partition record enqueue) ──────────────

/// Stream processing partition record queue entry for ALICE-Queue.
pub struct StreamProcQueueEntry {
    /// Content hash over pipeline + partition + offset.
    pub content_hash: u64,
    /// FNV-1a hash of the pipeline identifier.
    pub pipeline_hash: u64,
    /// Source partition number.
    pub partition: u32,
    /// Record offset within the partition.
    pub offset: u64,
    /// Serialised record payload size in bytes.
    pub payload_bytes: u64,
}

/// Build a stream processing partition record queue entry for ALICE-Queue.
#[inline]
#[must_use]
pub fn stream_proc_to_queue_entry(
    pipeline_hash: u64,
    partition: u32,
    offset: u64,
    payload_bytes: u64,
) -> StreamProcQueueEntry {
    let mut key = [0u8; 28];
    key[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    key[8..12].copy_from_slice(&partition.to_le_bytes());
    key[12..20].copy_from_slice(&offset.to_le_bytes());
    key[20..28].copy_from_slice(&payload_bytes.to_le_bytes());
    StreamProcQueueEntry {
        content_hash: fnv1a(&key),
        pipeline_hash,
        partition,
        offset,
        payload_bytes,
    }
}

// ── Bridge 5: StreamProc → Monitor (consumer lag status) ─────────────────

/// Stream processing consumer lag status for ALICE-Monitor.
pub struct StreamProcMonitorStatus {
    /// Content hash over pipeline lag status fields.
    pub content_hash: u64,
    /// FNV-1a hash of the pipeline identifier.
    pub pipeline_hash: u64,
    /// Current consumer lag (records behind the head of the stream).
    pub consumer_lag: u64,
    /// Whether the pipeline is currently running.
    pub is_running: bool,
    /// Number of processing errors encountered.
    pub error_count: u32,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a stream processing consumer lag status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn stream_proc_to_monitor_status(
    pipeline_hash: u64,
    consumer_lag: u64,
    is_running: bool,
    error_count: u32,
    timestamp_ms: u64,
) -> StreamProcMonitorStatus {
    let mut key = [0u8; 29];
    key[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    key[8..16].copy_from_slice(&consumer_lag.to_le_bytes());
    key[16] = is_running as u8;
    key[17..21].copy_from_slice(&error_count.to_le_bytes());
    key[21..29].copy_from_slice(&timestamp_ms.to_le_bytes());
    StreamProcMonitorStatus {
        content_hash: fnv1a(&key),
        pipeline_hash,
        consumer_lag,
        is_running,
        error_count,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const PIPELINE_HASH: u64 = 0x9988_7766_5544_3322;

    #[test]
    fn test_stream_proc_to_db_record_hash_nonzero() {
        let rec = stream_proc_to_db_record(PIPELINE_HASH, 1_000_000, 5_000, 4, 2);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_stream_proc_to_db_record_fields() {
        let rec = stream_proc_to_db_record(PIPELINE_HASH, 500_000, 10_000, 3, 1);
        assert_eq!(rec.pipeline_hash, PIPELINE_HASH);
        assert_eq!(rec.record_count, 500_000);
        assert_eq!(rec.window_ms, 10_000);
        assert_eq!(rec.source_count, 3);
        assert_eq!(rec.sink_count, 1);
    }

    #[test]
    fn test_stream_proc_to_cache_entry_normal_ttl() {
        let entry = stream_proc_to_cache_entry(PIPELINE_HASH, 1_000_000, 42);
        assert_eq!(entry.ttl_secs, 300);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_stream_proc_to_cache_entry_large_state_ttl() {
        // state_bytes > 10 MiB → reduced TTL = 30 s.
        let entry = stream_proc_to_cache_entry(PIPELINE_HASH, 10_485_761, 42);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_stream_proc_to_analytics_event_fields() {
        let ev = stream_proc_to_analytics_event(500_000, 10_000, 5_000, 20, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.record_count, 500_000);
        assert_eq!(ev.throughput_rps, 10_000);
        assert_eq!(ev.latency_p99_us, 5_000);
        assert_eq!(ev.backpressure_pct, 20);
    }

    #[test]
    fn test_stream_proc_to_analytics_event_determinism() {
        let a = stream_proc_to_analytics_event(1, 2, 3, 4, 5);
        let b = stream_proc_to_analytics_event(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_stream_proc_to_queue_entry_fields() {
        let entry = stream_proc_to_queue_entry(PIPELINE_HASH, 7, 123_456, 2_048);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.pipeline_hash, PIPELINE_HASH);
        assert_eq!(entry.partition, 7);
        assert_eq!(entry.offset, 123_456);
        assert_eq!(entry.payload_bytes, 2_048);
    }

    #[test]
    fn test_stream_proc_to_monitor_status_running() {
        let s = stream_proc_to_monitor_status(PIPELINE_HASH, 100, true, 0, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(s.is_running);
        assert_eq!(s.consumer_lag, 100);
        assert_eq!(s.error_count, 0);
    }

    #[test]
    fn test_stream_proc_to_monitor_status_determinism() {
        let a = stream_proc_to_monitor_status(PIPELINE_HASH, 50, false, 3, 0);
        let b = stream_proc_to_monitor_status(PIPELINE_HASH, 50, false, 3, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
