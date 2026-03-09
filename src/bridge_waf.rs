//! WAF bridges — ALICE-WAF ↔ DB, Cache, Analytics, Monitor, Notify
//!
//! 5 bridges connecting web application firewall events to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: WAF → DB (event log) ───────────────────────────────────────

/// WAF event log record for ALICE-DB persistence.
///
/// Written for every WAF decision so that security analysts can replay
/// blocked requests, tune rules, and generate compliance reports offline.
pub struct WafDbEventLog {
    /// FNV-1a hash of the client IP + URI combined.
    pub content_hash: u64,
    /// Total number of active WAF rules evaluated for this request.
    pub rule_count: u32,
    /// Number of requests blocked in the current sampling window.
    pub blocked_count: u64,
    /// Number of requests allowed through in the current sampling window.
    pub allowed_count: u64,
    /// Number of SQL injection patterns detected.
    pub sqli_detected: u32,
    /// Number of cross-site scripting patterns detected.
    pub xss_detected: u32,
    /// Decision code: 0 = allow, 1 = block, 2 = challenge.
    pub decision: u8,
    /// Unix timestamp in milliseconds when the decision was made.
    pub timestamp_ms: u64,
}

/// Build a WAF event log record for ALICE-DB.
///
/// `content_hash` is derived by XOR-chaining hashes of the client IP and
/// URI so that a change to either component produces a different composite
/// hash — branchless, single XOR, no allocation.
#[inline]
#[must_use]
pub fn waf_to_db_event_log(
    client_ip: &[u8],
    uri: &[u8],
    rule_count: u32,
    blocked_count: u64,
    allowed_count: u64,
    sqli_detected: u32,
    xss_detected: u32,
    decision: u8,
    timestamp_ms: u64,
) -> WafDbEventLog {
    let content_hash = fnv1a(client_ip) ^ fnv1a(uri);
    WafDbEventLog {
        content_hash,
        rule_count,
        blocked_count,
        allowed_count,
        sqli_detected,
        xss_detected,
        decision: decision.min(2),
        timestamp_ms,
    }
}

// ── Bridge 2: WAF → Cache (rule cache) ───────────────────────────────────

/// Rule cache entry for ALICE-Cache.
///
/// Caches the compiled WAF rule set so that per-request evaluation can
/// retrieve pre-compiled rules without re-parsing from storage.  TTL is
/// reduced when SQLi or XSS detections are elevated, forcing faster
/// rule refresh to incorporate updated signatures.
pub struct WafCacheRuleEntry {
    /// FNV-1a hash of the rule-set identifier used as the cache key.
    pub content_hash: u64,
    /// Number of rules compiled into this cache entry.
    pub rule_count: u32,
    /// SQLi detection count that influenced this rule set version.
    pub sqli_detected: u32,
    /// XSS detection count that influenced this rule set version.
    pub xss_detected: u32,
    /// Number of requests blocked using this rule set.
    pub blocked_count: u64,
    /// Time-to-live in seconds for this rule cache entry.
    pub ttl_seconds: u32,
    /// Rule set version for cache invalidation.
    pub rule_version: u32,
}

/// Build a WAF rule cache entry with threat-adjusted TTL.
///
/// TTL derivation (branchless):
/// - (sqli_detected + xss_detected) > 10 → 60 s  (high threat, refresh fast)
/// - else                                 → 300 s (normal refresh cycle)
#[inline]
#[must_use]
pub fn waf_to_cache_rule_entry(
    rule_set_id: &[u8],
    rule_count: u32,
    sqli_detected: u32,
    xss_detected: u32,
    blocked_count: u64,
    rule_version: u32,
) -> WafCacheRuleEntry {
    let content_hash = fnv1a(rule_set_id);
    let combined_detections = sqli_detected.saturating_add(xss_detected);
    // Branchless TTL: high-threat=60, normal=300.
    let high_threat = (combined_detections > 10) as u32;
    let ttl_seconds = 300 - high_threat * 240; // 300 or 60
    WafCacheRuleEntry {
        content_hash,
        rule_count,
        sqli_detected,
        xss_detected,
        blocked_count,
        ttl_seconds,
        rule_version,
    }
}

// ── Bridge 3: WAF → Analytics (security metrics) ─────────────────────────

/// Security metrics event for ALICE-Analytics ingestion.
///
/// Aggregates WAF counters per sampling interval so that the analytics
/// layer can chart attack trends, block rates, and rule effectiveness
/// without storing raw request payloads.
pub struct WafAnalyticsSecurityEvent {
    /// FNV-1a hash of the origin identifier for this metrics event.
    pub content_hash: u64,
    /// Number of WAF rules active during the sampling interval.
    pub rule_count: u32,
    /// Total requests blocked in the sampling interval.
    pub blocked_count: u64,
    /// Total requests allowed in the sampling interval.
    pub allowed_count: u64,
    /// SQL injection detections in the sampling interval.
    pub sqli_detected: u32,
    /// XSS detections in the sampling interval.
    pub xss_detected: u32,
    /// Block rate in integer percent (0–100).
    pub block_rate_pct: u8,
    /// Unix timestamp in milliseconds when the interval ended.
    pub timestamp_ms: u64,
}

