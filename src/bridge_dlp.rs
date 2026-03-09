//! DLP bridges — ALICE-DLP ↔ DB, Cache, Analytics, Monitor, Notify
//!
//! 5 bridges connecting data loss prevention scan results to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: DLP → DB (scan log) ────────────────────────────────────────

/// DLP scan log record for ALICE-DB persistence.
///
/// Written for every DLP scan so that compliance officers can audit PII
/// detection rates, policy violations, and data masking effectiveness
/// across the organisation's data pipelines.
pub struct DlpDbScanLog {
    /// FNV-1a hash of the scan target identifier (e.g. pipeline + dataset name).
    pub content_hash: u64,
    /// Number of PII tokens detected in the scanned payload.
    pub pii_count: u32,
    /// Number of PII tokens that were masked or redacted.
    pub masked_count: u32,
    /// Number of policy violations triggered by this scan.
    pub policy_violations: u32,
    /// Total bytes scanned in this DLP operation.
    pub scan_bytes: u64,
    /// Sensitivity level of the scanned data (0=public, 1=internal, 2=confidential, 3=restricted).
    pub sensitivity_level: u8,
    /// Scan decision: 0 = pass, 1 = quarantine, 2 = block.
    pub decision: u8,
    /// Unix timestamp in milliseconds when the scan completed.
    pub timestamp_ms: u64,
}

/// Build a DLP scan log record for ALICE-DB.
///
/// `content_hash` is derived from the target identifier so that log records
/// for the same pipeline/dataset can be grouped without additional lookups —
/// deterministic, allocation-free.
#[inline]
#[must_use]
pub fn dlp_to_db_scan_log(
    target: &[u8],
    pii_count: u32,
    masked_count: u32,
    policy_violations: u32,
    scan_bytes: u64,
    sensitivity_level: u8,
    decision: u8,
    timestamp_ms: u64,
) -> DlpDbScanLog {
    let content_hash = fnv1a(target);
    DlpDbScanLog {
        content_hash,
        pii_count,
        masked_count,
        policy_violations,
        scan_bytes,
        sensitivity_level: sensitivity_level.min(3),
        decision: decision.min(2),
        timestamp_ms,
    }
}

// ── Bridge 2: DLP → Cache (policy cache) ─────────────────────────────────

/// Policy cache entry for ALICE-Cache.
///
/// Caches the compiled DLP policy set so that per-scan evaluation can
/// retrieve pre-compiled rules without re-fetching from policy storage.
/// TTL is shortened when recent scans have produced policy violations,
/// forcing faster policy refresh to pick up updated rules.
pub struct DlpCachePolicyEntry {
    /// FNV-1a hash of the policy set identifier used as the cache key.
    pub content_hash: u64,
    /// Number of PII patterns compiled into this policy set.
    pub pii_count: u32,
    /// Number of masking rules compiled into this policy set.
    pub masked_count: u32,
    /// Policy violation count that triggered this cache refresh.
    pub policy_violations: u32,
    /// Total bytes covered by the most recent scan using this policy.
    pub scan_bytes: u64,
    /// Sensitivity level the policy targets (0–3).
    pub sensitivity_level: u8,
    /// Time-to-live in seconds for this policy cache entry.
    pub ttl_seconds: u32,
}

/// Build a DLP policy cache entry with violation-adjusted TTL.
///
/// TTL derivation (branchless):
/// - policy_violations > 0 →  60 s (recent violations, refresh quickly)
/// - else                  → 600 s (stable policy, cache aggressively)
#[inline]
#[must_use]
pub fn dlp_to_cache_policy_entry(
    policy_id: &[u8],
    pii_count: u32,
    masked_count: u32,
    policy_violations: u32,
    scan_bytes: u64,
    sensitivity_level: u8,
) -> DlpCachePolicyEntry {
    let content_hash = fnv1a(policy_id);
    // Branchless TTL: violations present=60, stable=600.
    let has_violations = (policy_violations > 0) as u32;
    let ttl_seconds = 600 - has_violations * 540; // 600 or 60
    DlpCachePolicyEntry {
        content_hash,
        pii_count,
        masked_count,
        policy_violations,
        scan_bytes,
        sensitivity_level: sensitivity_level.min(3),
        ttl_seconds,
    }
}

