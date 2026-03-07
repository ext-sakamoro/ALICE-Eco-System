//! Experiment bridges — ALICE-Experiment ↔ DB, Analytics, Cache, ML, Edge
//!
//! 5 bridges connecting A/B experiments, statistical tests, and bandit
//! algorithms to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Experiment → DB (experiment records) ────────────────────────

/// Experiment record for ALICE-DB persistence.
///
/// Captures the definition and current status of an A/B experiment so
/// that analysis queries can reconstruct assignment decisions offline.
pub struct ExperimentDbRecord {
    /// FNV-1a hash over experiment ID and variant list.
    pub content_hash: u64,
    /// Experiment ID hash.
    pub experiment_id_hash: u64,
    /// Total number of variants in this experiment.
    pub variant_count: u32,
    /// True when the experiment is currently active.
    pub is_active: bool,
    /// Number of users assigned so far (approximated by caller).
    pub assigned_users: u64,
    /// Record creation timestamp in milliseconds.
    pub created_at_ms: u64,
}

/// Serialize an experiment definition for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn experiment_to_db_record(
    experiment_id: &str,
    variant_names: &[&str],
    is_active: bool,
    assigned_users: u64,
    created_at_ms: u64,
) -> ExperimentDbRecord {
    let experiment_id_hash = fnv1a(experiment_id.as_bytes());
    // Hash over the concatenated variant names to capture the full definition.
    let mut variant_hash_acc: u64 = 0xcbf29ce484222325;
    for name in variant_names {
        for &b in name.as_bytes() {
            variant_hash_acc ^= b as u64;
            variant_hash_acc = variant_hash_acc.wrapping_mul(0x100000001b3);
        }
        variant_hash_acc ^= 0xff; // separator byte between names
        variant_hash_acc = variant_hash_acc.wrapping_mul(0x100000001b3);
    }
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&experiment_id_hash.to_le_bytes());
    data[8..16].copy_from_slice(&variant_hash_acc.to_le_bytes());
    ExperimentDbRecord {
        content_hash: fnv1a(&data),
        experiment_id_hash,
        variant_count: variant_names.len() as u32,
        is_active,
        assigned_users,
        created_at_ms,
    }
}

// ── Bridge 2: Experiment → Analytics (experiment metrics) ────────────────

/// Experiment metrics payload for ALICE-Analytics.
///
/// Feeds variant conversion rates and statistical significance results
/// into the analytics pipeline for real-time experiment monitoring.
pub struct ExperimentAnalyticsMetrics {
    /// FNV-1a hash over experiment ID and variant name.
    pub content_hash: u64,
    /// Experiment ID hash for analytics stream routing.
    pub experiment_id_hash: u64,
    /// Variant name hash.
    pub variant_hash: u64,
    /// Number of visitors assigned to this variant.
    pub visitors: u64,
    /// Number of conversions observed for this variant.
    pub conversions: u64,
    /// Conversion rate (conversions / visitors); 0.0 when visitors is zero.
    pub conversion_rate: f64,
    /// Z-test statistic against the control variant (caller-computed).
    pub z_statistic: f64,
    /// True when the result is statistically significant at alpha=0.05.
    pub is_significant: bool,
}

/// Build an experiment metrics payload for ALICE-Analytics.
///
/// `conversion_rate` is computed branchlessly with a reciprocal multiply.
#[inline]
#[must_use]
pub fn experiment_to_analytics_metrics(
    experiment_id: &str,
    variant_name: &str,
    visitors: u64,
    conversions: u64,
    z_statistic: f64,
    alpha: f64,
) -> ExperimentAnalyticsMetrics {
    let experiment_id_hash = fnv1a(experiment_id.as_bytes());
    let variant_hash = fnv1a(variant_name.as_bytes());
    let rcp_visitors = 1.0 / visitors.max(1) as f64;
    let conversion_rate = conversions as f64 * rcp_visitors;
    // Significance threshold: |z| > 1.959964 for alpha=0.05 (two-tailed).
    // Use caller-supplied alpha mapped to z-critical via lookup is unnecessary
    // here — the caller provides z_statistic; we derive significance from alpha.
    let z_critical = if (alpha - 0.05).abs() < 1e-9 {
        1.959_964_f64
    } else if (alpha - 0.01).abs() < 1e-9 {
        2.575_829_f64
    } else {
        1.644_854_f64 // alpha=0.10 default
    };
    let is_significant = z_statistic.abs() > z_critical;
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&experiment_id_hash.to_le_bytes());
    data[8..16].copy_from_slice(&variant_hash.to_le_bytes());
    ExperimentAnalyticsMetrics {
        content_hash: fnv1a(&data),
        experiment_id_hash,
        variant_hash,
        visitors,
        conversions,
        conversion_rate,
        z_statistic,
        is_significant,
    }
}

