//! RL bridges — ALICE-RL ↔ DB, Cache, Analytics, ML, Monitor
//!
//! 5 bridges connecting reinforcement learning training to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: RL → DB (training log record) ───────────────────────────────

/// Training log record for ALICE-DB persistence.
pub struct RlDbRecord {
    /// Content hash over the training snapshot.
    pub content_hash: u64,
    /// Total number of completed episodes.
    pub episode_count: u64,
    /// Total number of environment steps.
    pub step_count: u64,
    /// Cumulative reward multiplied by 100 (signed).
    pub reward_sum_x100: i64,
    /// Hash of the policy model checkpoint.
    pub model_hash: u64,
    /// Hash of the training environment configuration.
    pub env_hash: u64,
}

/// Serialize an RL training snapshot for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn rl_to_db_record(
    episode_count: u64,
    step_count: u64,
    reward_sum_x100: i64,
    model_hash: u64,
    env_hash: u64,
) -> RlDbRecord {
    let mut buf = [0u8; 40];
    buf[0..8].copy_from_slice(&episode_count.to_le_bytes());
    buf[8..16].copy_from_slice(&step_count.to_le_bytes());
    buf[16..24].copy_from_slice(&reward_sum_x100.to_le_bytes());
    buf[24..32].copy_from_slice(&model_hash.to_le_bytes());
    buf[32..40].copy_from_slice(&env_hash.to_le_bytes());
    RlDbRecord {
        content_hash: fnv1a(&buf),
        episode_count,
        step_count,
        reward_sum_x100,
        model_hash,
        env_hash,
    }
}

// ── Bridge 2: RL → Cache (policy checkpoint cache) ───────────────────────

/// Policy checkpoint cache entry for ALICE-Cache.
pub struct RlCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Byte size of the serialised policy.
    pub policy_bytes: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Model version of the cached policy.
    pub model_version: u32,
    /// Number of episodes completed at this checkpoint.
    pub episode_count: u64,
}

/// Build a policy checkpoint cache entry for ALICE-Cache.
///
/// Mature policies (more episodes) receive a longer TTL (3600 s vs 600 s)
/// because they are more expensive to retrain from scratch.
#[inline]
#[must_use]
pub fn rl_to_cache_entry(
    policy_bytes: u64,
    model_version: u32,
    episode_count: u64,
) -> RlCacheEntry {
    let mut buf = [0u8; 20];
    buf[0..8].copy_from_slice(&policy_bytes.to_le_bytes());
    buf[8..12].copy_from_slice(&model_version.to_le_bytes());
    buf[12..20].copy_from_slice(&episode_count.to_le_bytes());
    let mature = (episode_count >= 1_000) as u32;
    let ttl_secs = 600 + mature * 3_000;
    RlCacheEntry {
        content_hash: fnv1a(&buf),
        policy_bytes,
        ttl_secs,
        model_version,
        episode_count,
    }
}

// ── Bridge 3: RL → Analytics (training metrics event) ────────────────────

/// Training metrics event for ALICE-Analytics ingestion.
pub struct RlAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Total number of completed episodes.
    pub episode_count: u64,
    /// Average reward per episode multiplied by 100 (signed).
    pub avg_reward_x100: i64,
    /// Training step duration in milliseconds.
    pub train_time_ms: u64,
    /// Exploration rate (epsilon) multiplied by 1000.
    pub epsilon_x1000: u32,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a training metrics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn rl_to_analytics_event(
    episode_count: u64,
    avg_reward_x100: i64,
    train_time_ms: u64,
    epsilon_x1000: u32,
    timestamp_ms: u64,
) -> RlAnalyticsEvent {
    let mut buf = [0u8; 36];
    buf[0..8].copy_from_slice(&episode_count.to_le_bytes());
    buf[8..16].copy_from_slice(&avg_reward_x100.to_le_bytes());
    buf[16..24].copy_from_slice(&train_time_ms.to_le_bytes());
    buf[24..28].copy_from_slice(&epsilon_x1000.to_le_bytes());
    buf[28..36].copy_from_slice(&timestamp_ms.to_le_bytes());
    RlAnalyticsEvent {
        content_hash: fnv1a(&buf),
        episode_count,
        avg_reward_x100,
        train_time_ms,
        epsilon_x1000,
        timestamp_ms,
    }
}

// ── Bridge 4: RL → ML (model checkpoint) ─────────────────────────────────

