//! Cognitive bridges — ALICE-Cognitive ↔ DB, Cache, Analytics, ML, Search
//!
//! 5 bridges connecting the cognitive reasoning engine (Project-ALICE V3) to
//! the ALICE ecosystem.  Covers reasoning result persistence, inference cache,
//! cognitive metrics, ML feature extraction, and dialogue search indexing.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Cognitive → DB (reasoning result persistence) ─────────────

/// Reasoning result record for ALICE-DB persistence.
///
/// Stores one inference result: the reasoning mode used, confidence score,
/// and provenance metadata for audit trail and replay.
pub struct CognitiveDbReasoningRecord {
    /// FNV-1a hash over query_hash + mode — row deduplication key.
    pub content_hash: u64,
    /// Hash of the original query string.
    pub query_hash: u64,
    /// Reasoning mode: 0 = Causal, 1 = Temporal, 2 = Spatial, 3 = Symbolic.
    pub mode: u8,
    /// Confidence score in permille (0–1000).
    pub confidence_permille: u16,
    /// Number of reasoning steps executed.
    pub step_count: u32,
    /// Wall-clock inference time in microseconds.
    pub inference_time_us: u64,
    /// True if anomaly detector flagged the query.
    pub anomaly_flagged: bool,
}

/// Build a `CognitiveDbReasoningRecord` from reasoning output.
#[inline]
#[must_use]
pub fn cognitive_to_db_reasoning_record(
    query_hash: u64,
    mode: u8,
    confidence_permille: u16,
    step_count: u32,
    inference_time_us: u64,
    anomaly_flagged: bool,
) -> CognitiveDbReasoningRecord {
    let mut buf = [0u8; 9];
    buf[0..8].copy_from_slice(&query_hash.to_le_bytes());
    buf[8] = mode;
    let content_hash = fnv1a(&buf);
    CognitiveDbReasoningRecord {
        content_hash,
        query_hash,
        mode,
        confidence_permille,
        step_count,
        inference_time_us,
        anomaly_flagged,
    }
}

// ── Bridge 2: Cognitive → Cache (inference cache entry) ─────────────────

/// Inference cache entry for ALICE-Cache.
///
/// Caches reasoning results keyed by query hash so identical queries
/// can be served without re-inference.  TTL is shorter for low-confidence
/// results (branchless).
pub struct CognitiveCacheEntry {
    /// FNV-1a hash over query_hash — cache lookup key.
    pub content_hash: u64,
    /// Hash of the original query.
    pub query_hash: u64,
    /// Confidence score in permille.
    pub confidence_permille: u16,
    /// Reasoning mode used.
    pub mode: u8,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Serialised result size in bytes.
    pub result_size_bytes: u32,
}

/// Build a `CognitiveCacheEntry` for one inference result.
///
/// TTL: high-confidence (>= 800 permille) → 300 s, low → 60 s (branchless).
#[inline]
#[must_use]
pub fn cognitive_to_cache_entry(
    query_hash: u64,
    confidence_permille: u16,
    mode: u8,
    result_size_bytes: u32,
) -> CognitiveCacheEntry {
    let content_hash = fnv1a(&query_hash.to_le_bytes());
    // Branchless TTL: high confidence → 300s, low → 60s.
    let is_high = (confidence_permille >= 800) as u32;
    let ttl_secs = 60 + is_high * 240;
    CognitiveCacheEntry {
        content_hash,
        query_hash,
        confidence_permille,
        mode,
        ttl_secs,
        result_size_bytes,
    }
}

// ── Bridge 3: Cognitive → Analytics (reasoning metrics) ──────────────────

/// Cognitive reasoning metrics for ALICE-Analytics.
///
/// Emitted per inference batch to track reasoning throughput, mode
/// distribution, and anomaly detection rates.
pub struct CognitiveAnalyticsMetrics {
    /// FNV-1a hash over session_id + tick — deduplication key.
    pub content_hash: u64,
    /// Session identifier.
    pub session_id: u64,
    /// Metric tick (monotonic counter).
    pub tick: u64,
    /// Number of inferences completed in this interval.
    pub inference_count: u32,
    /// Mean confidence in permille across all inferences.
    pub mean_confidence_permille: u16,
    /// Number of anomalies detected.
    pub anomaly_count: u32,
    /// Mean inference time in microseconds.
    pub mean_inference_time_us: u32,
    /// Dominant reasoning mode in this interval.
    pub dominant_mode: u8,
}

/// Build a `CognitiveAnalyticsMetrics` event for one tick.
#[inline]
#[must_use]
pub fn cognitive_to_analytics_metrics(
    session_id: u64,
    tick: u64,
    inference_count: u32,
    mean_confidence_permille: u16,
    anomaly_count: u32,
    mean_inference_time_us: u32,
    dominant_mode: u8,
) -> CognitiveAnalyticsMetrics {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&session_id.to_le_bytes());
    buf[8..16].copy_from_slice(&tick.to_le_bytes());
    let content_hash = fnv1a(&buf);
    CognitiveAnalyticsMetrics {
        content_hash,
        session_id,
        tick,
        inference_count,
        mean_confidence_permille,
        anomaly_count,
        mean_inference_time_us,
        dominant_mode,
    }
}

