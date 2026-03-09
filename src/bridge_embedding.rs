//! Embedding bridges — ALICE-Embedding ↔ DB, Cache, VectorDB, Analytics, ML
//!
//! 5 bridges connecting dense vector embedding pipelines to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Embedding → DB (vector store record) ───────────────────────

/// Vector store record for ALICE-DB persistence.
pub struct EmbeddingDbRecord {
    /// Content hash over the vector snapshot.
    pub content_hash: u64,
    /// Dimensionality of each embedding vector.
    pub dim: u32,
    /// Total number of stored vectors.
    pub vector_count: u64,
    /// Hash of the encoder model.
    pub model_hash: u64,
    /// Numeric precision identifier (e.g. 16 = fp16, 32 = fp32).
    pub precision: u8,
    /// Total byte size of the stored vectors.
    pub storage_bytes: u64,
}

/// Serialize an embedding vector store for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn embedding_to_db_record(
    dim: u32,
    vector_count: u64,
    model_hash: u64,
    precision: u8,
    storage_bytes: u64,
) -> EmbeddingDbRecord {
    let mut buf = [0u8; 29];
    buf[0..4].copy_from_slice(&dim.to_le_bytes());
    buf[4..12].copy_from_slice(&vector_count.to_le_bytes());
    buf[12..20].copy_from_slice(&model_hash.to_le_bytes());
    buf[20] = precision;
    buf[21..29].copy_from_slice(&storage_bytes.to_le_bytes());
    EmbeddingDbRecord {
        content_hash: fnv1a(&buf),
        dim,
        vector_count,
        model_hash,
        precision,
        storage_bytes,
    }
}

// ── Bridge 2: Embedding → Cache (query result cache) ─────────────────────

/// Query result cache entry for ALICE-Cache.
pub struct EmbeddingCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Dimensionality of the cached embedding.
    pub dim: u32,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Number of vectors in the cached result set.
    pub vector_count: u64,
    /// Byte size of the cached result.
    pub cache_bytes: u64,
}

/// Build a query result cache entry for ALICE-Cache.
///
/// High-dimensional embeddings receive a shorter TTL (120 s vs 600 s)
/// because they consume more memory per entry.
#[inline]
#[must_use]
pub fn embedding_to_cache_entry(
    dim: u32,
    vector_count: u64,
    cache_bytes: u64,
) -> EmbeddingCacheEntry {
    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&dim.to_le_bytes());
    buf[4..12].copy_from_slice(&vector_count.to_le_bytes());
    buf[12..20].copy_from_slice(&cache_bytes.to_le_bytes());
    let high_dim = (dim > 1024) as u32;
    let ttl_secs = 600 - high_dim * 480;
    EmbeddingCacheEntry {
        content_hash: fnv1a(&buf),
        dim,
        ttl_secs,
        vector_count,
        cache_bytes,
    }
}

// ── Bridge 3: Embedding → VectorDB (index entry) ─────────────────────────

/// Vector index entry for ALICE-VectorDB ingestion.
pub struct EmbeddingVectorDbEntry {
    /// Content hash over the index entry.
    pub content_hash: u64,
    /// Dimensionality of each indexed vector.
    pub dim: u32,
    /// Hash of the index type (e.g. HNSW, IVF).
    pub index_type_hash: u64,
    /// Number of vectors in this index shard.
    pub vector_count: u64,
    /// Shard identifier for distributed retrieval.
    pub shard_id: u32,
}

/// Build a vector index entry for ALICE-VectorDB.
#[inline]
#[must_use]
pub fn embedding_to_vectordb_entry(
    dim: u32,
    index_type_hash: u64,
    vector_count: u64,
    shard_id: u32,
) -> EmbeddingVectorDbEntry {
    let mut buf = [0u8; 24];
    buf[0..4].copy_from_slice(&dim.to_le_bytes());
    buf[4..12].copy_from_slice(&index_type_hash.to_le_bytes());
    buf[12..20].copy_from_slice(&vector_count.to_le_bytes());
    buf[20..24].copy_from_slice(&shard_id.to_le_bytes());
    EmbeddingVectorDbEntry {
        content_hash: fnv1a(&buf),
        dim,
        index_type_hash,
        vector_count,
        shard_id,
    }
}

// ── Bridge 4: Embedding → Analytics (query event) ────────────────────────

