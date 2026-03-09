//! Recommend bridges — ALICE-Recommend ↔ DB, Cache, Analytics, ML, API
//!
//! 5 bridges connecting recommendation engine data (extracted as primitives)
//! to the ALICE ecosystem. No external crate types are imported; all fields
//! use primitive types derived from serialised recommendation state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Recommend → DB (recommendation snapshot persistence) ────────

/// Recommendation snapshot record for ALICE-DB persistence.
pub struct RecommendDbRecord {
    /// Content hash over user_count, item_count, and recommendation_count.
    pub content_hash: u64,
    /// Number of users for whom recommendations were generated.
    pub user_count: u64,
    /// Number of candidate items in the catalogue.
    pub item_count: u64,
    /// Total recommendations produced in this batch.
    pub recommendation_count: u64,
    /// Mean cosine similarity score across all pairs (0.0–1.0).
    pub mean_similarity_score: f64,
    /// Model version identifier hash.
    pub model_version_hash: u64,
    /// Unix timestamp (seconds) when this snapshot was taken.
    pub snapshot_ts: u64,
}

/// Build a DB persistence record from extracted recommendation batch data.
#[inline]
#[must_use]
pub fn recommend_to_db_record(
    user_count: u64,
    item_count: u64,
    recommendation_count: u64,
    mean_similarity_score: f64,
    model_version: &[u8],
    snapshot_ts: u64,
) -> RecommendDbRecord {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&user_count.to_le_bytes());
    buf[8..16].copy_from_slice(&item_count.to_le_bytes());
    buf[16..24].copy_from_slice(&recommendation_count.to_le_bytes());
    RecommendDbRecord {
        content_hash: fnv1a(&buf),
        user_count,
        item_count,
        recommendation_count,
        mean_similarity_score,
        model_version_hash: fnv1a(model_version),
        snapshot_ts,
    }
}

// ── Bridge 2: Recommend → Cache (top-N result caching) ───────────────────

/// Cached top-N recommendation entry for ALICE-Cache.
pub struct RecommendCacheEntry {
    /// Content hash over user_id and top_n.
    pub content_hash: u64,
    /// User identifier (hashed for anonymity).
    pub user_id_hash: u64,
    /// Number of top recommendations stored.
    pub top_n: u32,
    /// TTL in seconds (branchless: shorter when similarity is low).
    pub ttl_secs: u32,
    /// Mean similarity score of the cached recommendations.
    pub mean_similarity_score: f64,
    /// Unix timestamp when this entry was cached.
    pub cached_at: u64,
}

/// Build a cache entry for top-N recommendations.
///
/// TTL is 3600 s by default; reduced to 900 s when `mean_similarity_score`
/// falls below 0.5 (low-confidence recommendations expire faster).
#[inline]
#[must_use]
pub fn recommend_to_cache_entry(
    user_id: &[u8],
    top_n: u32,
    mean_similarity_score: f64,
    cached_at: u64,
) -> RecommendCacheEntry {
    let user_id_hash = fnv1a(user_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&user_id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&top_n.to_le_bytes());
    // Branchless TTL: 3600 - (low_confidence * 2700)
    let low_confidence = (mean_similarity_score < 0.5) as u32;
    let ttl_secs = 3600 - low_confidence * 2700;
    RecommendCacheEntry {
        content_hash: fnv1a(&buf),
        user_id_hash,
        top_n,
        ttl_secs,
        mean_similarity_score,
        cached_at,
    }
}

// ── Bridge 3: Recommend → Analytics (batch metrics ingestion) ─────────────

/// Recommendation batch metrics for ALICE-Analytics ingestion.
pub struct RecommendAnalyticsEvent {
    /// Content hash over user_count and recommendation_count.
    pub content_hash: u64,
    /// Number of users processed in this batch.
    pub user_count: u64,
    /// Total recommendations emitted.
    pub recommendation_count: u64,
    /// Mean similarity score across all recommendations.
    pub mean_similarity_score: f64,
    /// Coverage ratio: distinct items recommended / total item_count (0.0–1.0).
    pub coverage_ratio: f64,
    /// Batch processing duration in microseconds.
    pub duration_us: u64,
}

/// Build an analytics ingestion event from recommendation batch results.
#[inline]
#[must_use]
pub fn recommend_to_analytics_event(
    user_count: u64,
    recommendation_count: u64,
    item_count: u64,
    distinct_items_recommended: u64,
    mean_similarity_score: f64,
    duration_us: u64,
) -> RecommendAnalyticsEvent {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&user_count.to_le_bytes());
    buf[8..16].copy_from_slice(&recommendation_count.to_le_bytes());
    let coverage_ratio = distinct_items_recommended as f64 / item_count.max(1) as f64;
    RecommendAnalyticsEvent {
        content_hash: fnv1a(&buf),
        user_count,
        recommendation_count,
        mean_similarity_score,
        coverage_ratio,
        duration_us,
    }
}

// ── Bridge 4: Recommend → ML (feature vector for model training) ──────────

