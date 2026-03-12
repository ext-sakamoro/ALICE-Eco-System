//! Consciousness bridges — ALICE-Consciousness ↔ DB, Cache, Analytics, Compliance, Log
//!
//! 5 bridges connecting the IIT Φ engine and ethical reasoning subsystem
//! (Project-ALICE V3) to the ALICE ecosystem.  Covers Φ value persistence,
//! consciousness state caching, ethics metrics, compliance audit, and
//! taboo violation logging.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Consciousness → DB (Φ measurement record) ─────────────────

/// IIT Φ measurement record for ALICE-DB persistence.
///
/// Stores the result of one Φ computation: partition structure, Φ value,
/// number of concepts, and consciousness level classification.
pub struct ConsciousnessDbPhiRecord {
    /// FNV-1a hash over agent_id + measurement_id — row deduplication key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Measurement identifier (monotonic).
    pub measurement_id: u64,
    /// Φ value in milliphi (1000 = Φ 1.0).
    pub phi_milliphi: u32,
    /// Number of concepts in the complex.
    pub concept_count: u32,
    /// Number of elements in the system.
    pub element_count: u32,
    /// Consciousness level: 0 = None, 1 = Minimal, 2 = Low, 3 = Medium, 4 = High.
    pub level: u8,
    /// Computation time in microseconds.
    pub compute_time_us: u64,
}

/// Build a `ConsciousnessDbPhiRecord` for Φ persistence.
#[inline]
#[must_use]
pub fn consciousness_to_db_phi_record(
    agent_id: u64,
    measurement_id: u64,
    phi_milliphi: u32,
    concept_count: u32,
    element_count: u32,
    level: u8,
    compute_time_us: u64,
) -> ConsciousnessDbPhiRecord {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&agent_id.to_le_bytes());
    buf[8..16].copy_from_slice(&measurement_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    ConsciousnessDbPhiRecord {
        content_hash,
        agent_id,
        measurement_id,
        phi_milliphi,
        concept_count,
        element_count,
        level,
        compute_time_us,
    }
}

// ── Bridge 2: Consciousness → Cache (state snapshot) ─────────────────────

/// Consciousness state cache for ALICE-Cache.
///
/// Per-agent snapshot of current Φ value and consciousness level.
/// TTL is shorter for higher consciousness levels (state evolves faster).
pub struct ConsciousnessCacheState {
    /// FNV-1a hash over agent_id — cache lookup key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Current Φ value in milliphi.
    pub phi_milliphi: u32,
    /// Consciousness level (0–4).
    pub level: u8,
    /// Number of concepts.
    pub concept_count: u32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build a `ConsciousnessCacheState` entry.
///
/// TTL: high consciousness (level >= 3) → 5 s, otherwise → 30 s (branchless).
#[inline]
#[must_use]
pub fn consciousness_to_cache_state(
    agent_id: u64,
    phi_milliphi: u32,
    level: u8,
    concept_count: u32,
) -> ConsciousnessCacheState {
    let content_hash = fnv1a(&agent_id.to_le_bytes());
    // Branchless TTL: high consciousness → 5s, low → 30s.
    let is_high = (level >= 3) as u32;
    let ttl_secs = 30 - is_high * 25;
    ConsciousnessCacheState {
        content_hash,
        agent_id,
        phi_milliphi,
        level,
        concept_count,
        ttl_secs,
    }
}

// ── Bridge 3: Consciousness → Analytics (ethics metrics) ─────────────────

/// Ethics evaluation metrics for ALICE-Analytics.
///
/// Tracks ethical reasoning throughput: evaluations performed, taboo
/// checks, constraint violations, and developmental stage transitions.
pub struct ConsciousnessAnalyticsMetrics {
    /// FNV-1a hash over agent_id + tick — deduplication key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Metric tick.
    pub tick: u64,
    /// Number of ethical evaluations performed.
    pub evaluations: u32,
    /// Number of taboo checks executed.
    pub taboo_checks: u32,
    /// Number of taboo violations detected.
    pub taboo_violations: u32,
    /// Current developmental stage: 0 = Infant, 1 = Child, 2 = Adolescent, 3 = Adult.
    pub developmental_stage: u8,
    /// Mean Φ in milliphi for this interval.
    pub mean_phi_milliphi: u32,
}

/// Build a `ConsciousnessAnalyticsMetrics` event.
#[inline]
#[must_use]
pub fn consciousness_to_analytics_metrics(
    agent_id: u64,
    tick: u64,
    evaluations: u32,
    taboo_checks: u32,
    taboo_violations: u32,
    developmental_stage: u8,
    mean_phi_milliphi: u32,
) -> ConsciousnessAnalyticsMetrics {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&agent_id.to_le_bytes());
    buf[8..16].copy_from_slice(&tick.to_le_bytes());
    let content_hash = fnv1a(&buf);
    ConsciousnessAnalyticsMetrics {
        content_hash,
        agent_id,
        tick,
        evaluations,
        taboo_checks,
        taboo_violations,
        developmental_stage,
        mean_phi_milliphi,
    }
}

