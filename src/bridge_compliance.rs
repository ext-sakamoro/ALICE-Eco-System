//! Compliance bridges — ALICE-Compliance ↔ DB, Analytics, Cache, Legal, Edge
//!
//! 5 bridges connecting ALICE-Compliance (GDPR/SOX/HIPAA rules, risk scoring,
//! audit logs, data classification) to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Compliance → DB (audit record persistence) ─────────────────

/// Audit log record for ALICE-DB persistence.
///
/// Serializes a single `AuditEntry` into a flat, allocation-free record
/// suitable for insertion into the ALICE-DB audit table.
pub struct ComplianceDbAuditRecord {
    /// FNV-1a hash of `actor || action || resource` — primary DB key.
    pub content_hash: u64,
    /// FNV-1a hash of the actor identifier for actor-scoped queries.
    pub actor_hash: u64,
    /// FNV-1a hash of the resource identifier for resource-scoped queries.
    pub resource_hash: u64,
    /// Audit outcome: 0=success, 1=denied, 2=error.
    pub outcome: u8,
    /// Event timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Byte length of the action string.
    pub action_byte_len: usize,
}

/// Convert an audit entry into a DB persistence record.
///
/// # Optimization notes
/// - content_hash is computed over a 24-byte stack buffer containing
///   `fnv1a(actor)`, `fnv1a(action)`, `fnv1a(resource)` packed as LE u64s —
///   three FNV passes, no heap allocation.
/// - outcome is mapped from `AuditOutcome` via explicit match (no `as u8`).
#[inline]
#[must_use]
pub fn compliance_to_db_audit_record(
    actor: &str,
    action: &str,
    resource: &str,
    outcome: u8,
    timestamp_ms: u64,
) -> ComplianceDbAuditRecord {
    let actor_hash = fnv1a(actor.as_bytes());
    let action_hash = fnv1a(action.as_bytes());
    let resource_hash = fnv1a(resource.as_bytes());

    // Derive composite content_hash over all three hashes.
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&actor_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&action_hash.to_le_bytes());
    buf[16..24].copy_from_slice(&resource_hash.to_le_bytes());
    let content_hash = fnv1a(&buf);

    ComplianceDbAuditRecord {
        content_hash,
        actor_hash,
        resource_hash,
        outcome,
        timestamp_ms,
        action_byte_len: action.len(),
    }
}

// ── Bridge 2: Compliance → Analytics (compliance metrics) ────────────────

