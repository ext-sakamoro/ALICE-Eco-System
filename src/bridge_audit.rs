//! Audit bridges — ALICE-Audit ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting audit event streams to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Audit → DB (event persistence) ────────────────────────────

/// Audit event record for ALICE-DB persistence.
pub struct AuditDbRecord {
    /// Content hash over the event payload.
    pub content_hash: u64,
    /// Total audit events persisted in this batch.
    pub event_count: u64,
    /// Length of the audit chain (monotonically increasing).
    pub chain_length: u64,
    /// Hash of the chain integrity proof.
    pub integrity_hash: u64,
    /// Hash of the acting principal (anonymised).
    pub actor_hash: u64,
    /// Severity level (0 = info, 1 = warn, 2 = error, 3 = critical).
    pub severity_level: u8,
}

/// Serialize an audit batch for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn audit_to_db_record(
    event_count: u64,
    chain_length: u64,
    integrity_hash: u64,
    actor_hash: u64,
    severity_level: u8,
) -> AuditDbRecord {
    let mut buf = [0u8; 33];
    buf[0..8].copy_from_slice(&event_count.to_le_bytes());
    buf[8..16].copy_from_slice(&chain_length.to_le_bytes());
    buf[16..24].copy_from_slice(&integrity_hash.to_le_bytes());
    buf[24..32].copy_from_slice(&actor_hash.to_le_bytes());
    buf[32] = severity_level;
    AuditDbRecord {
        content_hash: fnv1a(&buf),
        event_count,
        chain_length,
        integrity_hash,
        actor_hash,
        severity_level,
    }
}

// ── Bridge 2: Audit → Cache (recent events) ─────────────────────────────

/// Recent audit event cache entry for ALICE-Cache.
pub struct AuditCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of events in the cache window.
    pub event_count: u64,
    /// Hash of the most recent event in the window.
    pub latest_event_hash: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Severity level of the most recent event.
    pub severity_level: u8,
}

/// Build a cache entry for the most recent audit window.
///
/// TTL is reduced by 30 s when severity is critical (level >= 3) so
/// high-severity windows expire faster and are re-fetched from DB.
#[inline]
#[must_use]
pub fn audit_to_cache_entry(
    event_count: u64,
    latest_event_hash: u64,
    severity_level: u8,
) -> AuditCacheEntry {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&event_count.to_le_bytes());
    buf[8..16].copy_from_slice(&latest_event_hash.to_le_bytes());
    buf[16] = severity_level;
    let is_critical = (severity_level >= 3) as u32;
    let ttl_secs = 120 - is_critical * 30;
    AuditCacheEntry {
        content_hash: fnv1a(&buf),
        event_count,
        latest_event_hash,
        ttl_secs,
        severity_level,
    }
}

// ── Bridge 3: Audit → Analytics (compliance metrics) ────────────────────

/// Compliance metrics from audit events for ALICE-Analytics ingestion.
pub struct AuditAnalyticsMetrics {
    /// Content hash over the metric tuple.
    pub content_hash: u64,
    /// Total events observed in the reporting period.
    pub event_count: u64,
    /// Number of events flagged as compliance violations.
    pub violation_count: u64,
    /// Violation rate in the range [0.0, 1.0].
    pub violation_rate: f64,
    /// Average chain length across all audit sessions.
    pub avg_chain_length: f64,
    /// Peak severity observed in the reporting period.
    pub peak_severity: u8,
}

/// Build compliance metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn audit_to_analytics_metrics(
    event_count: u64,
    violation_count: u64,
    total_chain_length: u64,
    peak_severity: u8,
) -> AuditAnalyticsMetrics {
    let rcp = 1.0 / event_count.max(1) as f64;
    let violation_rate = violation_count as f64 * rcp;
    let avg_chain_length = total_chain_length as f64 * rcp;
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&event_count.to_le_bytes());
    buf[8..16].copy_from_slice(&violation_count.to_le_bytes());
    buf[16] = peak_severity;
    AuditAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        event_count,
        violation_count,
        violation_rate,
        avg_chain_length,
        peak_severity,
    }
}

// ── Bridge 4: Audit → Monitor (alerts) ──────────────────────────────────

/// Alert payload from audit events for ALICE-Monitor.
pub struct AuditMonitorAlert {
    /// Content hash over the alert payload.
    pub content_hash: u64,
    /// Number of events that triggered this alert.
    pub event_count: u64,
    /// Hash of the chain segment under alert.
    pub chain_segment_hash: u64,
    /// Severity level of the alert.
    pub severity_level: u8,
    /// Whether the alert requires immediate escalation.
    pub requires_escalation: bool,
    /// Number of distinct actors involved.
    pub actor_count: u32,
}