/// Feature vector extracted from recommendation state for ALICE-ML training.
pub struct RecommendMlFeatures {
    /// Content hash over the feature values.
    pub content_hash: u64,
    /// Number of users (normalisation denominator).
    pub user_count: u64,
    /// Number of candidate items.
    pub item_count: u64,
    /// Mean similarity score.
    pub mean_similarity_score: f64,
    /// Coverage ratio.
    pub coverage_ratio: f64,
    /// Sparsity of the user-item interaction matrix (0.0–1.0).
    pub matrix_sparsity: f64,
    /// Packed feature array [user_count_f64, item_count_f64, similarity, coverage, sparsity].
    pub features: [f64; 5],
}

/// Extract an ML feature vector from recommendation batch statistics.
#[inline]
#[must_use]
pub fn recommend_to_ml_features(
    user_count: u64,
    item_count: u64,
    interaction_count: u64,
    mean_similarity_score: f64,
    coverage_ratio: f64,
) -> RecommendMlFeatures {
    let possible_interactions = (user_count as f64) * (item_count as f64);
    let matrix_sparsity = 1.0 - interaction_count as f64 / possible_interactions.max(1.0);
    let features = [
        user_count as f64,
        item_count as f64,
        mean_similarity_score,
        coverage_ratio,
        matrix_sparsity,
    ];
    let mut buf = [0u8; 40];
    for (i, &v) in features.iter().enumerate() {
        buf[i * 8..(i + 1) * 8].copy_from_slice(&v.to_le_bytes());
    }
    RecommendMlFeatures {
        content_hash: fnv1a(&buf),
        user_count,
        item_count,
        mean_similarity_score,
        coverage_ratio,
        matrix_sparsity,
        features,
    }
}

// ── Bridge 5: Recommend → API (response payload) ─────────────────────────

/// Recommendation API response payload for ALICE-API serialisation.
pub struct RecommendApiPayload {
    /// Content hash over user_id_hash and top_n.
    pub content_hash: u64,
    /// Hashed user identifier.
    pub user_id_hash: u64,
    /// Number of recommendations in the response.
    pub top_n: u32,
    /// Mean similarity score of returned recommendations.
    pub mean_similarity_score: f64,
    /// Whether the result was served from cache.
    pub from_cache: bool,
    /// Response latency in microseconds.
    pub latency_us: u64,
    /// API version tag hash.
    pub api_version_hash: u64,
}

/// Build an API response payload for a recommendation request.
#[inline]
#[must_use]
pub fn recommend_to_api_payload(
    user_id: &[u8],
    top_n: u32,
    mean_similarity_score: f64,
    from_cache: bool,
    latency_us: u64,
    api_version: &[u8],
) -> RecommendApiPayload {
    let user_id_hash = fnv1a(user_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&user_id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&top_n.to_le_bytes());
    RecommendApiPayload {
        content_hash: fnv1a(&buf),
        user_id_hash,
        top_n,
        mean_similarity_score,
        from_cache,
        latency_us,
        api_version_hash: fnv1a(api_version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = recommend_to_db_record(100, 500, 1000, 0.75, b"v1.0", 1_700_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn db_record_fields_preserved() {
        let rec = recommend_to_db_record(42, 200, 840, 0.88, b"model-v2", 9_999_999);
        assert_eq!(rec.user_count, 42);
        assert_eq!(rec.item_count, 200);
        assert_eq!(rec.recommendation_count, 840);
        assert!((rec.mean_similarity_score - 0.88).abs() < f64::EPSILON);
        assert_eq!(rec.snapshot_ts, 9_999_999);
        assert_ne!(rec.model_version_hash, 0);
    }

    #[test]
    fn db_record_hash_is_deterministic() {
        let a = recommend_to_db_record(10, 50, 100, 0.5, b"v1", 0);
        let b = recommend_to_db_record(10, 50, 100, 0.5, b"v1", 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_high_similarity_long_ttl() {
        let entry = recommend_to_cache_entry(b"user-abc", 10, 0.9, 1_000);
        assert_eq!(entry.ttl_secs, 3600);
    }

    #[test]
    fn cache_entry_low_similarity_short_ttl() {
        let entry = recommend_to_cache_entry(b"user-xyz", 10, 0.3, 2_000);
        assert_eq!(entry.ttl_secs, 900);
    }

    #[test]
    fn cache_entry_hash_nonzero() {
        let entry = recommend_to_cache_entry(b"user-123", 5, 0.7, 500);
        assert_ne!(entry.content_hash, 0);
        assert_ne!(entry.user_id_hash, 0);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_coverage_ratio_correct() {
        // 200 distinct items out of 400 total → 0.5
        let ev = recommend_to_analytics_event(50, 500, 400, 200, 0.8, 1_200);
        assert!((ev.coverage_ratio - 0.5).abs() < 1e-9);
        assert_ne!(ev.content_hash, 0);
    }

    // ── ML features tests ─────────────────────────────────────────────────

    #[test]
    fn ml_features_sparsity_range() {
        // 100 interactions / (10 * 100) = 0.1 density → sparsity = 0.9
        let feat = recommend_to_ml_features(10, 100, 100, 0.7, 0.4);
        assert!((feat.matrix_sparsity - 0.9).abs() < 1e-9);
        assert_eq!(feat.features[0], 10.0);
        assert_ne!(feat.content_hash, 0);
    }
}