/// Compliance metrics event for ALICE-Analytics.
///
/// Aggregates violation counts and risk scores for a regulation at a point
/// in time so the analytics dashboard can track compliance health.
pub struct ComplianceAnalyticsEvent {
    /// FNV-1a hash of the regulation identifier — analytics stream key.
    pub content_hash: u64,
    /// Regulation code: 0=GDPR, 1=SOX, 2=HIPAA, 3=PCI, 4=ISO27001.
    pub regulation: u8,
    /// Total number of violations detected.
    pub violation_count: u32,
    /// Number of critical severity violations.
    pub critical_count: u32,
    /// Risk score (×100 as u32 for integer storage; 0–10000 represents 0–100.00).
    pub risk_score_centis: u32,
    /// Snapshot timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a compliance analytics event from violation summary data.
///
/// # Optimization notes
/// - `risk_score_centis` stores `risk_score * 100` as u32 — no f64 in the struct.
/// - regulation code is passed as pre-mapped u8; callers use match on the
///   `Regulation` enum (no `as u8` cast at call site).
#[inline]
#[must_use]
pub fn compliance_to_analytics_event(
    regulation_id: &str,
    regulation: u8,
    violation_count: u32,
    critical_count: u32,
    risk_score: f64,
    timestamp_ms: u64,
) -> ComplianceAnalyticsEvent {
    let content_hash = fnv1a(regulation_id.as_bytes());
    // Store risk_score as centis (×100) to avoid f64 serialization.
    let risk_score_centis = (risk_score * 100.0).clamp(0.0, 10_000.0) as u32;

    ComplianceAnalyticsEvent {
        content_hash,
        regulation,
        violation_count,
        critical_count,
        risk_score_centis,
        timestamp_ms,
    }
}

// ── Bridge 3: Compliance → Cache (regulation rule cache) ─────────────────

/// Regulation rule cache entry for ALICE-Cache.
///
/// Caches a parsed regulation rule set keyed by regulation identifier to
/// avoid repeated rule parsing on every compliance check invocation.
pub struct ComplianceCacheEntry {
    /// FNV-1a hash of the regulation identifier — cache lookup key.
    pub content_hash: u64,
    /// Regulation code: 0=GDPR, 1=SOX, 2=HIPAA, 3=PCI, 4=ISO27001.
    pub regulation: u8,
    /// Number of rules in the cached rule set.
    pub rule_count: u32,
    /// Serialized rule set byte length.
    pub rule_set_byte_len: usize,
    /// Cache TTL in seconds.
    ///
    /// Regulations with more rules expire sooner via branchless formula:
    /// `base - min(rule_count, 20) * step`.
    pub ttl_seconds: u32,
}

/// Build a regulation rule cache entry.
///
/// # Optimization notes
/// - TTL formula: `base=7200 - min(rule_count, 20) * 300`.
///   More rules → higher update frequency → shorter TTL.
///   Branchless: `min` compiles to `cmov` on x86-64.
#[inline]
#[must_use]
pub fn compliance_to_cache_entry(
    regulation_id: &str,
    regulation: u8,
    rule_count: u32,
    rule_set_byte_len: usize,
) -> ComplianceCacheEntry {
    let content_hash = fnv1a(regulation_id.as_bytes());

    // Branchless TTL: base=7200, step=300 per rule, max 20 rules subtracted.
    const BASE: u32 = 7_200;
    const STEP: u32 = 300;
    const MAX_RULES: u32 = 20;
    let clamped = rule_count.min(MAX_RULES);
    let ttl_seconds = BASE - clamped * STEP;

    ComplianceCacheEntry {
        content_hash,
        regulation,
        rule_count,
        rule_set_byte_len,
        ttl_seconds,
    }
}

// ── Bridge 4: Compliance → Legal (legal framework link) ──────────────────

/// Legal framework linkage record for ALICE-Legal.
///
/// Associates a compliance violation with the governing legal article so
/// the legal pipeline can generate remediation recommendations.
pub struct ComplianceLegalRecord {
    /// FNV-1a hash of `regulation_id || rule_id` — legal record key.
    pub content_hash: u64,
    /// FNV-1a hash of the rule identifier alone for rule-scoped queries.
    pub rule_hash: u64,
    /// Regulation code: 0=GDPR, 1=SOX, 2=HIPAA, 3=PCI, 4=ISO27001.
    pub regulation: u8,
    /// Severity: 0=info, 1=low, 2=medium, 3=high, 4=critical.
    pub severity: u8,
    /// Data classification of the affected field: 0=public .. 3=restricted.
    pub data_classification: u8,
    /// Violation timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a legal framework linkage record from a compliance rule violation.
///
/// # Optimization notes
/// - content_hash is derived over a 16-byte buffer combining hashes of
///   `regulation_id` and `rule_id` — two FNV passes, no heap allocation.
/// - severity and data_classification are passed as pre-mapped u8 values.
#[inline]
#[must_use]
pub fn compliance_to_legal_record(
    regulation_id: &str,
    rule_id: &str,
    regulation: u8,
    severity: u8,
    data_classification: u8,
    timestamp_ms: u64,
) -> ComplianceLegalRecord {
    let reg_hash = fnv1a(regulation_id.as_bytes());
    let rule_hash = fnv1a(rule_id.as_bytes());

    // Composite hash over regulation + rule hashes.
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&reg_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&rule_hash.to_le_bytes());
    let content_hash = fnv1a(&buf);

