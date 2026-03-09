//! Monitor bridges — ALICE-Monitor ↔ DB, Cache, Analytics, Notify, API
//!
//! 5 bridges connecting health-check monitoring to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Monitor → DB (incident log) ────────────────────────────────

/// Incident log record for ALICE-DB persistence.
pub struct MonitorDbRecord {
    /// Content hash over the incident fields.
    pub content_hash: u64,
    /// Total health checks performed.
    pub check_count: u64,
    /// Number of checks that returned healthy.
    pub healthy_count: u64,
    /// Number of checks that returned unhealthy.
    pub unhealthy_count: u64,
    /// Number of incidents opened in the period.
    pub incident_count: u32,
    /// Timestamp of the last recorded incident (Unix seconds).
    pub last_incident_ts: u64,
    /// SLA target as a fraction (e.g. 0.999 for 99.9%).
    pub sla_target: f64,
}

/// Serialize monitor incident data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn monitor_to_db_record(
    check_count: u64,
    healthy_count: u64,
    unhealthy_count: u64,
    incident_count: u32,
    last_incident_ts: u64,
    sla_target: f64,
) -> MonitorDbRecord {
    let mut key = [0u8; 28];
    key[0..8].copy_from_slice(&check_count.to_le_bytes());
    key[8..16].copy_from_slice(&healthy_count.to_le_bytes());
    key[16..20].copy_from_slice(&incident_count.to_le_bytes());
    key[20..28].copy_from_slice(&last_incident_ts.to_le_bytes());
    MonitorDbRecord {
        content_hash: fnv1a(&key),
        check_count,
        healthy_count,
        unhealthy_count,
        incident_count,
        last_incident_ts,
        sla_target,
    }
}

// ── Bridge 2: Monitor → Cache (health cache) ─────────────────────────────

/// Health status cache entry for ALICE-Cache.
pub struct MonitorCacheEntry {
    /// Content hash for cache key derivation.
    pub content_hash: u64,
    /// Uptime percentage over the observation window (0.0–100.0).
    pub uptime_pct: f64,
    /// Number of healthy checks.
    pub healthy_count: u64,
    /// Number of unhealthy checks.
    pub unhealthy_count: u64,
    /// TTL in seconds (branchless: shorter TTL when unhealthy checks are present).
    pub ttl_secs: u32,
}

/// Cache health status for ALICE-Cache.
#[inline]
#[must_use]
pub fn monitor_to_cache_entry(
    check_count: u64,
    healthy_count: u64,
    unhealthy_count: u64,
) -> MonitorCacheEntry {
    let rcp = 1.0 / check_count.max(1) as f64;
    let uptime_pct = healthy_count as f64 * rcp * 100.0;

    // Branchless TTL: 60 s normally, 10 s when any unhealthy checks exist.
    let has_unhealthy = (unhealthy_count > 0) as u32;
    let ttl_secs = 60_u32 - has_unhealthy * 50;

    let data = healthy_count.to_le_bytes();
    MonitorCacheEntry {
        content_hash: fnv1a(&data),
        uptime_pct,
        healthy_count,
        unhealthy_count,
        ttl_secs,
    }
}

// ── Bridge 3: Monitor → Analytics (uptime metrics) ───────────────────────

/// Uptime metrics for ALICE-Analytics ingestion.
pub struct MonitorAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total health checks performed.
    pub check_count: u64,
    /// Uptime percentage (0.0–100.0).
    pub uptime_pct: f64,
    /// Mean time between incidents in seconds.
    pub mtbi_secs: f64,
    /// Number of incidents.
    pub incident_count: u32,
    /// SLA target as a fraction.
    pub sla_target: f64,
    /// SLA compliance: 1 if uptime_pct/100 >= sla_target, 0 otherwise.
    pub sla_met: u8,
}

/// Build uptime metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn monitor_to_analytics_metrics(
    check_count: u64,
    healthy_count: u64,
    incident_count: u32,
    observation_secs: f64,
    sla_target: f64,
) -> MonitorAnalyticsMetrics {
    let rcp_checks = 1.0 / check_count.max(1) as f64;
    let uptime_pct = healthy_count as f64 * rcp_checks * 100.0;

    let rcp_incidents = 1.0 / incident_count.max(1) as f64;
    let mtbi_secs = observation_secs * rcp_incidents;

    // Branchless SLA compliance flag.
    let sla_met = (uptime_pct * 0.01 >= sla_target) as u8;

    let mut key = [0u8; 12];
    key[0..8].copy_from_slice(&check_count.to_le_bytes());
    key[8..12].copy_from_slice(&incident_count.to_le_bytes());
    MonitorAnalyticsMetrics {
        content_hash: fnv1a(&key),
        check_count,
        uptime_pct,
        mtbi_secs,
        incident_count,
        sla_target,
        sla_met,
    }
}

// ── Bridge 4: Monitor → Notify (alerts) ──────────────────────────────────

/// Alert payload for ALICE-Notify dispatch.
pub struct MonitorNotifyAlert {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Alert severity level: 0 = info, 1 = warning, 2 = critical.
    pub severity: u8,
    /// Number of consecutive unhealthy checks that triggered this alert.
    pub consecutive_failures: u32,
    /// Uptime percentage at alert time.
    pub uptime_pct: f64,
    /// Incident count at alert time.
    pub incident_count: u32,
    /// Unix timestamp of the alert (seconds).
    pub alert_ts: u64,
}

