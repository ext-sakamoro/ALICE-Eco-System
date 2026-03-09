//! NLP bridges — ALICE-NLP ↔ DB, Cache, Analytics, Search, ML
//!
//! 5 bridges connecting natural language processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: NLP → DB (token corpus record) ─────────────────────────────

/// Token corpus record for ALICE-DB persistence.
pub struct NlpDbRecord {
    /// Content hash over the corpus snapshot.
    pub content_hash: u64,
    /// Number of tokens in the corpus.
    pub token_count: u32,
    /// Vocabulary size.
    pub vocab_size: u32,
    /// Dimensionality of the embedding vectors.
    pub embedding_dim: u16,
    /// Hash of the language identifier.
    pub language_hash: u64,
    /// Total byte size of the corpus.
    pub corpus_bytes: u64,
}

/// Serialize an NLP corpus snapshot for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn nlp_to_db_record(
    token_count: u32,
    vocab_size: u32,
    embedding_dim: u16,
    language_hash: u64,
    corpus_bytes: u64,
) -> NlpDbRecord {
    let mut buf = [0u8; 30];
    buf[0..4].copy_from_slice(&token_count.to_le_bytes());
    buf[4..8].copy_from_slice(&vocab_size.to_le_bytes());
    buf[8..10].copy_from_slice(&embedding_dim.to_le_bytes());
    buf[10..18].copy_from_slice(&language_hash.to_le_bytes());
    buf[18..26].copy_from_slice(&corpus_bytes.to_le_bytes());
    NlpDbRecord {
        content_hash: fnv1a(&buf),
        token_count,
        vocab_size,
        embedding_dim,
        language_hash,
        corpus_bytes,
    }
}

// ── Bridge 2: NLP → Cache (inference result cache) ───────────────────────

/// Inference result cache entry for ALICE-Cache.
pub struct NlpCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of tokens in the cached sequence.
    pub token_count: u32,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Model version that produced the result.
    pub model_version: u32,
    /// Dimensionality of the cached embedding.
    pub embedding_dim: u16,
}

/// Build an inference result cache entry for ALICE-Cache.
///
/// Larger sequences receive a shorter TTL (60 s vs 300 s) because they
/// are cheaper to recompute in chunks than to hold in memory.
#[inline]
#[must_use]
pub fn nlp_to_cache_entry(
    token_count: u32,
    model_version: u32,
    embedding_dim: u16,
) -> NlpCacheEntry {
    let mut buf = [0u8; 10];
    buf[0..4].copy_from_slice(&token_count.to_le_bytes());
    buf[4..8].copy_from_slice(&model_version.to_le_bytes());
    buf[8..10].copy_from_slice(&embedding_dim.to_le_bytes());
    let long_seq = (token_count > 512) as u32;
    let ttl_secs = 300 - long_seq * 240;
    NlpCacheEntry {
        content_hash: fnv1a(&buf),
        token_count,
        ttl_secs,
        model_version,
        embedding_dim,
    }
}

// ── Bridge 3: NLP → Analytics (inference event) ──────────────────────────

/// Inference analytics event for ALICE-Analytics ingestion.
pub struct NlpAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of tokens processed.
    pub token_count: u32,
    /// Inference latency in microseconds.
    pub inference_time_us: u64,
    /// Accuracy in basis points (0–10 000).
    pub accuracy_bps: u16,
    /// Batch size used for inference.
    pub batch_size: u32,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an inference analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn nlp_to_analytics_event(
    token_count: u32,
    inference_time_us: u64,
    accuracy_bps: u16,
    batch_size: u32,
    timestamp_ms: u64,
) -> NlpAnalyticsEvent {
    let mut buf = [0u8; 30];
    buf[0..4].copy_from_slice(&token_count.to_le_bytes());
    buf[4..12].copy_from_slice(&inference_time_us.to_le_bytes());
    buf[12..14].copy_from_slice(&accuracy_bps.to_le_bytes());
    buf[14..18].copy_from_slice(&batch_size.to_le_bytes());
    buf[18..26].copy_from_slice(&timestamp_ms.to_le_bytes());
    NlpAnalyticsEvent {
        content_hash: fnv1a(&buf),
        token_count,
        inference_time_us,
        accuracy_bps,
        batch_size,
        timestamp_ms,
    }
}

// ── Bridge 4: NLP → Search (inverted index entry) ────────────────────────

