//! Cognitive swarm bridges — Project-ALICE Swarm ↔ DB, Cache, Analytics, ML, Log
//!
//! 5 bridges connecting the cognitive multi-agent swarm layer (Project-ALICE V3
//! SwarmManager, PSO, Voting, CollectiveIntelligence) to the ALICE ecosystem.
//! Distinct from `bridge_swarm` which covers the Eco-System drone swarm crate.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: CognitiveSwarm → DB (voting result record) ─────────────────

/// Voting result record for ALICE-DB persistence.
///
/// Logs each completed vote with its outcome, quorum status, and
/// voting method used.
pub struct CognitiveSwarmDbVoteRecord {
    /// FNV-1a hash over session_id + vote_id — row deduplication key.
    pub content_hash: u64,
    /// Voting session identifier.
    pub session_id: u64,
    /// Vote identifier (monotonic within session).
    pub vote_id: u64,
    /// Vote type: 0 = Majority, 1 = Weighted, 2 = ConfidenceWeighted.
    pub vote_type: u8,
    /// Number of votes cast.
    pub total_votes: u32,
    /// Quorum reached flag.
    pub quorum_reached: bool,
    /// Winning option hash.
    pub winner_hash: u64,
    /// Winner vote count or weight (permille of total).
    pub winner_weight_permille: u16,
}

/// Build a `CognitiveSwarmDbVoteRecord`.
#[inline]
#[must_use]
pub fn cognitive_swarm_to_db_vote_record(
    session_id: u64,
    vote_id: u64,
    vote_type: u8,
    total_votes: u32,
    quorum_reached: bool,
    winner_hash: u64,
    winner_weight_permille: u16,
) -> CognitiveSwarmDbVoteRecord {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&session_id.to_le_bytes());
    buf[8..16].copy_from_slice(&vote_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    CognitiveSwarmDbVoteRecord {
        content_hash,
        session_id,
        vote_id,
        vote_type,
        total_votes,
        quorum_reached,
        winner_hash,
        winner_weight_permille,
    }
}

// ── Bridge 2: CognitiveSwarm → Cache (PSO state snapshot) ────────────────

/// PSO optimiser state cache for ALICE-Cache.
///
/// Caches the current best position and fitness of a running PSO instance.
/// TTL is shorter when fitness is still improving rapidly.
pub struct CognitiveSwarmCachePsoState {
    /// FNV-1a hash over optimizer_id — cache lookup key.
    pub content_hash: u64,
    /// Optimiser identifier.
    pub optimizer_id: u64,
    /// Current iteration count.
    pub iteration: u32,
    /// Number of particles.
    pub particle_count: u32,
    /// Best fitness (Q16.16 fixed-point — lower is better for minimisation).
    pub best_fitness_q16: i64,
    /// Number of dimensions.
    pub dimensions: u32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build a `CognitiveSwarmCachePsoState` entry.
///
/// TTL: early iterations (< 50) → 5 s (fast-changing), later → 30 s (branchless).
#[inline]
#[must_use]
pub fn cognitive_swarm_to_cache_pso_state(
    optimizer_id: u64,
    iteration: u32,
    particle_count: u32,
    best_fitness_q16: i64,
    dimensions: u32,
) -> CognitiveSwarmCachePsoState {
    let content_hash = fnv1a(&optimizer_id.to_le_bytes());
    // Branchless TTL: early iterations → 5s, converged → 30s.
    let is_early = (iteration < 50) as u32;
    let ttl_secs = 30 - is_early * 25;
    CognitiveSwarmCachePsoState {
        content_hash,
        optimizer_id,
        iteration,
        particle_count,
        best_fitness_q16,
        dimensions,
        ttl_secs,
    }
}

// ── Bridge 3: CognitiveSwarm → Analytics (swarm manager metrics) ─────────

/// Cognitive swarm manager metrics for ALICE-Analytics.
///
/// Tracks agent coordination metrics: message throughput, task assignments,
/// collective knowledge contributions, and load balancer statistics.
pub struct CognitiveSwarmAnalyticsMetrics {
    /// FNV-1a hash over swarm_id + tick — deduplication key.
    pub content_hash: u64,
    /// Swarm manager identifier.
    pub swarm_id: u64,
    /// Metric tick.
    pub tick: u64,
    /// Number of registered agents.
    pub agent_count: u32,
    /// Messages sent in this interval.
    pub messages_sent: u32,
    /// Tasks assigned in this interval.
    pub tasks_assigned: u32,
    /// Tasks completed in this interval.
    pub tasks_completed: u32,
    /// Knowledge contributions in this interval.
    pub knowledge_contributions: u32,
}

/// Build a `CognitiveSwarmAnalyticsMetrics` event.
#[inline]
#[must_use]
pub fn cognitive_swarm_to_analytics_metrics(
    swarm_id: u64,
    tick: u64,
    agent_count: u32,
    messages_sent: u32,
    tasks_assigned: u32,
    tasks_completed: u32,
    knowledge_contributions: u32,
) -> CognitiveSwarmAnalyticsMetrics {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&swarm_id.to_le_bytes());
    buf[8..16].copy_from_slice(&tick.to_le_bytes());
    let content_hash = fnv1a(&buf);
    CognitiveSwarmAnalyticsMetrics {
        content_hash,
        swarm_id,
        tick,
        agent_count,
        messages_sent,
        tasks_assigned,
        tasks_completed,
        knowledge_contributions,
    }
}