/// Model checkpoint for ALICE-ML experiment tracking.
pub struct RlMlCheckpoint {
    /// Content hash over the checkpoint payload.
    pub content_hash: u64,
    /// Hash of the saved model weights.
    pub model_hash: u64,
    /// Environment steps completed at this checkpoint.
    pub step_count: u64,
    /// Cumulative reward multiplied by 100 (signed).
    pub reward_sum_x100: i64,
    /// Byte size of the checkpoint file.
    pub checkpoint_bytes: u64,
}

/// Build a model checkpoint for ALICE-ML experiment tracking.
#[inline]
#[must_use]
pub fn rl_to_ml_checkpoint(
    model_hash: u64,
    step_count: u64,
    reward_sum_x100: i64,
    checkpoint_bytes: u64,
) -> RlMlCheckpoint {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&model_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&step_count.to_le_bytes());
    buf[16..24].copy_from_slice(&reward_sum_x100.to_le_bytes());
    buf[24..32].copy_from_slice(&checkpoint_bytes.to_le_bytes());
    RlMlCheckpoint {
        content_hash: fnv1a(&buf),
        model_hash,
        step_count,
        reward_sum_x100,
        checkpoint_bytes,
    }
}

// ── Bridge 5: RL → Monitor (convergence status) ───────────────────────────

/// Convergence status report for ALICE-Monitor.
pub struct RlMonitorStatus {
    /// Content hash over the status snapshot.
    pub content_hash: u64,
    /// Total episodes completed.
    pub episode_count: u64,
    /// Average reward per episode multiplied by 100 (signed).
    pub avg_reward_x100: i64,
    /// Whether the policy has converged.
    pub is_converged: bool,
    /// Total training time elapsed in milliseconds.
    pub train_time_ms: u64,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a convergence status report for ALICE-Monitor.
#[inline]
#[must_use]
pub fn rl_to_monitor_status(
    episode_count: u64,
    avg_reward_x100: i64,
    is_converged: bool,
    train_time_ms: u64,
    timestamp_ms: u64,
) -> RlMonitorStatus {
    let mut buf = [0u8; 33];
    buf[0..8].copy_from_slice(&episode_count.to_le_bytes());
    buf[8..16].copy_from_slice(&avg_reward_x100.to_le_bytes());
    buf[16] = is_converged as u8;
    buf[17..25].copy_from_slice(&train_time_ms.to_le_bytes());
    buf[25..33].copy_from_slice(&timestamp_ms.to_le_bytes());
    RlMonitorStatus {
        content_hash: fnv1a(&buf),
        episode_count,
        avg_reward_x100,
        is_converged,
        train_time_ms,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rl_db_record_hash_nonzero() {
        let rec = rl_to_db_record(1_000, 500_000, 125_000, 0x706f_6c69, 0x656e_7668);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_rl_db_record_fields() {
        let rec = rl_to_db_record(500, 250_000, -3_000, 0x1234, 0x5678);
        assert_eq!(rec.episode_count, 500);
        assert_eq!(rec.step_count, 250_000);
        assert_eq!(rec.reward_sum_x100, -3_000);
    }

    #[test]
    fn test_rl_db_record_determinism() {
        let a = rl_to_db_record(200, 100_000, 50_000, 0xaaaa, 0xbbbb);
        let b = rl_to_db_record(200, 100_000, 50_000, 0xaaaa, 0xbbbb);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_rl_cache_entry_early_policy_ttl() {
        let entry = rl_to_cache_entry(1_048_576, 1, 500);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 600);
    }

    #[test]
    fn test_rl_cache_entry_mature_policy_ttl() {
        let entry = rl_to_cache_entry(4_194_304, 3, 5_000);
        assert_eq!(entry.ttl_secs, 3_600);
        assert_eq!(entry.episode_count, 5_000);
    }

    #[test]
    fn test_rl_analytics_event() {
        let ev = rl_to_analytics_event(2_000, 8_750, 45_000, 100, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.epsilon_x1000, 100);
        assert_eq!(ev.avg_reward_x100, 8_750);
    }

    #[test]
    fn test_rl_ml_checkpoint() {
        let ck = rl_to_ml_checkpoint(0x7765_6967, 1_000_000, 920_000, 134_217_728);
        assert_ne!(ck.content_hash, 0);
        assert_eq!(ck.step_count, 1_000_000);
        assert_eq!(ck.checkpoint_bytes, 134_217_728);
    }

    #[test]
    fn test_rl_monitor_status_not_converged() {
        let s = rl_to_monitor_status(100, 1_200, false, 60_000, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(!s.is_converged);
    }

    #[test]
    fn test_rl_monitor_status_converged() {
        let s = rl_to_monitor_status(10_000, 98_500, true, 7_200_000, 1_700_000_001_000);
        assert!(s.is_converged);
        assert_eq!(s.avg_reward_x100, 98_500);
    }
}