// ── Bridge 4: Consciousness → Compliance (ethics audit record) ───────────

/// Ethics compliance audit record for ALICE-Compliance.
///
/// Periodic report on ethical reasoning posture for regulatory review.
pub struct ConsciousnessComplianceAudit {
    /// FNV-1a hash over agent_id + audit_epoch — audit key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Audit epoch.
    pub audit_epoch: u64,
    /// Total evaluations in this epoch.
    pub total_evaluations: u32,
    /// Total taboo violations in this epoch.
    pub total_violations: u32,
    /// Ethics compliance score in permille (1000 = fully compliant).
    pub ethics_score_permille: u16,
    /// Highest developmental stage reached.
    pub max_stage: u8,
    /// Maximum Φ value observed in this epoch (milliphi).
    pub max_phi_milliphi: u32,
}

/// Build a `ConsciousnessComplianceAudit`.
///
/// Ethics score: 1000 - violations * 100, clamped to [0, 1000].
#[inline]
#[must_use]
pub fn consciousness_to_compliance_audit(
    agent_id: u64,
    audit_epoch: u64,
    total_evaluations: u32,
    total_violations: u32,
    max_stage: u8,
    max_phi_milliphi: u32,
) -> ConsciousnessComplianceAudit {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&agent_id.to_le_bytes());
    buf[8..16].copy_from_slice(&audit_epoch.to_le_bytes());
    let content_hash = fnv1a(&buf);
    let penalty = total_violations.saturating_mul(100).min(1000);
    let ethics_score_permille = (1000 - penalty) as u16;
    ConsciousnessComplianceAudit {
        content_hash,
        agent_id,
        audit_epoch,
        total_evaluations,
        total_violations,
        ethics_score_permille,
        max_stage,
        max_phi_milliphi,
    }
}

// ── Bridge 5: Consciousness → Log (taboo violation event) ────────────────

/// Taboo violation event for ALICE-Log.
///
/// Immutable log entry emitted when the TabooIndex detects a constraint
/// violation at the current developmental stage.
pub struct ConsciousnessLogTabooEvent {
    /// FNV-1a hash over agent_id + timestamp_ns — log key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Violation timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Developmental stage at time of violation (0–3).
    pub stage: u8,
    /// Taboo constraint hash (identifies which taboo was violated).
    pub constraint_hash: u64,
    /// Severity: 0 = Warning, 1 = Violation, 2 = Critical.
    pub severity: u8,
}

/// Build a `ConsciousnessLogTabooEvent`.
#[inline]
#[must_use]
pub fn consciousness_to_log_taboo_event(
    agent_id: u64,
    timestamp_ns: u64,
    stage: u8,
    constraint_hash: u64,
    severity: u8,
) -> ConsciousnessLogTabooEvent {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&agent_id.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ns.to_le_bytes());
    let content_hash = fnv1a(&buf);
    ConsciousnessLogTabooEvent {
        content_hash,
        agent_id,
        timestamp_ns,
        stage,
        constraint_hash,
        severity,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consciousness_db_phi_hash_nonzero() {
        let rec = consciousness_to_db_phi_record(1, 1, 500, 10, 4, 2, 5000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_consciousness_db_phi_deterministic() {
        let a = consciousness_to_db_phi_record(1, 1, 500, 10, 4, 2, 5000);
        let b = consciousness_to_db_phi_record(1, 1, 500, 10, 4, 2, 5000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_consciousness_cache_high_level_ttl() {
        let entry = consciousness_to_cache_state(1, 800, 4, 15);
        assert_eq!(entry.ttl_secs, 5);
    }

    #[test]
    fn test_consciousness_cache_low_level_ttl() {
        let entry = consciousness_to_cache_state(1, 100, 1, 3);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_consciousness_analytics_fields() {
        let m = consciousness_to_analytics_metrics(1, 10, 50, 30, 2, 3, 750);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.evaluations, 50);
        assert_eq!(m.taboo_violations, 2);
    }

    #[test]
    fn test_consciousness_compliance_no_violations() {
        let audit = consciousness_to_compliance_audit(1, 1, 100, 0, 3, 900);
        assert_eq!(audit.ethics_score_permille, 1000);
    }

    #[test]
    fn test_consciousness_compliance_with_violations() {
        let audit = consciousness_to_compliance_audit(1, 1, 100, 3, 2, 500);
        assert_eq!(audit.ethics_score_permille, 700); // 1000 - 3*100
    }

    #[test]
    fn test_consciousness_log_taboo_fields() {
        let ev = consciousness_to_log_taboo_event(7, 1_000_000, 2, 0xdeadbeef, 1);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.stage, 2);
        assert_eq!(ev.severity, 1);
    }

    #[test]
    fn test_consciousness_different_agents_differ() {
        let a = consciousness_to_db_phi_record(1, 1, 500, 10, 4, 2, 5000);
        let b = consciousness_to_db_phi_record(2, 1, 500, 10, 4, 2, 5000);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
