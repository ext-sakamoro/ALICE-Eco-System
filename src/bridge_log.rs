//! Log bridges — ALICE-Log ↔ Analytics, DB, Cache, Search, Edge
//!
//! 5 bridges connecting the logging layer to the ALICE ecosystem.

use alice_log::{Level, LogEntry, SamplingPolicy, TraceContext};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// ログレベルを u8 に変換（`as u8` キャスト禁止ルール準拠）
#[inline(always)]
fn level_to_u8(level: Level) -> u8 {
    match level {
        Level::Trace => 0,
        Level::Debug => 1,
        Level::Info => 2,
        Level::Warn => 3,
        Level::Error => 4,
    }
}

// ── Bridge 1: Log → Analytics (log metrics) ───────────────────────────────

/// Log throughput metrics event for ALICE-Analytics.
///
/// Emitted per log entry so the analytics layer can compute error rates,
/// trace coverage, and per-module log volume distributions.
pub struct LogAnalyticsEvent {
    /// FNV-1a hash of the log message — analytics stream key.
    pub content_hash: u64,
    /// Log level as u8 (0=Trace … 4=Error).
    pub level: u8,
    /// FNV-1a hash of the module name.
    pub module_hash: u64,
    /// Entry timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Number of structured key-value fields attached.
    pub field_count: u16,
    /// True when a trace ID is present.
    pub has_trace: bool,
}

/// Build a log throughput metrics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn log_to_analytics_event(entry: &LogEntry) -> LogAnalyticsEvent {
    let content_hash = fnv1a(entry.message.as_bytes());
    let module_hash = fnv1a(entry.module.as_bytes());
    LogAnalyticsEvent {
        content_hash,
        level: level_to_u8(entry.level),
        module_hash,
        timestamp_ms: entry.timestamp_ms,
        field_count: entry.fields.len().min(u16::MAX as usize) as u16,
        has_trace: entry.trace_id.is_some(),
    }
}

// ── Bridge 2: Log → DB (log persistence) ──────────────────────────────────

/// Log persistence record for ALICE-DB.
///
/// Written for Warn/Error entries (and sampled Info/Debug) so that
/// post-incident investigations can replay the log trail from the database.
pub struct LogDbRecord {
    /// FNV-1a hash of the message — DB row key.
    pub content_hash: u64,
    /// Log level as u8.
    pub level: u8,
    /// FNV-1a hash of the module name.
    pub module_hash: u64,
    /// Entry timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Trace ID (0 when absent).
    pub trace_id: u64,
    /// Span ID (0 when absent).
    pub span_id: u64,
    /// Message length in bytes.
    pub message_len: usize,
}

/// Build a log persistence record for ALICE-DB.
#[inline]
#[must_use]
pub fn log_to_db_record(entry: &LogEntry) -> LogDbRecord {
    let content_hash = fnv1a(entry.message.as_bytes());
    LogDbRecord {
        content_hash,
        level: level_to_u8(entry.level),
        module_hash: fnv1a(entry.module.as_bytes()),
        timestamp_ms: entry.timestamp_ms,
        trace_id: entry.trace_id.unwrap_or(0),
        span_id: entry.span_id.unwrap_or(0),
        message_len: entry.message.len(),
    }
}

// ── Bridge 3: Log → Cache (recent log cache) ──────────────────────────────

/// Recent log entry cached for ALICE-Cache.
///
/// Hot recent logs (last ~60 s) are cached so that the UI can display
/// a live log stream without hitting the database on every request.
/// TTL is branchlessly extended for Error entries to keep them visible
/// longer than routine Info/Debug lines.
pub struct LogCacheEntry {
    /// FNV-1a hash of the message — cache key.
    pub content_hash: u64,
    /// Log level as u8.
    pub level: u8,
    /// Module name hash.
    pub module_hash: u64,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Cache TTL in seconds (branchless: 300 for Error, 60 for others).
    pub ttl_secs: u32,
    /// Sampling policy approved: true when the entry passed the policy filter.
    pub sampled: bool,
}

