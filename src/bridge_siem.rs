//! SIEM bridges — ALICE-SIEM ↔ DB, Cache, Analytics, Monitor, Notify
//!
//! 5 bridges connecting security information and event management to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: SIEM → DB (event storage) ──────────────────────────────────

/// SIEM event storage record for ALICE-DB persistence.
pub struct SiemDbRecord {
    /// Content hash over the event metadata.
    pub content_hash: u64,
    /// Total number of security events stored.
    pub event_count: u64,
    /// Number of unique event sources.
    pub source_count: u32,
    /// Number of detection rules active.
    pub rule_count: u32,
    /// Maximum severity level observed (0-255).
    pub severity_max: u8,
    /// Record timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize SIEM event metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn siem_to_db_record(
    event_count: u64,
    source_count: u32,
    rule_count: u32,
    severity_max: u8,
    timestamp_ms: u64,
) -> SiemDbRecord {
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&event_count.to_le_bytes());
    buf[8..12].copy_from_slice(&source_count.to_le_bytes());
    buf[12..16].copy_from_slice(&rule_count.to_le_bytes());
    buf[16] = severity_max;
    buf[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    SiemDbRecord {
        content_hash: fnv1a(&buf),
        event_count,
        source_count,
        rule_count,
        severity_max,
        timestamp_ms,
    }
}

// ── Bridge 2: SIEM → Cache (rule cache) ──────────────────────────────────

/// SIEM detection rule cache entry for ALICE-Cache.
pub struct SiemCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Hash of the detection rule set.
    pub rule_hash: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Number of events matched by this rule in the cache window.
    pub event_count: u64,
    /// Correlation identifier for chained rules.
    pub correlation_id: u64,
}

/// Build a SIEM detection rule cache entry for ALICE-Cache.
///
/// Rules with recent matches (event_count > 0) get a short TTL (30 s) for
/// rapid re-evaluation; idle rules get 300 s.
#[inline]
#[must_use]
pub fn siem_to_cache_entry(
    rule_hash: u64,
    event_count: u64,
    correlation_id: u64,
) -> SiemCacheEntry {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&rule_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&event_count.to_le_bytes());
    buf[16..24].copy_from_slice(&correlation_id.to_le_bytes());
    let has_events = (event_count > 0) as u32;
    let ttl_secs = 300 - has_events * 270;
    SiemCacheEntry {
        content_hash: fnv1a(&buf),
        rule_hash,
        ttl_secs,
        event_count,
        correlation_id,
    }
}

// ── Bridge 3: SIEM → Analytics (detection event) ─────────────────────────

/// SIEM detection analytics event for ALICE-Analytics.
pub struct SiemAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Total number of security events in this window.
    pub event_count: u64,
    /// Number of alerts generated.
    pub alert_count: u32,
    /// False positive rate in basis points.
    pub false_positive_bps: u16,
    /// Mean time to detect in milliseconds.
    pub mean_time_detect_ms: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a SIEM detection analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn siem_to_analytics_event(
    event_count: u64,
    alert_count: u32,
    false_positive_bps: u16,
    mean_time_detect_ms: u64,
    timestamp_ms: u64,
) -> SiemAnalyticsEvent {
    let mut buf = [0u8; 30];
    buf[0..8].copy_from_slice(&event_count.to_le_bytes());
    buf[8..12].copy_from_slice(&alert_count.to_le_bytes());
    buf[12..14].copy_from_slice(&false_positive_bps.to_le_bytes());
    buf[14..22].copy_from_slice(&mean_time_detect_ms.to_le_bytes());
    buf[22..30].copy_from_slice(&timestamp_ms.to_le_bytes());
    SiemAnalyticsEvent {
        content_hash: fnv1a(&buf),
        event_count,
        alert_count,
        false_positive_bps,
        mean_time_detect_ms,
        timestamp_ms,
    }
}

// ── Bridge 4: SIEM → Monitor (dashboard) ─────────────────────────────────

/// SIEM security operations monitor dashboard for ALICE-Monitor.
pub struct SiemMonitorDashboard {
    /// Content hash over the dashboard state.
    pub content_hash: u64,
    /// Current event ingestion rate (events per second).
    pub event_rate: u64,
    /// Number of active security incidents.
    pub active_incidents: u32,
    /// Severity distribution summary (packed count per tier).
    pub severity_distribution: u32,
    /// Whether all SIEM pipelines are operating within thresholds.
    pub is_healthy: bool,
}