// ── Bridge 3: DLP → Analytics (detection metrics) ────────────────────────

/// Detection metrics event for ALICE-Analytics ingestion.
///
/// Aggregates DLP scan counters per sampling interval so that the analytics
/// layer can chart PII exposure rates, masking coverage, and policy
/// compliance trends without storing raw scan payloads.
pub struct DlpAnalyticsDetectionEvent {
    /// FNV-1a hash of the pipeline identifier for this metrics event.
    pub content_hash: u64,
    /// Total PII tokens detected in the sampling interval.
    pub pii_count: u32,
    /// Total PII tokens masked in the sampling interval.
    pub masked_count: u32,
    /// Total policy violations in the sampling interval.
    pub policy_violations: u32,
    /// Total bytes scanned in the sampling interval.
    pub scan_bytes: u64,
    /// Sensitivity level of the most sensitive data observed (0–3).
    pub sensitivity_level: u8,
    /// Masking coverage in integer percent (0–100).
    pub masking_pct: u8,
    /// Unix timestamp in milliseconds when the interval ended.
    pub timestamp_ms: u64,
}

/// Build a DLP detection metrics event for ALICE-Analytics.
///
/// `masking_pct` = `masked_count / pii_count * 100` — integer arithmetic,
/// denominator clamped to 1 to avoid division by zero, result clamped to 100.
#[inline]
#[must_use]
pub fn dlp_to_analytics_detection_event(
    pipeline: &[u8],
    pii_count: u32,
    masked_count: u32,
    policy_violations: u32,
    scan_bytes: u64,
    sensitivity_level: u8,
    timestamp_ms: u64,
) -> DlpAnalyticsDetectionEvent {
    let content_hash = fnv1a(pipeline);
    let denom = (pii_count as u64).max(1);
    let masking_pct = (((masked_count as u64) * 100) / denom).min(100) as u8;
    DlpAnalyticsDetectionEvent {
        content_hash,
        pii_count,
        masked_count,
        policy_violations,
        scan_bytes,
        sensitivity_level: sensitivity_level.min(3),
        masking_pct,
        timestamp_ms,
    }
}

// ── Bridge 4: DLP → Monitor (compliance) ─────────────────────────────────

/// Compliance status record for ALICE-Monitor.
///
/// Encodes the current DLP compliance posture so that the monitor layer
/// can update dashboards and trigger escalations without re-aggregating
/// raw scan logs.
///
/// `compliance_level`: 0 = compliant, 1 = at-risk, 2 = non-compliant.
pub struct DlpMonitorCompliance {
    /// FNV-1a hash of the pipeline identifier.
    pub content_hash: u64,
    /// PII token count driving the compliance assessment.
    pub pii_count: u32,
    /// Masked token count driving the compliance assessment.
    pub masked_count: u32,
    /// Policy violation count in the assessment window.
    pub policy_violations: u32,
    /// Total bytes scanned in the assessment window.
    pub scan_bytes: u64,
    /// Sensitivity level of the most sensitive data assessed (0–3).
    pub sensitivity_level: u8,
    /// Compliance level (0=compliant, 1=at-risk, 2=non-compliant).
    pub compliance_level: u8,
    /// Unix timestamp in milliseconds of the assessment.
    pub timestamp_ms: u64,
}