/// Build a WAF security metrics event for ALICE-Analytics.
///
/// `block_rate_pct` = `blocked / (blocked + allowed) * 100` — integer
/// arithmetic, denominator clamped to 1, result clamped to 100.
#[inline]
#[must_use]
pub fn waf_to_analytics_security_event(
    origin: &[u8],
    rule_count: u32,
    blocked_count: u64,
    allowed_count: u64,
    sqli_detected: u32,
    xss_detected: u32,
    timestamp_ms: u64,
) -> WafAnalyticsSecurityEvent {
    let content_hash = fnv1a(origin);
    let total = blocked_count.saturating_add(allowed_count).max(1);
    let block_rate_pct = ((blocked_count * 100) / total).min(100) as u8;
    WafAnalyticsSecurityEvent {
        content_hash,
        rule_count,
        blocked_count,
        allowed_count,
        sqli_detected,
        xss_detected,
        block_rate_pct,
        timestamp_ms,
    }
}

// ── Bridge 4: WAF → Monitor (threat level) ───────────────────────────────

/// Threat level record for ALICE-Monitor.
///
/// Encodes the current WAF threat posture so that the monitor layer can
/// update dashboards and trigger escalations without re-aggregating raw
/// event logs.
///
/// `threat_level`: 0 = low, 1 = medium, 2 = high, 3 = critical.
pub struct WafMonitorThreatLevel {
    /// FNV-1a hash of the origin identifier.
    pub content_hash: u64,
    /// Number of WAF rules currently active.
    pub rule_count: u32,
    /// Blocked request count contributing to threat assessment.
    pub blocked_count: u64,
    /// Allowed request count in the same window.
    pub allowed_count: u64,
    /// SQLi detections in the assessment window.
    pub sqli_detected: u32,
    /// XSS detections in the assessment window.
    pub xss_detected: u32,
    /// Assessed threat level (0=low, 1=medium, 2=high, 3=critical).
    pub threat_level: u8,
    /// Unix timestamp in milliseconds of the assessment.
    pub timestamp_ms: u64,
}

/// Build a WAF threat level record for ALICE-Monitor.
///
/// Threat level derivation (branchless integer arithmetic):
/// - sqli_detected > 100 OR xss_detected > 100 → critical (3)
/// - sqli_detected > 10  OR xss_detected > 10  → high (2)
/// - blocked_count > 1000                       → medium (1)
/// - else                                       → low (0)
///
/// Levels computed as saturating additions of condition flags, clamped to 3.
#[inline]
#[must_use]
pub fn waf_to_monitor_threat_level(
    origin: &[u8],
    rule_count: u32,
    blocked_count: u64,
    allowed_count: u64,
    sqli_detected: u32,
    xss_detected: u32,
    timestamp_ms: u64,
) -> WafMonitorThreatLevel {
    let content_hash = fnv1a(origin);
    let is_critical = ((sqli_detected > 100) || (xss_detected > 100)) as u8;
    let is_high = ((sqli_detected > 10) || (xss_detected > 10)) as u8;
    let is_medium = (blocked_count > 1000) as u8;
    // Branchless level: critical overrides high overrides medium overrides low.
    let threat_level = (is_critical * 3).max(is_high * 2).max(is_medium);
    WafMonitorThreatLevel {
        content_hash,
        rule_count,
        blocked_count,
        allowed_count,
        sqli_detected,
        xss_detected,
        threat_level,
        timestamp_ms,
    }
}

// ── Bridge 5: WAF → Notify (alerts) ──────────────────────────────────────

/// Alert record for ALICE-Notify.
///
/// Emitted only for medium-or-higher threat levels so that the notify layer
/// does not flood operators with low-priority events during normal traffic.
///
/// `severity`: 1 = medium, 2 = high, 3 = critical.
pub struct WafNotifyAlert {
    /// FNV-1a hash of the origin identifier for routing the alert.
    pub content_hash: u64,
    /// Alert severity derived from threat level (1–3).
    pub severity: u8,
    /// SQLi detection count that triggered the alert.
    pub sqli_detected: u32,
    /// XSS detection count that triggered the alert.
    pub xss_detected: u32,
    /// Blocked request count in the alert window.
    pub blocked_count: u64,
    /// Estimated notification payload size in bytes.
    pub payload_bytes: usize,
    /// Unix timestamp in milliseconds when the alert was raised.
    pub timestamp_ms: u64,
}