/// Build alert payload for ALICE-Notify dispatch.
#[inline]
#[must_use]
pub fn monitor_to_notify_alert(
    consecutive_failures: u32,
    uptime_pct: f64,
    incident_count: u32,
    alert_ts: u64,
) -> MonitorNotifyAlert {
    // Severity: 0 = info (<3 failures), 1 = warning (3–9), 2 = critical (10+).
    let severity = match consecutive_failures {
        0..=2 => 0u8,
        3..=9 => 1u8,
        _ => 2u8,
    };

    let mut key = [0u8; 16];
    key[0..4].copy_from_slice(&consecutive_failures.to_le_bytes());
    key[4..8].copy_from_slice(&incident_count.to_le_bytes());
    key[8..16].copy_from_slice(&alert_ts.to_le_bytes());
    MonitorNotifyAlert {
        content_hash: fnv1a(&key),
        severity,
        consecutive_failures,
        uptime_pct,
        incident_count,
        alert_ts,
    }
}

// ── Bridge 5: Monitor → API (status page) ────────────────────────────────

/// Status page payload for ALICE-API exposure.
pub struct MonitorApiStatus {
    /// Content hash for ETag generation.
    pub content_hash: u64,
    /// Uptime percentage (0.0–100.0).
    pub uptime_pct: f64,
    /// Total check count.
    pub check_count: u64,
    /// Healthy check count.
    pub healthy_count: u64,
    /// Unhealthy check count.
    pub unhealthy_count: u64,
    /// Open incident count.
    pub incident_count: u32,
    /// SLA target as a fraction.
    pub sla_target: f64,
    /// Overall status: 0 = operational, 1 = degraded, 2 = outage.
    pub status_code: u8,
}

/// Build status page payload for ALICE-API.
#[inline]
#[must_use]
pub fn monitor_to_api_status(
    check_count: u64,
    healthy_count: u64,
    unhealthy_count: u64,
    incident_count: u32,
    sla_target: f64,
) -> MonitorApiStatus {
    let rcp = 1.0 / check_count.max(1) as f64;
    let uptime_pct = healthy_count as f64 * rcp * 100.0;

    let status_code = if unhealthy_count == 0 {
        0u8
    } else if uptime_pct * 0.01 >= sla_target {
        1u8
    } else {
        2u8
    };

    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&check_count.to_le_bytes());
    key[8..16].copy_from_slice(&healthy_count.to_le_bytes());
    key[16..20].copy_from_slice(&incident_count.to_le_bytes());
    MonitorApiStatus {
        content_hash: fnv1a(&key),
        uptime_pct,
        check_count,
        healthy_count,
        unhealthy_count,
        incident_count,
        sla_target,
        status_code,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_to_db_record_hash_nonzero() {
        let rec = monitor_to_db_record(100, 98, 2, 1, 1_700_000_000, 0.999);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.check_count, 100);
        assert_eq!(rec.incident_count, 1);
    }

    #[test]
    fn test_monitor_to_db_record_counts() {
        let rec = monitor_to_db_record(50, 50, 0, 0, 0, 1.0);
        assert_eq!(rec.healthy_count, 50);
        assert_eq!(rec.unhealthy_count, 0);
        assert_eq!(rec.incident_count, 0);
    }

    #[test]
    fn test_monitor_to_cache_entry_uptime() {
        let entry = monitor_to_cache_entry(1000, 990, 10);
        assert_ne!(entry.content_hash, 0);
        assert!((entry.uptime_pct - 99.0).abs() < 0.01);
        assert_eq!(entry.ttl_secs, 10); // unhealthy present → short TTL
    }

    #[test]
    fn test_monitor_to_cache_entry_full_healthy() {
        let entry = monitor_to_cache_entry(100, 100, 0);
        assert_eq!(entry.ttl_secs, 60); // no unhealthy → long TTL
        assert!((entry.uptime_pct - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_monitor_to_analytics_sla_met() {
        let m = monitor_to_analytics_metrics(1000, 999, 2, 86400.0, 0.999);
        assert_ne!(m.content_hash, 0);
        // uptime = 99.9% → sla_target 0.999 → sla_met = 1
        assert_eq!(m.sla_met, 1);
        assert!((m.mtbi_secs - 43200.0).abs() < 0.1);
    }

    #[test]
    fn test_monitor_to_analytics_sla_not_met() {
        let m = monitor_to_analytics_metrics(1000, 900, 5, 3600.0, 0.999);
        assert_eq!(m.sla_met, 0);
    }

    #[test]
    fn test_monitor_to_notify_alert_severity() {
        let a_info = monitor_to_notify_alert(1, 99.5, 0, 1_700_000_000);
        assert_eq!(a_info.severity, 0);

        let a_warn = monitor_to_notify_alert(5, 95.0, 1, 1_700_000_001);
        assert_eq!(a_warn.severity, 1);

        let a_crit = monitor_to_notify_alert(15, 60.0, 3, 1_700_000_002);
        assert_eq!(a_crit.severity, 2);
        assert_ne!(a_crit.content_hash, 0);
    }

    #[test]
    fn test_monitor_to_api_status_codes() {
        let op = monitor_to_api_status(100, 100, 0, 0, 0.999);
        assert_eq!(op.status_code, 0); // operational

        let deg = monitor_to_api_status(1000, 999, 1, 1, 0.999);
        assert_eq!(deg.status_code, 1); // degraded but SLA met

        let out = monitor_to_api_status(1000, 800, 200, 10, 0.999);
        assert_eq!(out.status_code, 2); // outage
        assert_ne!(out.content_hash, 0);
    }
}