/// Build a DLP compliance status record for ALICE-Monitor.
///
/// Compliance level derivation (branchless):
/// - policy_violations > 10 OR sensitivity_level == 3 AND pii_count > 0 → non-compliant (2)
/// - policy_violations > 0  OR unmasked PII > 0                          → at-risk (1)
/// - else                                                                 → compliant (0)
///
/// `unmasked_pii` = pii_count.saturating_sub(masked_count).
#[inline]
#[must_use]
pub fn dlp_to_monitor_compliance(
    pipeline: &[u8],
    pii_count: u32,
    masked_count: u32,
    policy_violations: u32,
    scan_bytes: u64,
    sensitivity_level: u8,
    timestamp_ms: u64,
) -> DlpMonitorCompliance {
    let content_hash = fnv1a(pipeline);
    let unmasked_pii = pii_count.saturating_sub(masked_count);
    let level = sensitivity_level.min(3);
    let is_non_compliant = ((policy_violations > 10) || (level == 3 && pii_count > 0)) as u8;
    let is_at_risk = ((policy_violations > 0) || (unmasked_pii > 0)) as u8;
    // Non-compliant overrides at-risk overrides compliant.
    let compliance_level = (is_non_compliant * 2).max(is_at_risk);
    DlpMonitorCompliance {
        content_hash,
        pii_count,
        masked_count,
        policy_violations,
        scan_bytes,
        sensitivity_level: level,
        compliance_level,
        timestamp_ms,
    }
}

// ── Bridge 5: DLP → Notify (alerts) ──────────────────────────────────────

/// Alert record for ALICE-Notify.
///
/// Emitted only for at-risk or non-compliant compliance levels so that the
/// notify layer does not send alerts during fully compliant scans.
///
/// `severity`: 1 = at-risk, 2 = non-compliant.
pub struct DlpNotifyAlert {
    /// FNV-1a hash of the pipeline identifier for routing the alert.
    pub content_hash: u64,
    /// Alert severity (1=at-risk, 2=non-compliant).
    pub severity: u8,
    /// PII token count that triggered the alert.
    pub pii_count: u32,
    /// Policy violation count that triggered the alert.
    pub policy_violations: u32,
    /// Bytes that were scanned when the alert was raised.
    pub scan_bytes: u64,
    /// Sensitivity level of the data that triggered the alert (0–3).
    pub sensitivity_level: u8,
    /// Estimated notification payload size in bytes.
    pub payload_bytes: usize,
    /// Unix timestamp in milliseconds when the alert was raised.
    pub timestamp_ms: u64,
}