// ── Bridge 3: Experiment → Cache (assignment cache) ───────────────────────

/// Experiment variant assignment cache entry for ALICE-Cache.
///
/// Caches a user's variant assignment so that repeated page loads do not
/// re-compute the assignment hash and risk exposing the user to a different
/// variant.  TTL is computed branchlessly based on whether the experiment is
/// still active.
pub struct ExperimentCacheAssignment {
    /// FNV-1a hash over experiment ID and user ID — cache key.
    pub content_hash: u64,
    /// Assigned variant name hash.
    pub variant_hash: u64,
    /// Cache TTL in seconds (branchless: longer for active experiments).
    pub ttl_secs: u32,
    /// True when the experiment was active at assignment time.
    pub experiment_active: bool,
}

/// Build an experiment assignment cache entry for ALICE-Cache.
///
/// Active experiments get a 3600 s TTL; inactive ones get 60 s.
/// The TTL selection is branchless via integer arithmetic.
#[inline]
#[must_use]
pub fn experiment_to_cache_assignment(
    experiment_id: &str,
    user_id: &str,
    variant_name: &str,
    experiment_active: bool,
) -> ExperimentCacheAssignment {
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&fnv1a(experiment_id.as_bytes()).to_le_bytes());
    data[8..16].copy_from_slice(&fnv1a(user_id.as_bytes()).to_le_bytes());
    let content_hash = fnv1a(&data);
    let variant_hash = fnv1a(variant_name.as_bytes());
    // Branchless TTL: active → 3600 s, inactive → 60 s.
    let active = experiment_active as u32;
    let ttl_secs = 60u32 + active * 3540u32;
    ExperimentCacheAssignment {
        content_hash,
        variant_hash,
        ttl_secs,
        experiment_active,
    }
}

// ── Bridge 4: Experiment → ML (bandit model data) ────────────────────────

/// Bandit arm state for ALICE-ML model training and inference.
///
/// Exposes per-arm success/failure counts and the expected reward so that
/// the ML layer can incorporate bandit outcomes into offline model updates.
pub struct ExperimentMlBanditRecord {
    /// FNV-1a hash over experiment ID and arm name.
    pub content_hash: u64,
    /// Experiment ID hash.
    pub experiment_id_hash: u64,
    /// Arm name hash.
    pub arm_name_hash: u64,
    /// Cumulative successes for this arm.
    pub successes: u64,
    /// Cumulative failures for this arm.
    pub failures: u64,
    /// Expected reward: successes / (successes + failures); 0.5 when no data.
    pub expected_reward: f64,
    /// UCB1 index value for arm selection (caller-computed; 0.0 if unused).
    pub ucb1_index: f64,
}

/// Build a bandit arm record for ALICE-ML model training.
///
/// `expected_reward` uses a reciprocal multiply to avoid division.
#[inline]
#[must_use]
pub fn experiment_to_ml_bandit_record(
    experiment_id: &str,
    arm_name: &str,
    successes: u64,
    failures: u64,
    ucb1_index: f64,
) -> ExperimentMlBanditRecord {
    let experiment_id_hash = fnv1a(experiment_id.as_bytes());
    let arm_name_hash = fnv1a(arm_name.as_bytes());
    let total = successes + failures;
    let rcp_total = 1.0 / total.max(1) as f64;
    // When no observations exist fall back to 0.5 (Laplace prior midpoint).
    let expected_reward = if total == 0 {
        0.5
    } else {
        successes as f64 * rcp_total
    };
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&experiment_id_hash.to_le_bytes());
    data[8..16].copy_from_slice(&arm_name_hash.to_le_bytes());
    ExperimentMlBanditRecord {
        content_hash: fnv1a(&data),
        experiment_id_hash,
        arm_name_hash,
        successes,
        failures,
        expected_reward,
        ucb1_index,
    }
}

// ── Bridge 5: Experiment → Edge (experiment events) ───────────────────────

/// Compact experiment event payload for ALICE-Edge.
///
/// Edge nodes emit lightweight assignment and conversion events so the
/// central experiment platform can track outcomes without full attribute sets.
pub struct ExperimentEdgeEvent {
    /// FNV-1a hash over experiment ID, user ID, and event type.
    pub content_hash: u64,
    /// Experiment ID hash.
    pub experiment_id_hash: u64,
    /// User ID hash (privacy-preserving; original ID is not transmitted).
    pub user_id_hash: u64,
    /// Variant name hash.
    pub variant_hash: u64,
    /// Event type: 0=assignment, 1=conversion, 2=exposure.
    pub event_type: u8,
    /// Event timestamp in milliseconds.
    pub event_at_ms: u64,
    /// Estimated wire size in bytes.
    pub wire_bytes: usize,
}