/// Build a SIEM security operations monitor dashboard for ALICE-Monitor.
#[inline]
#[must_use]
pub fn siem_to_monitor_dashboard(
    event_rate: u64,
    active_incidents: u32,
    severity_distribution: u32,
    is_healthy: bool,
) -> SiemMonitorDashboard {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&event_rate.to_le_bytes());
    buf[8..12].copy_from_slice(&active_incidents.to_le_bytes());
    buf[12..16].copy_from_slice(&severity_distribution.to_le_bytes());
    buf[16] = is_healthy as u8;
    SiemMonitorDashboard {
        content_hash: fnv1a(&buf),
        event_rate,
        active_incidents,
        severity_distribution,
        is_healthy,
    }
}

// ── Bridge 5: SIEM → Notify (incident alert) ─────────────────────────────

/// SIEM incident alert notification for ALICE-Notify.
pub struct SiemNotifyAlert {
    /// Content hash over the alert tuple.
    pub content_hash: u64,
    /// Severity level (0=info, 1=warn, 2=critical).
    pub severity: u8,
    /// Number of events that triggered this alert.
    pub event_count: u64,
    /// Hash of the detection rule that fired.
    pub rule_hash: u64,
    /// Hash of the event source identifier.
    pub source_hash: u64,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a SIEM incident alert notification for ALICE-Notify.
#[inline]
#[must_use]
pub fn siem_to_notify_alert(
    severity: u8,
    event_count: u64,
    rule_hash: u64,
    source_hash: u64,
    timestamp_ms: u64,
) -> SiemNotifyAlert {
    let mut buf = [0u8; 33];
    buf[0] = severity;
    buf[1..9].copy_from_slice(&event_count.to_le_bytes());
    buf[9..17].copy_from_slice(&rule_hash.to_le_bytes());
    buf[17..25].copy_from_slice(&source_hash.to_le_bytes());
    buf[25..33].copy_from_slice(&timestamp_ms.to_le_bytes());
    SiemNotifyAlert {
        content_hash: fnv1a(&buf),
        severity,
        event_count,
        rule_hash,
        source_hash,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_siem_to_db_record_hash_nonzero() {
        let rec = siem_to_db_record(100_000, 20, 150, 3, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_siem_to_db_record_fields() {
        let rec = siem_to_db_record(50_000, 10, 80, 2, 999_999);
        assert_eq!(rec.event_count, 50_000);
        assert_eq!(rec.source_count, 10);
        assert_eq!(rec.rule_count, 80);
        assert_eq!(rec.severity_max, 2);
        assert_eq!(rec.timestamp_ms, 999_999);
    }

    #[test]
    fn test_siem_to_db_record_deterministic() {
        let a = siem_to_db_record(1, 2, 3, 4, 5);
        let b = siem_to_db_record(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_siem_to_cache_entry_idle_ttl() {
        let entry = siem_to_cache_entry(0x1234, 0, 0xabcd);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_siem_to_cache_entry_active_ttl() {
        let entry = siem_to_cache_entry(0x5678, 500, 0xef01);
        assert_eq!(entry.ttl_secs, 30);
        assert_eq!(entry.event_count, 500);
    }

    #[test]
    fn test_siem_to_analytics_event() {
        let ev = siem_to_analytics_event(10_000, 25, 200, 1_500, 1_700_000_000_001);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.alert_count, 25);
        assert_eq!(ev.false_positive_bps, 200);
        assert_eq!(ev.mean_time_detect_ms, 1_500);
    }

    #[test]
    fn test_siem_to_monitor_dashboard_healthy() {
        let dash = siem_to_monitor_dashboard(5_000, 0, 0x00_01_00_00, true);
        assert_ne!(dash.content_hash, 0);
        assert!(dash.is_healthy);
        assert_eq!(dash.active_incidents, 0);
    }

    #[test]
    fn test_siem_to_notify_alert() {
        let alert = siem_to_notify_alert(2, 300, 0x1111, 0x2222, 1_700_000_000_002);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.event_count, 300);
        assert_eq!(alert.rule_hash, 0x1111);
    }
}
