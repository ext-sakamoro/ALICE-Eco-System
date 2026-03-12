//! Autonomy bridges — ALICE-Autonomy ↔ DB, Cache, Analytics, Log, Compliance
//!
//! 5 bridges connecting the Level 5 autonomy manager, KillSwitch, and
//! continual learning subsystem (Project-ALICE V3) to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Autonomy → DB (goal record) ───────────────────────────────

/// Level 5 goal record for ALICE-DB persistence.
///
/// Logs each autonomously generated goal with its status, safety verdict,
/// and the phase in which it was created.
pub struct AutonomyDbGoalRecord {
    /// FNV-1a hash over agent_id + goal_id — row deduplication key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Goal identifier (monotonic within agent).
    pub goal_id: u64,
    /// Phase (0–6) in the 7-phase autonomy lifecycle.
    pub phase: u8,
    /// Goal status: 0 = Pending, 1 = Active, 2 = Completed, 3 = Failed, 4 = Vetoed.
    pub status: u8,
    /// Safety verdict: 0 = Safe, 1 = Unsafe, 2 = Undecidable.
    pub safety_verdict: u8,
    /// Creation timestamp in nanoseconds.
    pub created_ns: u64,
}

/// Build an `AutonomyDbGoalRecord` for goal persistence.
#[inline]
#[must_use]
pub fn autonomy_to_db_goal_record(
    agent_id: u64,
    goal_id: u64,
    phase: u8,
    status: u8,
    safety_verdict: u8,
    created_ns: u64,
) -> AutonomyDbGoalRecord {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&agent_id.to_le_bytes());
    buf[8..16].copy_from_slice(&goal_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    AutonomyDbGoalRecord {
        content_hash,
        agent_id,
        goal_id,
        phase,
        status,
        safety_verdict,
        created_ns,
    }
}

// ── Bridge 2: Autonomy → Cache (phase state cache) ──────────────────────

/// Autonomy phase state cache for ALICE-Cache.
///
/// Caches the current phase and readiness metrics so external monitors
/// can query autonomy status without polling the Level 5 manager.
/// TTL is shorter for higher phases (state changes more rapidly).
pub struct AutonomyCachePhaseState {
    /// FNV-1a hash over agent_id — cache lookup key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Current phase (0–6).
    pub phase: u8,
    /// Kill switch engaged flag.
    pub kill_switch_engaged: bool,
    /// Number of active goals.
    pub active_goals: u32,
    /// Number of completed goals.
    pub completed_goals: u32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build an `AutonomyCachePhaseState` entry.
///
/// TTL: phase >= 4 (high autonomy) → 5 s, otherwise → 30 s (branchless).
#[inline]
#[must_use]
pub fn autonomy_to_cache_phase_state(
    agent_id: u64,
    phase: u8,
    kill_switch_engaged: bool,
    active_goals: u32,
    completed_goals: u32,
) -> AutonomyCachePhaseState {
    let content_hash = fnv1a(&agent_id.to_le_bytes());
    // Branchless TTL: high autonomy phases → 5s, low → 30s.
    let is_high_phase = (phase >= 4) as u32;
    let ttl_secs = 30 - is_high_phase * 25;
    AutonomyCachePhaseState {
        content_hash,
        agent_id,
        phase,
        kill_switch_engaged,
        active_goals,
        completed_goals,
        ttl_secs,
    }
}

// ── Bridge 3: Autonomy → Analytics (autonomy metrics) ────────────────────

/// Autonomy metrics for ALICE-Analytics.
///
/// Tracks Level 5 lifecycle metrics: goals generated, safety checks,
/// kill switch activations, and learning progress.
pub struct AutonomyAnalyticsMetrics {
    /// FNV-1a hash over agent_id + tick — deduplication key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Metric tick.
    pub tick: u64,
    /// Goals generated in this interval.
    pub goals_generated: u32,
    /// Safety checks performed.
    pub safety_checks: u32,
    /// Safety violations detected.
    pub safety_violations: u32,
    /// Kill switch trigger count (cumulative).
    pub kill_switch_triggers: u32,
    /// EWC learning steps completed.
    pub learning_steps: u32,
}

/// Build an `AutonomyAnalyticsMetrics` event for one tick.
#[inline]
#[must_use]
pub fn autonomy_to_analytics_metrics(
    agent_id: u64,
    tick: u64,
    goals_generated: u32,
    safety_checks: u32,
    safety_violations: u32,
    kill_switch_triggers: u32,
    learning_steps: u32,
) -> AutonomyAnalyticsMetrics {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&agent_id.to_le_bytes());
    buf[8..16].copy_from_slice(&tick.to_le_bytes());
    let content_hash = fnv1a(&buf);
    AutonomyAnalyticsMetrics {
        content_hash,
        agent_id,
        tick,
        goals_generated,
        safety_checks,
        safety_violations,
        kill_switch_triggers,
        learning_steps,
    }
}

// ── Bridge 4: Autonomy → Log (kill switch event) ─────────────────────────