// ── Bridge 4: CognitiveSwarm → ML (collective knowledge features) ────────

/// Collective knowledge feature vector for ALICE-ML.
///
/// Extracts aggregated knowledge characteristics as features for
/// meta-learning and knowledge quality models.
pub struct CognitiveSwarmMlKnowledgeFeatures {
    /// FNV-1a hash over swarm_id + topic_hash — feature vector key.
    pub content_hash: u64,
    /// Swarm identifier.
    pub swarm_id: u64,
    /// Topic hash.
    pub topic_hash: u64,
    /// Number of knowledge entries contributed.
    pub entry_count: u32,
    /// Mean confidence in permille (0–1000).
    pub mean_confidence_permille: u16,
    /// Confidence variance in permille-squared.
    pub confidence_variance_permille2: u32,
    /// Aggregation strategy: 0 = HighestConfidence, 1 = Majority, 2 = WeightedAverage.
    pub strategy: u8,
}

/// Build a `CognitiveSwarmMlKnowledgeFeatures` vector.
#[inline]
#[must_use]
pub fn cognitive_swarm_to_ml_knowledge_features(
    swarm_id: u64,
    topic_hash: u64,
    entry_count: u32,
    mean_confidence_permille: u16,
    confidence_variance_permille2: u32,
    strategy: u8,
) -> CognitiveSwarmMlKnowledgeFeatures {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&swarm_id.to_le_bytes());
    buf[8..16].copy_from_slice(&topic_hash.to_le_bytes());
    let content_hash = fnv1a(&buf);
    CognitiveSwarmMlKnowledgeFeatures {
        content_hash,
        swarm_id,
        topic_hash,
        entry_count,
        mean_confidence_permille,
        confidence_variance_permille2,
        strategy,
    }
}

// ── Bridge 5: CognitiveSwarm → Log (load balancer event) ─────────────────

/// Load balancer assignment event for ALICE-Log.
///
/// Logs each task assignment decision for audit and performance analysis.
pub struct CognitiveSwarmLogBalancerEvent {
    /// FNV-1a hash over swarm_id + task_id — log key.
    pub content_hash: u64,
    /// Swarm identifier.
    pub swarm_id: u64,
    /// Task identifier.
    pub task_id: u64,
    /// Assigned agent identifier (FNV-1a hash of agent name).
    pub assigned_agent_hash: u64,
    /// Balancing strategy: 0 = RoundRobin, 1 = LeastLoaded, 2 = CapabilityBased.
    pub strategy: u8,
    /// Agent load after assignment in permille (0–1000).
    pub agent_load_permille: u16,
}

/// Build a `CognitiveSwarmLogBalancerEvent`.
#[inline]
#[must_use]
pub fn cognitive_swarm_to_log_balancer_event(
    swarm_id: u64,
    task_id: u64,
    assigned_agent_hash: u64,
    strategy: u8,
    agent_load_permille: u16,
) -> CognitiveSwarmLogBalancerEvent {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&swarm_id.to_le_bytes());
    buf[8..16].copy_from_slice(&task_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    CognitiveSwarmLogBalancerEvent {
        content_hash,
        swarm_id,
        task_id,
        assigned_agent_hash,
        strategy,
        agent_load_permille,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cognitive_swarm_db_vote_hash_nonzero() {
        let rec = cognitive_swarm_to_db_vote_record(1, 10, 0, 5, true, 42, 600);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_cognitive_swarm_db_vote_deterministic() {
        let a = cognitive_swarm_to_db_vote_record(1, 10, 0, 5, true, 42, 600);
        let b = cognitive_swarm_to_db_vote_record(1, 10, 0, 5, true, 42, 600);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_cognitive_swarm_cache_early_ttl() {
        let entry = cognitive_swarm_to_cache_pso_state(1, 10, 20, -100, 3);
        assert_eq!(entry.ttl_secs, 5);
    }

    #[test]
    fn test_cognitive_swarm_cache_converged_ttl() {
        let entry = cognitive_swarm_to_cache_pso_state(1, 200, 20, -100, 3);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_cognitive_swarm_analytics_fields() {
        let m = cognitive_swarm_to_analytics_metrics(1, 50, 10, 100, 20, 15, 5);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.agent_count, 10);
        assert_eq!(m.tasks_assigned, 20);
    }

    #[test]
    fn test_cognitive_swarm_ml_features_fields() {
        let f = cognitive_swarm_to_ml_knowledge_features(1, 0xbeef, 30, 750, 1000, 2);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.entry_count, 30);
        assert_eq!(f.strategy, 2);
    }

    #[test]
    fn test_cognitive_swarm_log_balancer_fields() {
        let ev = cognitive_swarm_to_log_balancer_event(1, 42, 0xdead, 1, 500);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.strategy, 1);
        assert_eq!(ev.agent_load_permille, 500);
    }

    #[test]
    fn test_cognitive_swarm_different_sessions_differ() {
        let a = cognitive_swarm_to_db_vote_record(1, 10, 0, 5, true, 42, 600);
        let b = cognitive_swarm_to_db_vote_record(2, 10, 0, 5, true, 42, 600);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
