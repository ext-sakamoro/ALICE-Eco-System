//! Innovation bridges — ALICE-Innovation ↔ DB, Cache, Analytics, ML, Search
//!
//! 5 bridges connecting the innovation hub, technology designers, and
//! creativity evaluator (Project-ALICE V3) to the ALICE ecosystem.
//! Covers design result persistence, evaluation caching, innovation metrics,
//! ML feature extraction from creativity scores, and design search indexing.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Innovation → DB (design result record) ─────────────────────

/// Design result record for ALICE-DB persistence.
///
/// Stores one technology design proposal: domain, feasibility, novelty,
/// and the creativity rank assigned by the evaluator.
pub struct InnovationDbDesignRecord {
    /// FNV-1a hash over domain_hash + problem_hash — row deduplication key.
    pub content_hash: u64,
    /// Domain name hash (FNV-1a of domain string).
    pub domain_hash: u64,
    /// Problem description hash.
    pub problem_hash: u64,
    /// Feasibility score in permille (0–1000).
    pub feasibility_permille: u16,
    /// Number of design steps generated.
    pub step_count: u32,
    /// Creativity rank: 0 = Mundane, 1 = Interesting, 2 = Creative, 3 = Innovative, 4 = Breakthrough.
    pub creativity_rank: u8,
    /// Overall creativity score in permille (0–1000).
    pub overall_score_permille: u16,
}

/// Build an `InnovationDbDesignRecord`.
#[inline]
#[must_use]
pub fn innovation_to_db_design_record(
    domain_hash: u64,
    problem_hash: u64,
    feasibility_permille: u16,
    step_count: u32,
    creativity_rank: u8,
    overall_score_permille: u16,
) -> InnovationDbDesignRecord {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&domain_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&problem_hash.to_le_bytes());
    let content_hash = fnv1a(&buf);
    InnovationDbDesignRecord {
        content_hash,
        domain_hash,
        problem_hash,
        feasibility_permille,
        step_count,
        creativity_rank,
        overall_score_permille,
    }
}

// ── Bridge 2: Innovation → Cache (evaluation cache) ──────────────────────

/// Creativity evaluation cache for ALICE-Cache.
///
/// Caches the 4-dimensional creativity score for a given design so
/// repeated evaluations can be skipped.  TTL is shorter for highly
/// creative designs (more likely to be refined).
pub struct InnovationCacheEvaluation {
    /// FNV-1a hash over design_hash — cache lookup key.
    pub content_hash: u64,
    /// Design result hash.
    pub design_hash: u64,
    /// Novelty score in permille.
    pub novelty_permille: u16,
    /// Usefulness score in permille.
    pub usefulness_permille: u16,
    /// Surprise score in permille.
    pub surprise_permille: u16,
    /// Elegance score in permille.
    pub elegance_permille: u16,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build an `InnovationCacheEvaluation` entry.
///
/// TTL: overall >= 800 permille → 60 s, otherwise → 300 s (branchless).
/// High-scoring designs get shorter TTL because they're actively iterated.
#[inline]
#[must_use]
pub fn innovation_to_cache_evaluation(
    design_hash: u64,
    novelty_permille: u16,
    usefulness_permille: u16,
    surprise_permille: u16,
    elegance_permille: u16,
) -> InnovationCacheEvaluation {
    let content_hash = fnv1a(&design_hash.to_le_bytes());
    // Overall = simple average for TTL decision.
    let overall = (u32::from(novelty_permille)
        + u32::from(usefulness_permille)
        + u32::from(surprise_permille)
        + u32::from(elegance_permille))
        / 4;
    // Branchless TTL: high creativity → 60s, low → 300s.
    let is_high = (overall >= 800) as u32;
    let ttl_secs = 300 - is_high * 240;
    InnovationCacheEvaluation {
        content_hash,
        design_hash,
        novelty_permille,
        usefulness_permille,
        surprise_permille,
        elegance_permille,
        ttl_secs,
    }
}

// ── Bridge 3: Innovation → Analytics (hub metrics) ───────────────────────

/// Innovation hub metrics for ALICE-Analytics.
///
/// Tracks design throughput, domain distribution, and evaluation statistics.
pub struct InnovationAnalyticsMetrics {
    /// FNV-1a hash over hub_id + tick — deduplication key.
    pub content_hash: u64,
    /// Hub identifier.
    pub hub_id: u64,
    /// Metric tick.
    pub tick: u64,
    /// Number of designs generated in this interval.
    pub designs_generated: u32,
    /// Number of evaluations performed.
    pub evaluations_performed: u32,
    /// Mean overall creativity score in permille.
    pub mean_creativity_permille: u16,
    /// Number of registered domains.
    pub domain_count: u32,
    /// Number of breakthrough-ranked designs.
    pub breakthrough_count: u32,
}

/// Build an `InnovationAnalyticsMetrics` event.
#[inline]
#[must_use]
pub fn innovation_to_analytics_metrics(
    hub_id: u64,
    tick: u64,
    designs_generated: u32,
    evaluations_performed: u32,
    mean_creativity_permille: u16,
    domain_count: u32,
    breakthrough_count: u32,
) -> InnovationAnalyticsMetrics {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&hub_id.to_le_bytes());
    buf[8..16].copy_from_slice(&tick.to_le_bytes());
    let content_hash = fnv1a(&buf);
    InnovationAnalyticsMetrics {
        content_hash,
        hub_id,
        tick,
        designs_generated,
        evaluations_performed,
        mean_creativity_permille,
        domain_count,
        breakthrough_count,
    }
}