/// Build a compact experiment event payload for ALICE-Edge.
#[inline]
#[must_use]
pub fn experiment_to_edge_event(
    experiment_id: &str,
    user_id: &str,
    variant_name: &str,
    event_type: u8,
    event_at_ms: u64,
) -> ExperimentEdgeEvent {
    let experiment_id_hash = fnv1a(experiment_id.as_bytes());
    let user_id_hash = fnv1a(user_id.as_bytes());
    let variant_hash = fnv1a(variant_name.as_bytes());
    let mut data = [0u8; 25];
    data[0..8].copy_from_slice(&experiment_id_hash.to_le_bytes());
    data[8..16].copy_from_slice(&user_id_hash.to_le_bytes());
    data[16..24].copy_from_slice(&variant_hash.to_le_bytes());
    data[24] = event_type.min(2);
    ExperimentEdgeEvent {
        content_hash: fnv1a(&data),
        experiment_id_hash,
        user_id_hash,
        variant_hash,
        event_type: event_type.min(2),
        event_at_ms,
        // 8 experiment + 8 user + 8 variant + 1 type + 8 timestamp = 33 bytes.
        wire_bytes: 33,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experiment_to_db_record_content_hash_nonzero() {
        let rec = experiment_to_db_record(
            "exp-checkout-v2",
            &["control", "variant_a", "variant_b"],
            true,
            10_000,
            1_700_000_000_000,
        );
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.experiment_id_hash, 0);
        assert_eq!(rec.variant_count, 3);
        assert!(rec.is_active);
    }

    #[test]
    fn test_experiment_to_db_record_hash_determinism() {
        let a = experiment_to_db_record("exp-x", &["ctrl", "v1"], true, 0, 1_000);
        let b = experiment_to_db_record("exp-x", &["ctrl", "v1"], true, 0, 1_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_experiment_to_analytics_metrics_conversion_rate() {
        // 100 visitors, 25 conversions → 0.25 rate.
        let m = experiment_to_analytics_metrics("exp-1", "variant_a", 100, 25, 2.1, 0.05);
        assert_ne!(m.content_hash, 0);
        assert!((m.conversion_rate - 0.25).abs() < 1e-9, "rate={}", m.conversion_rate);
        assert!(m.is_significant, "z=2.1 should be significant at alpha=0.05");
    }

    #[test]
    fn test_experiment_to_analytics_metrics_zero_visitors_no_panic() {
        let m = experiment_to_analytics_metrics("exp-2", "control", 0, 0, 0.0, 0.05);
        assert_eq!(m.visitors, 0);
        assert!((m.conversion_rate - 0.0).abs() < 1e-9);
        assert!(!m.is_significant);
    }

    #[test]
    fn test_experiment_to_cache_assignment_active_ttl() {
        let entry = experiment_to_cache_assignment("exp-3", "user-42", "variant_b", true);
        assert_ne!(entry.content_hash, 0);
        assert_ne!(entry.variant_hash, 0);
        // active → ttl = 60 + 3540 = 3600
        assert_eq!(entry.ttl_secs, 3600);
        assert!(entry.experiment_active);
    }

    #[test]
    fn test_experiment_to_cache_assignment_inactive_ttl() {
        let entry = experiment_to_cache_assignment("exp-old", "user-99", "control", false);
        // inactive → ttl = 60
        assert_eq!(entry.ttl_secs, 60);
        assert!(!entry.experiment_active);
    }

    #[test]
    fn test_experiment_to_ml_bandit_record_expected_reward() {
        // 80 successes, 20 failures → expected_reward = 0.8.
        let rec = experiment_to_ml_bandit_record("bandit-1", "arm-a", 80, 20, 1.23);
        assert_ne!(rec.content_hash, 0);
        assert!((rec.expected_reward - 0.8).abs() < 1e-9, "reward={}", rec.expected_reward);
        assert!((rec.ucb1_index - 1.23).abs() < 1e-9);
    }

    #[test]
    fn test_experiment_to_ml_bandit_record_zero_observations_prior() {
        // No observations → Laplace prior midpoint 0.5.
        let rec = experiment_to_ml_bandit_record("bandit-2", "arm-b", 0, 0, 0.0);
        assert!((rec.expected_reward - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_experiment_to_edge_event_wire_bytes() {
        let ev = experiment_to_edge_event("exp-edge", "user-7", "ctrl", 1, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.event_type, 1);
        assert_eq!(ev.wire_bytes, 33);
        assert_eq!(ev.event_at_ms, 1_700_000_000_000);
    }
}
