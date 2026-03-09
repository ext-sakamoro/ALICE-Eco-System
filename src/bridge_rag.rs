//! RAG bridges — ALICE-RAG ↔ DB, Cache, VectorDB, Analytics, Search
//!
//! 5 bridges connecting retrieval-augmented generation to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: RAG → DB (knowledge base record) ───────────────────────────

/// Knowledge base record for ALICE-DB persistence.
pub struct RagDbRecord {
    /// Content hash over the knowledge base snapshot.
    pub content_hash: u64,
    /// Number of text chunks indexed.
    pub chunk_count: u64,
    /// Number of source documents indexed.
    pub doc_count: u64,
    /// Hash of the vector index configuration.
    pub index_hash: u64,
    /// Dimensionality of the chunk embeddings.
    pub embedding_dim: u32,
    /// Total token count across all chunks.
    pub total_tokens: u64,
}

/// Serialize a RAG knowledge base snapshot for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn rag_to_db_record(
    chunk_count: u64,
    doc_count: u64,
    index_hash: u64,
    embedding_dim: u32,
    total_tokens: u64,
) -> RagDbRecord {
    let mut buf = [0u8; 36];
    buf[0..8].copy_from_slice(&chunk_count.to_le_bytes());
    buf[8..16].copy_from_slice(&doc_count.to_le_bytes());
    buf[16..24].copy_from_slice(&index_hash.to_le_bytes());
    buf[24..28].copy_from_slice(&embedding_dim.to_le_bytes());
    buf[28..36].copy_from_slice(&total_tokens.to_le_bytes());
    RagDbRecord {
        content_hash: fnv1a(&buf),
        chunk_count,
        doc_count,
        index_hash,
        embedding_dim,
        total_tokens,
    }
}

// ── Bridge 2: RAG → Cache (context window cache) ─────────────────────────

/// Context window cache entry for ALICE-Cache.
pub struct RagCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of retrieved chunks in the context.
    pub chunk_count: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Byte size of the serialised context.
    pub context_bytes: u64,
    /// Model version that generated the context.
    pub model_version: u32,
}

/// Build a context window cache entry for ALICE-Cache.
///
/// Large context windows receive a shorter TTL (60 s vs 300 s) because
/// they are memory-intensive and less likely to be reused verbatim.
#[inline]
#[must_use]
pub fn rag_to_cache_entry(
    chunk_count: u64,
    context_bytes: u64,
    model_version: u32,
) -> RagCacheEntry {
    let mut buf = [0u8; 20];
    buf[0..8].copy_from_slice(&chunk_count.to_le_bytes());
    buf[8..16].copy_from_slice(&context_bytes.to_le_bytes());
    buf[16..20].copy_from_slice(&model_version.to_le_bytes());
    let large_ctx = (chunk_count > 20) as u32;
    let ttl_secs = 300 - large_ctx * 240;
    RagCacheEntry {
        content_hash: fnv1a(&buf),
        chunk_count,
        ttl_secs,
        context_bytes,
        model_version,
    }
}

// ── Bridge 3: RAG → VectorDB (chunk index entry) ──────────────────────────

/// Chunk index entry for ALICE-VectorDB ingestion.
pub struct RagVectorDbEntry {
    /// Content hash over the index entry.
    pub content_hash: u64,
    /// Number of chunks in this index shard.
    pub chunk_count: u64,
    /// Dimensionality of the chunk embeddings.
    pub embedding_dim: u32,
    /// Hash of the index configuration.
    pub index_hash: u64,
    /// Shard identifier for distributed retrieval.
    pub shard_id: u32,
}

/// Build a chunk index entry for ALICE-VectorDB.
#[inline]
#[must_use]
pub fn rag_to_vectordb_entry(
    chunk_count: u64,
    embedding_dim: u32,
    index_hash: u64,
    shard_id: u32,
) -> RagVectorDbEntry {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&chunk_count.to_le_bytes());
    buf[8..12].copy_from_slice(&embedding_dim.to_le_bytes());
    buf[12..20].copy_from_slice(&index_hash.to_le_bytes());
    buf[20..24].copy_from_slice(&shard_id.to_le_bytes());
    RagVectorDbEntry {
        content_hash: fnv1a(&buf),
        chunk_count,
        embedding_dim,
        index_hash,
        shard_id,
    }
}

// ── Bridge 4: RAG → Analytics (retrieval event) ──────────────────────────