// ── Bridge 4: Cognitive → ML (feature extraction for meta-learning) ─────

/// Cognitive feature vector for ALICE-ML meta-learning.
///
/// Extracts reasoning characteristics as numeric features for use in
/// meta-learning models that predict optimal reasoning mode selection.
pub struct CognitiveMlFeatures {
    /// FNV-1a hash over query_hash + mode — feature vector key.
    pub content_hash: u64,
    /// Query hash.
    pub query_hash: u64,
    /// Reasoning mode (0–3).
    pub mode: u8,
    /// Query complexity estimate in permille (0–1000).
    pub complexity_permille: u16,
    /// Step count normalised to permille (0–1000 mapped from 0–max_steps).
    pub step_ratio_permille: u16,
    /// Confidence score in permille.
    pub confidence_permille: u16,
    /// Anomaly score in permille (0 = normal, 1000 = extreme anomaly).
    pub anomaly_score_permille: u16,
}

/// Build a `CognitiveMlFeatures` vector for ML ingestion.
#[inline]
#[must_use]
pub fn cognitive_to_ml_features(
    query_hash: u64,
    mode: u8,
    complexity_permille: u16,
    step_ratio_permille: u16,
    confidence_permille: u16,
    anomaly_score_permille: u16,
) -> CognitiveMlFeatures {
    let mut buf = [0u8; 9];
    buf[0..8].copy_from_slice(&query_hash.to_le_bytes());
    buf[8] = mode;
    let content_hash = fnv1a(&buf);
    CognitiveMlFeatures {
        content_hash,
        query_hash,
        mode,
        complexity_permille,
        step_ratio_permille,
        confidence_permille,
        anomaly_score_permille,
    }
}

// ── Bridge 5: Cognitive → Search (dialogue index entry) ─────────────────

/// Dialogue index entry for ALICE-Search.
///
/// Indexes each dialogue turn for full-text search, enabling retrieval
/// of past conversations by intent, topic, or content.
pub struct CognitiveSearchEntry {
    /// FNV-1a hash over session_id + turn_id — index key.
    pub content_hash: u64,
    /// Dialogue session identifier.
    pub session_id: u64,
    /// Turn number within the session.
    pub turn_id: u32,
    /// Intent code: 0 = Greeting, 1 = Question, 2 = Command, 3 = Statement, 4 = Farewell, 5 = Unknown.
    pub intent_code: u8,
    /// Estimated topic hash for faceted search.
    pub topic_hash: u64,
    /// Turn text length in bytes.
    pub text_len: u32,
}

/// Build a `CognitiveSearchEntry` for dialogue indexing.
#[inline]
#[must_use]
pub fn cognitive_to_search_entry(
    session_id: u64,
    turn_id: u32,
    intent_code: u8,
    topic_hash: u64,
    text_len: u32,
) -> CognitiveSearchEntry {
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&session_id.to_le_bytes());
    buf[8..12].copy_from_slice(&turn_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    CognitiveSearchEntry {
        content_hash,
        session_id,
        turn_id,
        intent_code,
        topic_hash,
        text_len,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cognitive_db_reasoning_hash_nonzero() {
        let rec = cognitive_to_db_reasoning_record(12345, 0, 850, 5, 1000, false);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_cognitive_db_reasoning_deterministic() {
        let a = cognitive_to_db_reasoning_record(12345, 1, 700, 3, 500, true);
        let b = cognitive_to_db_reasoning_record(12345, 1, 700, 3, 500, true);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_cognitive_cache_high_confidence_ttl() {
        let entry = cognitive_to_cache_entry(999, 900, 0, 256);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_cognitive_cache_low_confidence_ttl() {
        let entry = cognitive_to_cache_entry(999, 500, 0, 256);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_cognitive_analytics_metrics_fields() {
        let m = cognitive_to_analytics_metrics(1, 100, 50, 750, 2, 800, 0);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.inference_count, 50);
        assert_eq!(m.dominant_mode, 0);
    }

    #[test]
    fn test_cognitive_ml_features_hash_nonzero() {
        let f = cognitive_to_ml_features(42, 2, 500, 300, 800, 100);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.mode, 2);
    }

    #[test]
    fn test_cognitive_search_entry_fields() {
        let e = cognitive_to_search_entry(10, 3, 1, 77777, 128);
        assert_ne!(e.content_hash, 0);
        assert_eq!(e.turn_id, 3);
        assert_eq!(e.intent_code, 1);
    }

    #[test]
    fn test_cognitive_different_modes_differ() {
        let a = cognitive_to_db_reasoning_record(100, 0, 800, 5, 1000, false);
        let b = cognitive_to_db_reasoning_record(100, 1, 800, 5, 1000, false);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