/// Query analytics event for ALICE-Analytics ingestion.
pub struct EmbeddingAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Dimensionality of the query vector.
    pub dim: u32,
    /// Query latency in microseconds.
    pub query_time_us: u64,
    /// Recall in basis points (0–10 000).
    pub recall_bps: u16,
    /// Batch size used for the query.
    pub batch_size: u32,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a query analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn embedding_to_analytics_event(
    dim: u32,
    query_time_us: u64,
    recall_bps: u16,
    batch_size: u32,
    timestamp_ms: u64,
) -> EmbeddingAnalyticsEvent {
    let mut buf = [0u8; 30];
    buf[0..4].copy_from_slice(&dim.to_le_bytes());
    buf[4..12].copy_from_slice(&query_time_us.to_le_bytes());
    buf[12..14].copy_from_slice(&recall_bps.to_le_bytes());
    buf[14..18].copy_from_slice(&batch_size.to_le_bytes());
    buf[18..26].copy_from_slice(&timestamp_ms.to_le_bytes());
    EmbeddingAnalyticsEvent {
        content_hash: fnv1a(&buf),
        dim,
        query_time_us,
        recall_bps,
        batch_size,
        timestamp_ms,
    }
}

// ── Bridge 5: Embedding → ML (pipeline descriptor) ───────────────────────

/// Pipeline descriptor for ALICE-ML embedding fine-tuning.
pub struct EmbeddingMlPipeline {
    /// Content hash over the pipeline configuration.
    pub content_hash: u64,
    /// Dimensionality of the input token representation.
    pub input_dim: u32,
    /// Dimensionality of the output embedding.
    pub output_dim: u32,
    /// Hash of the backbone model.
    pub model_hash: u64,
    /// Whether the model weights are quantized.
    pub quantized: bool,
}

/// Build a pipeline descriptor for ALICE-ML embedding fine-tuning.
#[inline]
#[must_use]
pub fn embedding_to_ml_pipeline(
    input_dim: u32,
    output_dim: u32,
    model_hash: u64,
    quantized: bool,
) -> EmbeddingMlPipeline {
    let mut buf = [0u8; 17];
    buf[0..4].copy_from_slice(&input_dim.to_le_bytes());
    buf[4..8].copy_from_slice(&output_dim.to_le_bytes());
    buf[8..16].copy_from_slice(&model_hash.to_le_bytes());
    buf[16] = quantized as u8;
    EmbeddingMlPipeline {
        content_hash: fnv1a(&buf),
        input_dim,
        output_dim,
        model_hash,
        quantized,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_db_record_hash_nonzero() {
        let rec = embedding_to_db_record(768, 1_000_000, 0xdead_cafe, 32, 3_072_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_embedding_db_record_fields() {
        let rec = embedding_to_db_record(512, 500_000, 0x1234, 16, 512_000_000);
        assert_eq!(rec.dim, 512);
        assert_eq!(rec.vector_count, 500_000);
        assert_eq!(rec.precision, 16);
        assert_eq!(rec.storage_bytes, 512_000_000);
    }

    #[test]
    fn test_embedding_db_record_determinism() {
        let a = embedding_to_db_record(256, 100_000, 0xbeef, 32, 102_400_000);
        let b = embedding_to_db_record(256, 100_000, 0xbeef, 32, 102_400_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_embedding_cache_entry_low_dim_ttl() {
        let entry = embedding_to_cache_entry(512, 64, 262_144);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 600);
    }

    #[test]
    fn test_embedding_cache_entry_high_dim_ttl() {
        let entry = embedding_to_cache_entry(2048, 64, 1_048_576);
        assert_eq!(entry.ttl_secs, 120);
        assert_eq!(entry.dim, 2048);
    }

    #[test]
    fn test_embedding_vectordb_entry() {
        let e = embedding_to_vectordb_entry(768, 0x4e53_5748, 250_000, 3);
        assert_ne!(e.content_hash, 0);
        assert_eq!(e.shard_id, 3);
        assert_eq!(e.vector_count, 250_000);
    }

    #[test]
    fn test_embedding_analytics_event() {
        let ev = embedding_to_analytics_event(768, 1_200, 9_800, 128, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.recall_bps, 9_800);
        assert_eq!(ev.batch_size, 128);
    }

    #[test]
    fn test_embedding_ml_pipeline_not_quantized() {
        let p = embedding_to_ml_pipeline(30_522, 768, 0xbeba_5e00, false);
        assert_ne!(p.content_hash, 0);
        assert!(!p.quantized);
        assert_eq!(p.output_dim, 768);
    }

    #[test]
    fn test_embedding_ml_pipeline_quantized() {
        let p = embedding_to_ml_pipeline(30_522, 768, 0xbeba_5e00, true);
        assert!(p.quantized);
    }
}