/// Build a cached log entry for ALICE-Cache.
///
/// `policy` is used to determine whether the entry should be cached at all.
#[inline]
#[must_use]
pub fn log_to_cache_entry(entry: &LogEntry, policy: &SamplingPolicy) -> LogCacheEntry {
    let content_hash = fnv1a(entry.message.as_bytes());
    let sampled = policy.should_log(entry.level, content_hash);
    let level_u8 = level_to_u8(entry.level);
    // ブランチレスTTL: Error (4) → 300秒、それ以外 → 60秒
    let is_error = (level_u8 >= 4) as u32;
    let ttl_secs = 60 + is_error * 240;
    LogCacheEntry {
        content_hash,
        level: level_u8,
        module_hash: fnv1a(entry.module.as_bytes()),
        timestamp_ms: entry.timestamp_ms,
        ttl_secs,
        sampled,
    }
}

// ── Bridge 4: Log → Search (log indexing) ────────────────────────────────

/// Log index document for ALICE-Search.
///
/// Each log entry is indexed so that operators can perform full-text search
/// over structured fields, trace IDs, and message tokens.
pub struct LogSearchDocument {
    /// FNV-1a hash of the message — primary search key.
    pub content_hash: u64,
    /// FNV-1a hash of all field keys concatenated — schema fingerprint.
    pub schema_hash: u64,
    /// Log level as u8 — used as a range filter.
    pub level: u8,
    /// Timestamp in milliseconds — sort key.
    pub timestamp_ms: u64,
    /// Trace ID (0 when absent) — trace join key.
    pub trace_id: u64,
    /// Total byte size of the entry (message + all field values).
    pub doc_bytes: usize,
}

/// Build a search index document for ALICE-Search.
#[inline]
#[must_use]
pub fn log_to_search_document(entry: &LogEntry) -> LogSearchDocument {
    let content_hash = fnv1a(entry.message.as_bytes());
    // フィールドキーを連結してスキーマフィンガープリント計算
    let schema_bytes: Vec<u8> = entry.fields.iter().flat_map(|(k, _)| k.bytes()).collect();
    let schema_hash = if schema_bytes.is_empty() {
        0
    } else {
        fnv1a(&schema_bytes)
    };
    let doc_bytes = entry.message.len()
        + entry
            .fields
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum::<usize>();
    LogSearchDocument {
        content_hash,
        schema_hash,
        level: level_to_u8(entry.level),
        timestamp_ms: entry.timestamp_ms,
        trace_id: entry.trace_id.unwrap_or(0),
        doc_bytes,
    }
}

// ── Bridge 5: Log → Edge (log forwarding) ────────────────────────────────

/// Forwarded log payload for ALICE-Edge.
///
/// Compact log representation forwarded to edge devices for local alerting
/// and anomaly detection without shipping full structured entries.
pub struct LogEdgeForward {
    /// FNV-1a hash of the message — edge routing key.
    pub content_hash: u64,
    /// Log level as u8.
    pub level: u8,
    /// Trace context: trace ID (0 when absent).
    pub trace_id: u64,
    /// Trace context: span ID (0 when absent).
    pub span_id: u64,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// True when message length exceeds 256 bytes (edge should truncate).
    pub is_large: bool,
}