/// Retrieval analytics event for ALICE-Analytics ingestion.
pub struct RagAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Query latency in microseconds.
    pub query_time_us: u64,
    /// Number of chunks retrieved.
    pub chunk_count: u32,
    /// Average chunk relevance score in basis points (0–10 000).
    pub relevance_bps: u16,
    /// Cache hit rate in basis points (0–10 000).
    pub hit_rate_bps: u16,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a retrieval analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn rag_to_analytics_event(
    query_time_us: u64,
    chunk_count: u32,
    relevance_bps: u16,
    hit_rate_bps: u16,
    timestamp_ms: u64,
) -> RagAnalyticsEvent {
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&query_time_us.to_le_bytes());
    buf[8..12].copy_from_slice(&chunk_count.to_le_bytes());
    buf[12..14].copy_from_slice(&relevance_bps.to_le_bytes());
    buf[14..16].copy_from_slice(&hit_rate_bps.to_le_bytes());
    buf[16..24].copy_from_slice(&timestamp_ms.to_le_bytes());
    RagAnalyticsEvent {
        content_hash: fnv1a(&buf[..24]),
        query_time_us,
        chunk_count,
        relevance_bps,
        hit_rate_bps,
        timestamp_ms,
    }
}

// ── Bridge 5: RAG → Search (ranked result set) ───────────────────────────

/// Ranked result set for ALICE-Search integration.
pub struct RagSearchResult {
    /// Content hash over the result set.
    pub content_hash: u64,
    /// Number of results returned.
    pub result_count: u32,
    /// Sum of relevance scores multiplied by 100.
    pub score_sum_x100: u64,
    /// Highest relevance score in basis points (0–10 000).
    pub top_score_bps: u16,
    /// Hash of the query that produced this result set.
    pub query_hash: u64,
}

/// Build a ranked result set for ALICE-Search.
#[inline]
#[must_use]
pub fn rag_to_search_result(
    result_count: u32,
    score_sum_x100: u64,
    top_score_bps: u16,
    query_hash: u64,
) -> RagSearchResult {
    let mut buf = [0u8; 22];
    buf[0..4].copy_from_slice(&result_count.to_le_bytes());
    buf[4..12].copy_from_slice(&score_sum_x100.to_le_bytes());
    buf[12..14].copy_from_slice(&top_score_bps.to_le_bytes());
    buf[14..22].copy_from_slice(&query_hash.to_le_bytes());
    RagSearchResult {
        content_hash: fnv1a(&buf),
        result_count,
        score_sum_x100,
        top_score_bps,
        query_hash,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rag_db_record_hash_nonzero() {
        let rec = rag_to_db_record(10_000, 500, 0x696e_6465, 768, 8_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_rag_db_record_fields() {
        let rec = rag_to_db_record(2_000, 100, 0x1234, 512, 1_600_000);
        assert_eq!(rec.chunk_count, 2_000);
        assert_eq!(rec.doc_count, 100);
        assert_eq!(rec.embedding_dim, 512);
        assert_eq!(rec.total_tokens, 1_600_000);
    }

    #[test]
    fn test_rag_db_record_determinism() {
        let a = rag_to_db_record(5_000, 200, 0xaaaa, 768, 4_000_000);
        let b = rag_to_db_record(5_000, 200, 0xaaaa, 768, 4_000_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_rag_cache_entry_small_ctx_ttl() {
        let entry = rag_to_cache_entry(10, 40_960, 1);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_rag_cache_entry_large_ctx_ttl() {
        let entry = rag_to_cache_entry(50, 204_800, 1);
        assert_eq!(entry.ttl_secs, 60);
        assert_eq!(entry.chunk_count, 50);
    }

    #[test]
    fn test_rag_vectordb_entry() {
        let e = rag_to_vectordb_entry(8_000, 768, 0x484e_5357, 2);
        assert_ne!(e.content_hash, 0);
        assert_eq!(e.shard_id, 2);
        assert_eq!(e.embedding_dim, 768);
    }

    #[test]
    fn test_rag_analytics_event() {
        let ev = rag_to_analytics_event(2_500, 5, 8_800, 9_000, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.chunk_count, 5);
        assert_eq!(ev.relevance_bps, 8_800);
    }

    #[test]
    fn test_rag_search_result() {
        let r = rag_to_search_result(10, 87_500, 9_500, 0x7175_6572);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.result_count, 10);
        assert_eq!(r.top_score_bps, 9_500);
    }

    #[test]
    fn test_rag_search_result_determinism() {
        let a = rag_to_search_result(5, 45_000, 9_200, 0x7168_6173);
        let b = rag_to_search_result(5, 45_000, 9_200, 0x7168_6173);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