// ── Bridge 4: Innovation → ML (creativity feature vector) ────────────────

/// Creativity feature vector for ALICE-ML.
///
/// Extracts the 4-dimensional creativity score as ML features for
/// training creativity prediction and design quality models.
pub struct InnovationMlCreativityFeatures {
    /// FNV-1a hash over design_hash + domain_hash — feature vector key.
    pub content_hash: u64,
    /// Design result hash.
    pub design_hash: u64,
    /// Domain hash.
    pub domain_hash: u64,
    /// Novelty in permille.
    pub novelty_permille: u16,
    /// Usefulness in permille.
    pub usefulness_permille: u16,
    /// Surprise in permille.
    pub surprise_permille: u16,
    /// Elegance in permille.
    pub elegance_permille: u16,
    /// Feasibility in permille.
    pub feasibility_permille: u16,
}

/// Build an `InnovationMlCreativityFeatures` vector.
#[inline]
#[must_use]
pub fn innovation_to_ml_creativity_features(
    design_hash: u64,
    domain_hash: u64,
    novelty_permille: u16,
    usefulness_permille: u16,
    surprise_permille: u16,
    elegance_permille: u16,
    feasibility_permille: u16,
) -> InnovationMlCreativityFeatures {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&design_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&domain_hash.to_le_bytes());
    let content_hash = fnv1a(&buf);
    InnovationMlCreativityFeatures {
        content_hash,
        design_hash,
        domain_hash,
        novelty_permille,
        usefulness_permille,
        surprise_permille,
        elegance_permille,
        feasibility_permille,
    }
}

// ── Bridge 5: Innovation → Search (design index entry) ───────────────────

/// Design index entry for ALICE-Search.
///
/// Indexes each design proposal for full-text search by domain, problem
/// description, and creativity rank.
pub struct InnovationSearchEntry {
    /// FNV-1a hash over domain_hash + problem_hash — index key.
    pub content_hash: u64,
    /// Domain hash.
    pub domain_hash: u64,
    /// Problem description hash.
    pub problem_hash: u64,
    /// Creativity rank (0–4).
    pub creativity_rank: u8,
    /// Overall score in permille.
    pub overall_score_permille: u16,
    /// Number of design steps.
    pub step_count: u32,
}

/// Build an `InnovationSearchEntry`.
#[inline]
#[must_use]
pub fn innovation_to_search_entry(
    domain_hash: u64,
    problem_hash: u64,
    creativity_rank: u8,
    overall_score_permille: u16,
    step_count: u32,
) -> InnovationSearchEntry {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&domain_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&problem_hash.to_le_bytes());
    let content_hash = fnv1a(&buf);
    InnovationSearchEntry {
        content_hash,
        domain_hash,
        problem_hash,
        creativity_rank,
        overall_score_permille,
        step_count,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_innovation_db_design_hash_nonzero() {
        let rec = innovation_to_db_design_record(100, 200, 800, 5, 3, 750);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_innovation_db_design_deterministic() {
        let a = innovation_to_db_design_record(100, 200, 800, 5, 3, 750);
        let b = innovation_to_db_design_record(100, 200, 800, 5, 3, 750);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_innovation_cache_high_creativity_ttl() {
        let entry = innovation_to_cache_evaluation(1, 900, 850, 800, 900);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_innovation_cache_low_creativity_ttl() {
        let entry = innovation_to_cache_evaluation(1, 400, 300, 200, 100);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_innovation_analytics_fields() {
        let m = innovation_to_analytics_metrics(1, 10, 20, 15, 650, 8, 2);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.designs_generated, 20);
        assert_eq!(m.breakthrough_count, 2);
    }

    #[test]
    fn test_innovation_ml_features_fields() {
        let f = innovation_to_ml_creativity_features(42, 99, 800, 700, 600, 500, 900);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.novelty_permille, 800);
        assert_eq!(f.feasibility_permille, 900);
    }

    #[test]
    fn test_innovation_search_entry_fields() {
        let e = innovation_to_search_entry(10, 20, 2, 650, 4);
        assert_ne!(e.content_hash, 0);
        assert_eq!(e.creativity_rank, 2);
    }

    #[test]
    fn test_innovation_different_domains_differ() {
        let a = innovation_to_db_design_record(100, 200, 800, 5, 3, 750);
        let b = innovation_to_db_design_record(999, 200, 800, 5, 3, 750);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