/// Build a compact log forward payload for ALICE-Edge.
///
/// `ctx` may be `None` when the log entry carries no trace context.
#[inline]
#[must_use]
pub fn log_to_edge_forward(entry: &LogEntry, ctx: Option<&TraceContext>) -> LogEdgeForward {
    let content_hash = fnv1a(entry.message.as_bytes());
    let (trace_id, span_id) = ctx.map_or_else(
        || (entry.trace_id.unwrap_or(0), entry.span_id.unwrap_or(0)),
        |c| (c.trace_id, c.span_id),
    );
    LogEdgeForward {
        content_hash,
        level: level_to_u8(entry.level),
        trace_id,
        span_id,
        timestamp_ms: entry.timestamp_ms,
        is_large: entry.message.len() > 256,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(level: Level, msg: &str) -> LogEntry {
        LogEntry {
            level,
            timestamp_ms: 1_700_000_000_000,
            module: String::from("alice::core"),
            message: String::from(msg),
            fields: vec![(String::from("req_id"), String::from("abc123"))],
            trace_id: Some(42),
            span_id: Some(7),
        }
    }

    // ── Bridge 1 ──────────────────────────────────────────────────────────

    #[test]
    fn test_analytics_event_hash_nonzero() {
        let entry = make_entry(Level::Info, "user login");
        let ev = log_to_analytics_event(&entry);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_analytics_event_fields() {
        let entry = make_entry(Level::Error, "connection refused");
        let ev = log_to_analytics_event(&entry);
        assert_eq!(ev.level, 4); // Error
        assert_ne!(ev.module_hash, 0);
        assert_eq!(ev.timestamp_ms, 1_700_000_000_000);
        assert_eq!(ev.field_count, 1);
        assert!(ev.has_trace);
    }

    #[test]
    fn test_analytics_event_determinism() {
        let entry = make_entry(Level::Warn, "disk full");
        let e1 = log_to_analytics_event(&entry);
        let e2 = log_to_analytics_event(&entry);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 2 ──────────────────────────────────────────────────────────

    #[test]
    fn test_db_record_hash_nonzero() {
        let entry = make_entry(Level::Error, "fatal error");
        let rec = log_to_db_record(&entry);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_db_record_fields() {
        let entry = make_entry(Level::Warn, "slow query");
        let rec = log_to_db_record(&entry);
        assert_eq!(rec.level, 3); // Warn
        assert_eq!(rec.trace_id, 42);
        assert_eq!(rec.span_id, 7);
        assert!(rec.message_len > 0);
    }

    // ── Bridge 3 ──────────────────────────────────────────────────────────

    #[test]
    fn test_cache_entry_ttl_error_extended() {
        let entry = make_entry(Level::Error, "crash");
        let policy = SamplingPolicy::default();
        let cached = log_to_cache_entry(&entry, &policy);
        // Error → TTL = 300
        assert_eq!(cached.ttl_secs, 300);
    }

    #[test]
    fn test_cache_entry_ttl_info_short() {
        let entry = make_entry(Level::Info, "health check ok");
        let policy = SamplingPolicy::default();
        let cached = log_to_cache_entry(&entry, &policy);
        // Info → TTL = 60
        assert_eq!(cached.ttl_secs, 60);
    }

    #[test]
    fn test_cache_entry_hash_nonzero() {
        let entry = make_entry(Level::Debug, "debug message");
        let policy = SamplingPolicy::default();
        let cached = log_to_cache_entry(&entry, &policy);
        assert_ne!(cached.content_hash, 0);
    }

    // ── Bridge 4 ──────────────────────────────────────────────────────────

    #[test]
    fn test_search_document_hash_nonzero() {
        let entry = make_entry(Level::Info, "search index test");
        let doc = log_to_search_document(&entry);
        assert_ne!(doc.content_hash, 0);
    }

    #[test]
    fn test_search_document_schema_hash() {
        let entry = make_entry(Level::Info, "test");
        let doc = log_to_search_document(&entry);
        // フィールドがあるのでスキーマハッシュ非ゼロ
        assert_ne!(doc.schema_hash, 0);
        assert_eq!(doc.trace_id, 42);
        assert!(doc.doc_bytes > 0);
    }

    // ── Bridge 5 ──────────────────────────────────────────────────────────

    #[test]
    fn test_edge_forward_hash_nonzero() {
        let entry = make_entry(Level::Warn, "edge forward");
        let fwd = log_to_edge_forward(&entry, None);
        assert_ne!(fwd.content_hash, 0);
    }

    #[test]
    fn test_edge_forward_with_trace_context() {
        let entry = make_entry(Level::Error, "edge error");
        let ctx = TraceContext::new();
        let fwd = log_to_edge_forward(&entry, Some(&ctx));
        assert_eq!(fwd.trace_id, ctx.trace_id);
        assert_eq!(fwd.span_id, ctx.span_id);
        assert!(!fwd.is_large);
    }

    #[test]
    fn test_edge_forward_determinism() {
        let entry = make_entry(Level::Info, "determinism check");
        let f1 = log_to_edge_forward(&entry, None);
        let f2 = log_to_edge_forward(&entry, None);
        assert_eq!(f1.content_hash, f2.content_hash);
    }
}