/// Build a DLP alert for ALICE-Notify.
///
/// Returns `None` for compliance_level == 0 (fully compliant) so the caller
/// never sends alerts when scans pass cleanly.
///
/// `payload_bytes` is estimated as `64 + severity as usize * 24` —
/// integer multiply, no division, no branches.
#[inline]
#[must_use]
pub fn dlp_to_notify_alert(
    pipeline: &[u8],
    compliance_level: u8,
    pii_count: u32,
    policy_violations: u32,
    scan_bytes: u64,
    sensitivity_level: u8,
    timestamp_ms: u64,
) -> Option<DlpNotifyAlert> {
    if compliance_level == 0 {
        return None;
    }
    let content_hash = fnv1a(pipeline);
    let severity = compliance_level.min(2);
    let payload_bytes = 64 + (severity as usize) * 24;
    Some(DlpNotifyAlert {
        content_hash,
        severity,
        pii_count,
        policy_violations,
        scan_bytes,
        sensitivity_level: sensitivity_level.min(3),
        payload_bytes,
        timestamp_ms,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlp_to_db_scan_log_basic() {
        let log = dlp_to_db_scan_log(
            b"pipeline-A/dataset-1",
            50,
            48,
            2,
            1_000_000,
            2,
            0,
            1_700_000_000_000,
        );
        assert_eq!(log.content_hash, fnv1a(b"pipeline-A/dataset-1"));
        assert_ne!(log.content_hash, 0);
        assert_eq!(log.pii_count, 50);
        assert_eq!(log.masked_count, 48);
        assert_eq!(log.policy_violations, 2);
        assert_eq!(log.sensitivity_level, 2);
        assert_eq!(log.decision, 0);
    }

    #[test]
    fn test_dlp_to_db_scan_log_fields_clamped() {
        let log = dlp_to_db_scan_log(b"x", 0, 0, 0, 0, 99, 99, 0);
        assert_eq!(log.sensitivity_level, 3, "sensitivity_level clamped to 3");
        assert_eq!(log.decision, 2, "decision clamped to 2");
    }

    #[test]
    fn test_dlp_to_cache_policy_entry_stable_ttl() {
        // No violations → TTL = 600 s
        let e = dlp_to_cache_policy_entry(b"policy-v2", 200, 180, 0, 5_000_000, 1);
        assert_eq!(e.ttl_seconds, 600);
        assert_ne!(e.content_hash, 0);
    }

    #[test]
    fn test_dlp_to_cache_policy_entry_violation_ttl() {
        // Violations present → TTL = 60 s
        let e = dlp_to_cache_policy_entry(b"policy-v2", 200, 180, 5, 5_000_000, 2);
        assert_eq!(e.ttl_seconds, 60);
    }

    #[test]
    fn test_dlp_to_analytics_detection_event_masking_pct() {
        // masked=90, pii=100 → masking_pct = 90
        let ev = dlp_to_analytics_detection_event(b"pipeline-B", 100, 90, 1, 2_000_000, 1, 0);
        assert_eq!(ev.masking_pct, 90);
        assert_ne!(ev.content_hash, 0);
        // masked=0, pii=0 → masking_pct = 0 (no division by zero)
        let ev2 = dlp_to_analytics_detection_event(b"pipeline-B", 0, 0, 0, 0, 0, 0);
        assert_eq!(ev2.masking_pct, 0);
    }

    #[test]
    fn test_dlp_to_monitor_compliance_levels() {
        // Compliant: no violations, all PII masked
        let c = dlp_to_monitor_compliance(b"pipe", 10, 10, 0, 500_000, 1, 0);
        assert_eq!(c.compliance_level, 0);
        // At-risk: unmasked PII > 0
        let r = dlp_to_monitor_compliance(b"pipe", 10, 8, 0, 500_000, 1, 0);
        assert_eq!(r.compliance_level, 1);
        // At-risk: policy_violations > 0
        let r2 = dlp_to_monitor_compliance(b"pipe", 10, 10, 1, 500_000, 1, 0);
        assert_eq!(r2.compliance_level, 1);
        // Non-compliant: violations > 10
        let nc = dlp_to_monitor_compliance(b"pipe", 10, 10, 15, 500_000, 1, 0);
        assert_eq!(nc.compliance_level, 2);
        // Non-compliant: sensitivity_level == 3 AND pii_count > 0
        let nc2 = dlp_to_monitor_compliance(b"pipe", 1, 1, 0, 500_000, 3, 0);
        assert_eq!(nc2.compliance_level, 2);
    }

    #[test]
    fn test_dlp_to_notify_alert_compliant_returns_none() {
        let r = dlp_to_notify_alert(b"pipe", 0, 0, 0, 0, 0, 0);
        assert!(r.is_none(), "compliant scan must not produce an alert");
    }

    #[test]
    fn test_dlp_to_notify_alert_severity_and_payload() {
        // At-risk → severity 1, payload = 64 + 1*24 = 88
        let a1 = dlp_to_notify_alert(b"prod-pipe", 1, 5, 1, 1_000_000, 2, 1_700_000_000_000)
            .expect("at-risk must produce an alert");
        assert_eq!(a1.severity, 1);
        assert_eq!(a1.payload_bytes, 88);
        assert_ne!(a1.content_hash, 0);
        // Non-compliant → severity 2, payload = 64 + 2*24 = 112
        let a2 = dlp_to_notify_alert(b"prod-pipe", 2, 50, 20, 5_000_000, 3, 0)
            .expect("non-compliant must produce an alert");
        assert_eq!(a2.severity, 2);
        assert_eq!(a2.payload_bytes, 112);
    }
}