/// Build a WAF alert for ALICE-Notify.
///
/// Returns `None` for threat_level == 0 (low) so the caller never sends
/// alerts during normal operation.
///
/// `payload_bytes` is estimated as `48 + severity as usize * 16` —
/// integer multiply, no division, no branches.
#[inline]
#[must_use]
pub fn waf_to_notify_alert(
    origin: &[u8],
    threat_level: u8,
    sqli_detected: u32,
    xss_detected: u32,
    blocked_count: u64,
    timestamp_ms: u64,
) -> Option<WafNotifyAlert> {
    if threat_level == 0 {
        return None;
    }
    let content_hash = fnv1a(origin);
    let severity = threat_level.min(3);
    let payload_bytes = 48 + (severity as usize) * 16;
    Some(WafNotifyAlert {
        content_hash,
        severity,
        sqli_detected,
        xss_detected,
        blocked_count,
        payload_bytes,
        timestamp_ms,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waf_to_db_event_log_basic() {
        let log = waf_to_db_event_log(
            b"192.168.1.1",
            b"/admin",
            42,
            10,
            990,
            3,
            1,
            1,
            1_700_000_000_000,
        );
        assert_eq!(log.content_hash, fnv1a(b"192.168.1.1") ^ fnv1a(b"/admin"));
        assert_ne!(log.content_hash, 0);
        assert_eq!(log.rule_count, 42);
        assert_eq!(log.sqli_detected, 3);
        assert_eq!(log.xss_detected, 1);
        assert_eq!(log.decision, 1);
    }

    #[test]
    fn test_waf_to_db_event_log_decision_clamped() {
        let log = waf_to_db_event_log(b"10.0.0.1", b"/", 0, 0, 0, 0, 0, 99, 0);
        assert_eq!(log.decision, 2, "decision clamped to 2");
    }

    #[test]
    fn test_waf_to_cache_rule_entry_normal_ttl() {
        let e = waf_to_cache_rule_entry(b"ruleset-v3", 150, 5, 3, 100, 3);
        assert_eq!(e.ttl_seconds, 300);
        assert_ne!(e.content_hash, 0);
        assert_eq!(e.rule_version, 3);
    }

    #[test]
    fn test_waf_to_cache_rule_entry_high_threat_ttl() {
        // sqli + xss > 10 → TTL = 60 s
        let e = waf_to_cache_rule_entry(b"ruleset-v3", 200, 8, 5, 9999, 4);
        assert_eq!(e.ttl_seconds, 60);
    }

    #[test]
    fn test_waf_to_analytics_security_event_block_rate() {
        // blocked=1, allowed=99 → block_rate_pct = 1
        let ev = waf_to_analytics_security_event(b"origin-1", 100, 1, 99, 0, 0, 0);
        assert_eq!(ev.block_rate_pct, 1);
        assert_ne!(ev.content_hash, 0);
        // blocked=50, allowed=50 → 50%
        let ev2 = waf_to_analytics_security_event(b"origin-1", 100, 50, 50, 0, 0, 0);
        assert_eq!(ev2.block_rate_pct, 50);
    }

    #[test]
    fn test_waf_to_monitor_threat_levels() {
        // Low: no significant activity
        let low = waf_to_monitor_threat_level(b"origin", 10, 100, 9900, 0, 0, 0);
        assert_eq!(low.threat_level, 0);
        // Medium: blocked > 1000
        let med = waf_to_monitor_threat_level(b"origin", 10, 1500, 8500, 0, 0, 0);
        assert_eq!(med.threat_level, 1);
        // High: sqli > 10
        let high = waf_to_monitor_threat_level(b"origin", 10, 100, 900, 15, 0, 0);
        assert_eq!(high.threat_level, 2);
        // Critical: sqli > 100
        let crit = waf_to_monitor_threat_level(b"origin", 10, 100, 900, 200, 0, 0);
        assert_eq!(crit.threat_level, 3);
    }

    #[test]
    fn test_waf_to_notify_alert_low_returns_none() {
        let r = waf_to_notify_alert(b"origin", 0, 0, 0, 0, 0);
        assert!(r.is_none(), "low threat must not produce an alert");
    }

    #[test]
    fn test_waf_to_notify_alert_severity_and_payload() {
        let alert = waf_to_notify_alert(b"prod-origin", 3, 200, 50, 5000, 1_700_000_000_000)
            .expect("critical threat must produce an alert");
        assert_eq!(alert.severity, 3);
        assert_eq!(alert.payload_bytes, 48 + 3 * 16); // 96
        assert_ne!(alert.content_hash, 0);
        // Medium severity payload
        let m = waf_to_notify_alert(b"prod-origin", 1, 0, 0, 0, 0).unwrap();
        assert_eq!(m.payload_bytes, 48 + 16); // 64
    }
}