    ComplianceLegalRecord {
        content_hash,
        rule_hash,
        regulation,
        severity,
        data_classification,
        timestamp_ms,
    }
}

// ── Bridge 5: Compliance → Edge (compliance alerts) ──────────────────────

/// Compliance alert for ALICE-Edge forwarding.
///
/// Propagates high-severity compliance violations to the edge layer so that
/// real-time protective actions (blocking, rate limiting) can be taken.
pub struct ComplianceEdgeAlert {
    /// FNV-1a hash of the resource or actor identifier — edge routing key.
    pub content_hash: u64,
    /// Alert severity: 0=info, 1=low, 2=medium, 3=high, 4=critical.
    pub severity: u8,
    /// Regulation code: 0=GDPR, 1=SOX, 2=HIPAA, 3=PCI, 4=ISO27001.
    pub regulation: u8,
    /// Number of violations triggering this alert.
    pub violation_count: u32,
    /// Alert timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a compliance alert for ALICE-Edge.
///
/// # Optimization notes
/// - content_hash covers `resource_id` in one FNV pass — no allocation.
/// - severity and regulation are passed as pre-mapped u8 values.
#[inline]
#[must_use]
pub fn compliance_to_edge_alert(
    resource_id: &str,
    regulation: u8,
    severity: u8,
    violation_count: u32,
    timestamp_ms: u64,
) -> ComplianceEdgeAlert {
    let content_hash = fnv1a(resource_id.as_bytes());

    ComplianceEdgeAlert {
        content_hash,
        severity,
        regulation,
        violation_count,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const ACTOR: &str = "alice";
    const ACTION: &str = "read";
    const RESOURCE: &str = "pii-database";
    const REG_ID: &str = "GDPR";
    const RULE_ID: &str = "GDPR-ENC-001";

    #[test]
    fn test_db_audit_record_basic() {
        let rec = compliance_to_db_audit_record(ACTOR, ACTION, RESOURCE, 0, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0, "content_hash must be non-zero");
        assert_ne!(rec.actor_hash, 0);
        assert_ne!(rec.resource_hash, 0);
        assert_eq!(rec.outcome, 0); // success
        assert_eq!(rec.timestamp_ms, 1_700_000_000_000);
        assert_eq!(rec.action_byte_len, ACTION.len());
    }

    #[test]
    fn test_db_audit_record_hash_determinism() {
        let a = compliance_to_db_audit_record(ACTOR, ACTION, RESOURCE, 1, 0);
        let b = compliance_to_db_audit_record(ACTOR, ACTION, RESOURCE, 1, 0);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.actor_hash, b.actor_hash);
    }

    #[test]
    fn test_db_audit_record_distinct_actors() {
        let a = compliance_to_db_audit_record("alice", ACTION, RESOURCE, 0, 0);
        let b = compliance_to_db_audit_record("bob", ACTION, RESOURCE, 0, 0);
        assert_ne!(a.content_hash, b.content_hash);
        assert_ne!(a.actor_hash, b.actor_hash);
    }

    #[test]
    fn test_analytics_event_basic() {
        let ev = compliance_to_analytics_event(REG_ID, 0, 5, 2, 75.5, 1_700_000_001_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.regulation, 0); // GDPR
        assert_eq!(ev.violation_count, 5);
        assert_eq!(ev.critical_count, 2);
        assert_eq!(ev.risk_score_centis, 7_550); // 75.5 * 100
        assert_eq!(ev.timestamp_ms, 1_700_000_001_000);
    }

    #[test]
    fn test_analytics_event_risk_score_clamp() {
        // score > 100 → clamped to 10000 centis.
        let ev = compliance_to_analytics_event(REG_ID, 0, 99, 99, 150.0, 0);
        assert_eq!(ev.risk_score_centis, 10_000);
    }

    #[test]
    fn test_cache_entry_ttl_branchless() {
        // 0 rules → TTL = 7200 - 0*300 = 7200.
        let e0 = compliance_to_cache_entry(REG_ID, 0, 0, 512);
        assert_ne!(e0.content_hash, 0);
        assert_eq!(e0.ttl_seconds, 7_200);

        // 10 rules → TTL = 7200 - 10*300 = 4200.
        let e10 = compliance_to_cache_entry(REG_ID, 0, 10, 2048);
        assert_eq!(e10.ttl_seconds, 4_200);

        // 20 rules → TTL = 7200 - 20*300 = 1200.
        let e20 = compliance_to_cache_entry(REG_ID, 1, 20, 8192);
        assert_eq!(e20.ttl_seconds, 1_200);

        // 100 rules → clamped to 20 → TTL = 1200.
        let e100 = compliance_to_cache_entry(REG_ID, 2, 100, 32768);
        assert_eq!(e100.ttl_seconds, 1_200);
    }

    #[test]
    fn test_legal_record_basic() {
        let rec = compliance_to_legal_record(REG_ID, RULE_ID, 0, 3, 2, 1_700_000_002_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.rule_hash, 0);
        assert_eq!(rec.regulation, 0);
        assert_eq!(rec.severity, 3); // high
        assert_eq!(rec.data_classification, 2); // confidential
        assert_eq!(rec.timestamp_ms, 1_700_000_002_000);
    }

    #[test]
    fn test_legal_record_hash_covers_both_ids() {
        let rec_a = compliance_to_legal_record(REG_ID, "RULE-A", 0, 1, 1, 0);
        let rec_b = compliance_to_legal_record(REG_ID, "RULE-B", 0, 1, 1, 0);
        assert_ne!(rec_a.content_hash, rec_b.content_hash, "different rule_id → different content_hash");
        assert_ne!(rec_a.rule_hash, rec_b.rule_hash);
    }

    #[test]
    fn test_edge_alert_basic() {
        let alert = compliance_to_edge_alert(RESOURCE, 0, 4, 3, 1_700_000_003_000);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.regulation, 0); // GDPR
        assert_eq!(alert.severity, 4); // critical
        assert_eq!(alert.violation_count, 3);
        assert_eq!(alert.timestamp_ms, 1_700_000_003_000);
    }

    #[test]
    fn test_edge_alert_hash_determinism() {
        let a = compliance_to_edge_alert(RESOURCE, 1, 2, 1, 999);
        let b = compliance_to_edge_alert(RESOURCE, 1, 2, 1, 999);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