/// Build a monitor alert from an audit anomaly window.
#[inline]
#[must_use]
pub fn audit_to_monitor_alert(
    event_count: u64,
    chain_segment_hash: u64,
    severity_level: u8,
    actor_count: u32,
) -> AuditMonitorAlert {
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&event_count.to_le_bytes());
    buf[8..16].copy_from_slice(&chain_segment_hash.to_le_bytes());
    buf[16] = severity_level;
    buf[17..21].copy_from_slice(&actor_count.to_le_bytes());
    let requires_escalation = severity_level >= 2;
    AuditMonitorAlert {
        content_hash: fnv1a(&buf),
        event_count,
        chain_segment_hash,
        severity_level,
        requires_escalation,
        actor_count,
    }
}

// ── Bridge 5: Audit → API (query interface) ──────────────────────────────

/// Audit query response for ALICE-API.
pub struct AuditApiResponse {
    /// Content hash over the response payload.
    pub content_hash: u64,
    /// Total events matched by the query.
    pub event_count: u64,
    /// Chain length at the time of query.
    pub chain_length: u64,
    /// Integrity hash of the returned chain segment.
    pub integrity_hash: u64,
    /// HTTP status code for the response.
    pub status_code: u16,
    /// Whether the result set was truncated.
    pub truncated: bool,
}

/// Build an API query response from audit chain state.
#[inline]
#[must_use]
pub fn audit_to_api_response(
    event_count: u64,
    chain_length: u64,
    integrity_hash: u64,
    truncated: bool,
) -> AuditApiResponse {
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&event_count.to_le_bytes());
    buf[8..16].copy_from_slice(&chain_length.to_le_bytes());
    buf[16..24].copy_from_slice(&integrity_hash.to_le_bytes());
    buf[24] = truncated as u8;
    let status_code = if truncated { 206 } else { 200 };
    AuditApiResponse {
        content_hash: fnv1a(&buf),
        event_count,
        chain_length,
        integrity_hash,
        status_code,
        truncated,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_to_db_record_hash_nonzero() {
        let rec = audit_to_db_record(100, 50, 0xdeadbeef, 0xcafe, 1);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_audit_to_db_record_fields() {
        let rec = audit_to_db_record(42, 7, 0x1234, 0x5678, 2);
        assert_eq!(rec.event_count, 42);
        assert_eq!(rec.chain_length, 7);
        assert_eq!(rec.integrity_hash, 0x1234);
        assert_eq!(rec.actor_hash, 0x5678);
        assert_eq!(rec.severity_level, 2);
    }

    #[test]
    fn test_audit_to_cache_entry_normal_ttl() {
        let entry = audit_to_cache_entry(10, 0xabcd, 1);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 120);
        assert_eq!(entry.severity_level, 1);
    }

    #[test]
    fn test_audit_to_cache_entry_critical_ttl() {
        let entry = audit_to_cache_entry(5, 0xffff, 3);
        assert_eq!(entry.ttl_secs, 90);
        assert_eq!(entry.severity_level, 3);
    }

    #[test]
    fn test_audit_to_analytics_metrics_rates() {
        let m = audit_to_analytics_metrics(200, 10, 600, 2);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.event_count, 200);
        assert_eq!(m.violation_count, 10);
        assert!((m.violation_rate - 0.05).abs() < 1e-9);
        assert!((m.avg_chain_length - 3.0).abs() < 1e-9);
        assert_eq!(m.peak_severity, 2);
    }

    #[test]
    fn test_audit_to_analytics_metrics_zero_events() {
        let m = audit_to_analytics_metrics(0, 0, 0, 0);
        assert_eq!(m.violation_rate, 0.0);
        assert_eq!(m.avg_chain_length, 0.0);
    }

    #[test]
    fn test_audit_to_monitor_alert_escalation() {
        let alert = audit_to_monitor_alert(3, 0xbeef, 2, 4);
        assert_ne!(alert.content_hash, 0);
        assert!(alert.requires_escalation);
        assert_eq!(alert.actor_count, 4);
    }

    #[test]
    fn test_audit_to_api_response_truncated() {
        let resp = audit_to_api_response(1000, 500, 0x9999, true);
        assert_ne!(resp.content_hash, 0);
        assert_eq!(resp.status_code, 206);
        assert!(resp.truncated);
    }
}