/// Kill switch event log for ALICE-Log.
///
/// Immutable audit record emitted when a kill switch is triggered.
/// Includes the reason code and the phase at which shutdown occurred.
pub struct AutonomyLogKillSwitchEvent {
    /// FNV-1a hash over agent_id + timestamp_ns — log key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Trigger timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Phase at time of trigger (0–6).
    pub phase_at_trigger: u8,
    /// Reason code: 0 = Manual, 1 = SafetyViolation, 2 = Timeout, 3 = External.
    pub reason_code: u8,
    /// Number of active goals aborted.
    pub goals_aborted: u32,
}

/// Build an `AutonomyLogKillSwitchEvent` for audit logging.
#[inline]
#[must_use]
pub fn autonomy_to_log_kill_switch_event(
    agent_id: u64,
    timestamp_ns: u64,
    phase_at_trigger: u8,
    reason_code: u8,
    goals_aborted: u32,
) -> AutonomyLogKillSwitchEvent {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&agent_id.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ns.to_le_bytes());
    let content_hash = fnv1a(&buf);
    AutonomyLogKillSwitchEvent {
        content_hash,
        agent_id,
        timestamp_ns,
        phase_at_trigger,
        reason_code,
        goals_aborted,
    }
}

// ── Bridge 5: Autonomy → Compliance (safety report) ─────────────────────

/// Autonomy safety compliance report for ALICE-Compliance.
///
/// Periodic report summarising safety posture for regulatory audit.
pub struct AutonomyComplianceReport {
    /// FNV-1a hash over agent_id + report_epoch — report key.
    pub content_hash: u64,
    /// Agent identifier.
    pub agent_id: u64,
    /// Report epoch (e.g. daily counter).
    pub report_epoch: u64,
    /// Total goals generated in this epoch.
    pub total_goals: u32,
    /// Total safety violations in this epoch.
    pub total_violations: u32,
    /// Kill switch activations in this epoch.
    pub kill_switch_count: u32,
    /// Compliance score in permille (1000 = fully compliant).
    pub compliance_permille: u16,
    /// Maximum phase reached in this epoch (0–6).
    pub max_phase_reached: u8,
}

/// Build an `AutonomyComplianceReport`.
///
/// `compliance_permille` = 1000 - violations * penalty, clamped to [0, 1000] (branchless clamp).
#[inline]
#[must_use]
pub fn autonomy_to_compliance_report(
    agent_id: u64,
    report_epoch: u64,
    total_goals: u32,
    total_violations: u32,
    kill_switch_count: u32,
    max_phase_reached: u8,
) -> AutonomyComplianceReport {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&agent_id.to_le_bytes());
    buf[8..16].copy_from_slice(&report_epoch.to_le_bytes());
    let content_hash = fnv1a(&buf);
    // Compliance score: 1000 - violations * 50, clamped to 0.
    let penalty = total_violations.saturating_mul(50).min(1000);
    let compliance_permille = (1000 - penalty) as u16;
    AutonomyComplianceReport {
        content_hash,
        agent_id,
        report_epoch,
        total_goals,
        total_violations,
        kill_switch_count,
        compliance_permille,
        max_phase_reached,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autonomy_db_goal_hash_nonzero() {
        let rec = autonomy_to_db_goal_record(1, 100, 3, 1, 0, 1_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_autonomy_db_goal_deterministic() {
        let a = autonomy_to_db_goal_record(1, 100, 3, 1, 0, 1_000_000_000);
        let b = autonomy_to_db_goal_record(1, 100, 3, 1, 0, 1_000_000_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_autonomy_cache_high_phase_ttl() {
        let entry = autonomy_to_cache_phase_state(1, 5, false, 3, 10);
        assert_eq!(entry.ttl_secs, 5);
    }

    #[test]
    fn test_autonomy_cache_low_phase_ttl() {
        let entry = autonomy_to_cache_phase_state(1, 2, false, 1, 5);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_autonomy_analytics_metrics_fields() {
        let m = autonomy_to_analytics_metrics(1, 50, 10, 20, 1, 0, 100);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.goals_generated, 10);
        assert_eq!(m.safety_violations, 1);
    }

    #[test]
    fn test_autonomy_log_kill_switch_fields() {
        let ev = autonomy_to_log_kill_switch_event(7, 999_999, 4, 1, 3);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.phase_at_trigger, 4);
        assert_eq!(ev.reason_code, 1);
        assert_eq!(ev.goals_aborted, 3);
    }

    #[test]
    fn test_autonomy_compliance_no_violations() {
        let report = autonomy_to_compliance_report(1, 1, 100, 0, 0, 6);
        assert_eq!(report.compliance_permille, 1000);
    }

    #[test]
    fn test_autonomy_compliance_with_violations() {
        let report = autonomy_to_compliance_report(1, 1, 100, 5, 1, 4);
        assert_eq!(report.compliance_permille, 750); // 1000 - 5*50
    }

    #[test]
    fn test_autonomy_compliance_max_penalty() {
        let report = autonomy_to_compliance_report(1, 1, 100, 100, 5, 6);
        assert_eq!(report.compliance_permille, 0); // clamped
    }
}