/// Inverted index entry for ALICE-Search integration.
pub struct NlpSearchIndex {
    /// Content hash over the index snapshot.
    pub content_hash: u64,
    /// Number of indexed documents.
    pub doc_count: u64,
    /// Total byte size of the index on disk.
    pub index_size_bytes: u64,
    /// Number of indexed fields.
    pub field_count: u16,
    /// Shard identifier for distributed search.
    pub shard_id: u32,
}

/// Build an inverted index entry for ALICE-Search.
#[inline]
#[must_use]
pub fn nlp_to_search_index(
    doc_count: u64,
    index_size_bytes: u64,
    field_count: u16,
    shard_id: u32,
) -> NlpSearchIndex {
    let mut buf = [0u8; 22];
    buf[0..8].copy_from_slice(&doc_count.to_le_bytes());
    buf[8..16].copy_from_slice(&index_size_bytes.to_le_bytes());
    buf[16..18].copy_from_slice(&field_count.to_le_bytes());
    buf[18..22].copy_from_slice(&shard_id.to_le_bytes());
    NlpSearchIndex {
        content_hash: fnv1a(&buf),
        doc_count,
        index_size_bytes,
        field_count,
        shard_id,
    }
}

// ── Bridge 5: NLP → ML (feature vector) ──────────────────────────────────

/// Feature vector descriptor for ALICE-ML downstream tasks.
pub struct NlpMlFeatures {
    /// Content hash over the feature snapshot.
    pub content_hash: u64,
    /// Dimensionality of the feature vector.
    pub feature_dim: u32,
    /// Number of training samples.
    pub sample_count: u64,
    /// Sparsity percentage (0–100).
    pub sparsity_pct: u8,
    /// Hash of the upstream model that produced the features.
    pub model_hash: u64,
}

/// Extract feature vectors for ALICE-ML downstream tasks.
#[inline]
#[must_use]
pub fn nlp_to_ml_features(
    feature_dim: u32,
    sample_count: u64,
    sparsity_pct: u8,
    model_hash: u64,
) -> NlpMlFeatures {
    let mut buf = [0u8; 21];
    buf[0..4].copy_from_slice(&feature_dim.to_le_bytes());
    buf[4..12].copy_from_slice(&sample_count.to_le_bytes());
    buf[12] = sparsity_pct;
    buf[13..21].copy_from_slice(&model_hash.to_le_bytes());
    NlpMlFeatures {
        content_hash: fnv1a(&buf),
        feature_dim,
        sample_count,
        sparsity_pct,
        model_hash,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nlp_db_record_hash_nonzero() {
        let rec = nlp_to_db_record(1024, 32_000, 768, 0xdead_beef_cafe_1234, 1_048_576);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_nlp_db_record_fields() {
        let rec = nlp_to_db_record(512, 16_000, 256, 0x1111_2222_3333_4444, 65_536);
        assert_eq!(rec.token_count, 512);
        assert_eq!(rec.vocab_size, 16_000);
        assert_eq!(rec.embedding_dim, 256);
        assert_eq!(rec.corpus_bytes, 65_536);
    }

    #[test]
    fn test_nlp_db_record_determinism() {
        let a = nlp_to_db_record(100, 8_000, 128, 0xaaaa, 4_096);
        let b = nlp_to_db_record(100, 8_000, 128, 0xaaaa, 4_096);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_nlp_cache_entry_short_seq_ttl() {
        let entry = nlp_to_cache_entry(256, 3, 512);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_nlp_cache_entry_long_seq_ttl() {
        let entry = nlp_to_cache_entry(1024, 3, 512);
        assert_eq!(entry.ttl_secs, 60);
        assert_eq!(entry.embedding_dim, 512);
    }

    #[test]
    fn test_nlp_analytics_event() {
        let ev = nlp_to_analytics_event(128, 5_000, 9_500, 32, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.accuracy_bps, 9_500);
        assert_eq!(ev.batch_size, 32);
    }

    #[test]
    fn test_nlp_search_index() {
        let idx = nlp_to_search_index(1_000_000, 536_870_912, 8, 0);
        assert_ne!(idx.content_hash, 0);
        assert_eq!(idx.doc_count, 1_000_000);
        assert_eq!(idx.field_count, 8);
        assert_eq!(idx.shard_id, 0);
    }

    #[test]
    fn test_nlp_ml_features() {
        let f = nlp_to_ml_features(4_096, 50_000, 12, 0xbeef_cafe_1234_5678);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.feature_dim, 4_096);
        assert_eq!(f.sparsity_pct, 12);
    }

    #[test]
    fn test_nlp_ml_features_determinism() {
        let a = nlp_to_ml_features(1024, 10_000, 5, 0x9999);
        let b = nlp_to_ml_features(1024, 10_000, 5, 0x9999);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
