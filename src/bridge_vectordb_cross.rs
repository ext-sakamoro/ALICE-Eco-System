//! Cross-domain bridges — ALICE-VectorDB ↔ ML, Token, Search, Analytics, Cache
//!
//! 5 bridges connecting vector similarity search to ML input features,
//! Search index entries, Analytics metrics, Token vocabulary embeddings,
//! and Cache.

use alice_vectordb::{HnswIndex, SearchResult, VectorRecord};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Hash an f32 slice via FNV-1a over raw little-endian bytes.
fn hash_f32_slice(data: &[f32]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &v in data {
        for &b in &v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

// ── Bridge 1: SearchResult → ML input features ─────────────────────

/// A VectorDB search result converted into ML input features.
///
/// Encodes the search result id hash, distance, and rank into a feature
/// vector so the ML layer can use retrieval results as input features
/// for re-ranking or classification.
pub struct VectordbSearchMlInput {
    /// FNV-1a hash over id hash, distance, rank bytes.
    pub content_hash: u64,
    /// Hash of the result record id.
    pub id_hash: u64,
    /// Distance from query.
    pub distance: f32,
    /// Rank (0-based position in result list).
    pub rank: usize,
    /// Reciprocal rank: 1.0 / (rank + 1).
    pub reciprocal_rank: f32,
    /// Feature dimension: always 4 (id_hash_lo, distance, rank, reciprocal_rank).
    pub feature_dim: usize,
}

/// Convert a VectorDB search result into ML input features.
#[inline]
#[must_use]
pub fn vectordb_search_to_ml_input(result: &SearchResult, rank: usize) -> VectordbSearchMlInput {
    let id_hash = fnv1a(result.id.as_bytes());
    let reciprocal_rank = 1.0 / (rank as f32 + 1.0);

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&id_hash.to_le_bytes());
    key[8..12].copy_from_slice(&result.distance.to_le_bytes());
    key[12..20].copy_from_slice(&(rank as u64).to_le_bytes());
    key[20..24].copy_from_slice(&reciprocal_rank.to_le_bytes());

    VectordbSearchMlInput {
        content_hash: fnv1a(&key),
        id_hash,
        distance: result.distance,
        rank,
        reciprocal_rank,
        feature_dim: 4,
    }
}

// ── Bridge 2: VectorRecord → Search index entry ─────────────────────

/// A VectorDB record converted into a Search index entry.
///
/// Creates a searchable text representation of a vector record so the
/// Search layer (FM-Index) can do text-based lookups on vector record ids.
pub struct VectordbSearchIndexEntry {
    /// FNV-1a hash over id, dimension, vector hash bytes.
    pub content_hash: u64,
    /// The record id.
    pub record_id: [u8; 64],
    /// Length of the record_id.
    pub record_id_len: usize,
    /// Vector dimensionality.
    pub dimension: usize,
    /// Hash of the vector data (for deduplication).
    pub vector_hash: u64,
    /// L2 norm of the vector (magnitude).
    pub l2_norm: f32,
}

/// Convert a VectorDB record into a Search index entry.
#[inline]
#[must_use]
pub fn vectordb_record_to_search_index(record: &VectorRecord) -> VectordbSearchIndexEntry {
    let dimension = record.vector.len();
    let vector_hash = hash_f32_slice(&record.vector);

    let norm_sq: f32 = record.vector.iter().map(|&x| x * x).sum();
    let l2_norm = sqrt_f32(norm_sq);

    // Copy record id into fixed buffer
    let mut record_id = [0u8; 64];
    let id_bytes = record.id.as_bytes();
    let copy_len = id_bytes.len().min(64);
    record_id[..copy_len].copy_from_slice(&id_bytes[..copy_len]);

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&fnv1a(id_bytes).to_le_bytes());
    key[8..16].copy_from_slice(&(dimension as u64).to_le_bytes());
    key[16..24].copy_from_slice(&vector_hash.to_le_bytes());

    VectordbSearchIndexEntry {
        content_hash: fnv1a(&key),
        record_id,
        record_id_len: copy_len,
        dimension,
        vector_hash,
        l2_norm,
    }
}

fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut g = x / 2.0;
    for _ in 0..15 {
        g = f32::midpoint(g, x / g);
    }
    g
}

// ── Bridge 3: HnswIndex stats → Analytics metrics ────────────────────

/// HNSW index statistics converted into Analytics-compatible metrics.
///
/// Captures the index size and metric type so the Analytics pipeline
/// can track vector index growth and health.
pub struct VectordbHnswAnalytics {
    /// FNV-1a hash over record_count, metric_type bytes.
    pub content_hash: u64,
    /// Number of records in the index.
    pub record_count: usize,
    /// Whether the index is empty.
    pub is_empty: bool,
    /// Distance metric type discriminant: 0=Euclidean, 1=Cosine, 2=DotProduct.
    pub metric_type: u8,
    /// Metric name hash for Analytics pipeline registration.
    pub metric_name_hash: u64,
}

/// Convert HNSW index statistics into Analytics metrics.
#[inline]
#[must_use]
pub fn vectordb_hnsw_to_analytics(index: &HnswIndex) -> VectordbHnswAnalytics {
    let record_count = index.len();
    let is_empty = index.is_empty();
    let metric_name_hash = fnv1a(b"vectordb.hnsw");

    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&(record_count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&metric_name_hash.to_le_bytes());

    VectordbHnswAnalytics {
        content_hash: fnv1a(&key),
        record_count,
        is_empty,
        metric_type: 0, // Default; actual metric is private in HnswIndex
        metric_name_hash,
    }
}

// ── Bridge 4: Token vocabulary → vector embedding seed ──────────────

/// Token vocabulary information converted into a vector embedding seed.
///
/// Creates a deterministic embedding seed from a token id and vocab size
/// so the VectorDB can store initial token embeddings without accessing
/// the Token crate's internal structures.
pub struct VectordbTokenVector {
    /// FNV-1a hash over token_id, vocab_size bytes.
    pub content_hash: u64,
    /// Token id.
    pub token_id: u32,
    /// Vocabulary size.
    pub vocab_size: usize,
    /// Embedding seed: deterministic hash-based seed for initializing the vector.
    pub embed_seed: u64,
    /// Recommended embedding dimension (based on vocab size: min(512, vocab_size/2)).
    pub recommended_dim: usize,
}

/// Convert a token id and vocabulary size into a vector embedding seed.
#[inline]
#[must_use]
pub fn vectordb_token_to_vector(token_id: u32, vocab_size: usize) -> VectordbTokenVector {
    let mut key = [0u8; 12];
    key[0..4].copy_from_slice(&token_id.to_le_bytes());
    key[4..12].copy_from_slice(&(vocab_size as u64).to_le_bytes());

    let embed_seed = fnv1a(&key);
    let recommended_dim = if vocab_size < 1024 {
        vocab_size / 2
    } else {
        512
    };
    let recommended_dim = if recommended_dim == 0 {
        1
    } else {
        recommended_dim
    };

    VectordbTokenVector {
        content_hash: fnv1a(&key),
        token_id,
        vocab_size,
        embed_seed,
        recommended_dim,
    }
}

// ── Bridge 5: VectorDB search result → Cache ─────────────────────────

/// A VectorDB search result converted into a Cache entry.
///
/// Caches a search result so repeated queries can be served without
/// re-running the similarity search. TTL is branchless-adjusted:
/// high-distance (low-relevance) results get shorter TTL.
pub struct VectordbResultCache {
    /// FNV-1a hash over id hash, distance, rank bytes.
    pub content_hash: u64,
    /// Hash of the result record id.
    pub id_hash: u64,
    /// Distance from query.
    pub distance: f32,
    /// Rank in the result list.
    pub rank: usize,
    /// TTL in seconds. High-distance results (>1.0) get shorter TTL.
    pub ttl_secs: u32,
    /// Cache key hash for direct lookup.
    pub cache_key: u64,
}

/// Convert a VectorDB search result into a Cache entry.
#[inline]
#[must_use]
pub fn vectordb_result_to_cache(result: &SearchResult, rank: usize) -> VectordbResultCache {
    let id_hash = fnv1a(result.id.as_bytes());

    // Branchless TTL: high distance (>1.0) gets 180s less
    let high_distance = (result.distance > 1.0) as u32;
    let ttl_secs: u32 = 600 - high_distance * 180;

    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&id_hash.to_le_bytes());
    key[8..12].copy_from_slice(&result.distance.to_le_bytes());
    key[12..20].copy_from_slice(&(rank as u64).to_le_bytes());

    let cache_key = fnv1a(&key);

    VectordbResultCache {
        content_hash: fnv1a(&key),
        id_hash,
        distance: result.distance,
        rank,
        ttl_secs,
        cache_key,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_vectordb::{DistanceMetric, HnswIndex, SearchResult, VectorRecord};

    fn make_search_result(id: &str, distance: f32) -> SearchResult {
        SearchResult {
            id: String::from(id),
            distance,
        }
    }

    fn make_record(id: &str, vec: Vec<f32>) -> VectorRecord {
        VectorRecord {
            id: String::from(id),
            vector: vec,
        }
    }

    // ── Bridge 1: search result → ml input ───────────────────────────

    #[test]
    fn test_vectordb_search_to_ml_input() {
        let result = make_search_result("doc_42", 0.15);
        let ml = vectordb_search_to_ml_input(&result, 0);
        assert_ne!(ml.content_hash, 0);
        assert!((ml.distance - 0.15).abs() < 1e-6);
        assert_eq!(ml.rank, 0);
        assert!((ml.reciprocal_rank - 1.0).abs() < 1e-6);
        assert_eq!(ml.feature_dim, 4);
    }

    #[test]
    fn test_vectordb_search_to_ml_input_deterministic() {
        let result = make_search_result("doc_7", 0.5);
        let m1 = vectordb_search_to_ml_input(&result, 2);
        let m2 = vectordb_search_to_ml_input(&result, 2);
        assert_eq!(m1.content_hash, m2.content_hash);
    }

    // ── Bridge 2: record → search index ──────────────────────────────

    #[test]
    fn test_vectordb_record_to_search_index() {
        let record = make_record("embedding_001", vec![1.0, 0.0, 0.0]);
        let entry = vectordb_record_to_search_index(&record);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.dimension, 3);
        assert!(entry.record_id_len > 0);
        assert!((entry.l2_norm - 1.0).abs() < 0.01);
        assert_ne!(entry.vector_hash, 0);
    }

    #[test]
    fn test_vectordb_record_to_search_index_deterministic() {
        let record = make_record("vec_99", vec![3.0, 4.0]);
        let e1 = vectordb_record_to_search_index(&record);
        let e2 = vectordb_record_to_search_index(&record);
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.vector_hash, e2.vector_hash);
    }

    // ── Bridge 3: hnsw → analytics ───────────────────────────────────

    #[test]
    fn test_vectordb_hnsw_to_analytics() {
        let mut index = HnswIndex::new(DistanceMetric::Euclidean, 4);
        index.insert(make_record("a", vec![1.0, 0.0]));
        index.insert(make_record("b", vec![0.0, 1.0]));
        let analytics = vectordb_hnsw_to_analytics(&index);
        assert_ne!(analytics.content_hash, 0);
        assert_eq!(analytics.record_count, 2);
        assert!(!analytics.is_empty);
        assert_ne!(analytics.metric_name_hash, 0);
    }

    #[test]
    fn test_vectordb_hnsw_to_analytics_empty() {
        let index = HnswIndex::new(DistanceMetric::Cosine, 8);
        let analytics = vectordb_hnsw_to_analytics(&index);
        assert_eq!(analytics.record_count, 0);
        assert!(analytics.is_empty);
    }

    // ── Bridge 4: token → vector ─────────────────────────────────────

    #[test]
    fn test_vectordb_token_to_vector() {
        let tv = vectordb_token_to_vector(42, 50000);
        assert_ne!(tv.content_hash, 0);
        assert_eq!(tv.token_id, 42);
        assert_eq!(tv.vocab_size, 50000);
        assert_ne!(tv.embed_seed, 0);
        assert_eq!(tv.recommended_dim, 512);
    }

    #[test]
    fn test_vectordb_token_to_vector_small_vocab() {
        let tv = vectordb_token_to_vector(0, 100);
        assert_eq!(tv.recommended_dim, 50); // 100/2
    }

    // ── Bridge 5: result → cache ─────────────────────────────────────

    #[test]
    fn test_vectordb_result_to_cache_close() {
        let result = make_search_result("doc_1", 0.1);
        let cache = vectordb_result_to_cache(&result, 0);
        assert_ne!(cache.content_hash, 0);
        // Low distance → full TTL
        assert_eq!(cache.ttl_secs, 600);
        assert_ne!(cache.cache_key, 0);
    }

    #[test]
    fn test_vectordb_result_to_cache_far_ttl() {
        let result = make_search_result("doc_2", 2.5);
        let cache = vectordb_result_to_cache(&result, 5);
        // Branchless: 600 - 1 * 180 = 420
        assert_eq!(cache.ttl_secs, 420);
    }

    #[test]
    fn test_vectordb_result_to_cache_deterministic() {
        let result = make_search_result("doc_3", 0.75);
        let c1 = vectordb_result_to_cache(&result, 1);
        let c2 = vectordb_result_to_cache(&result, 1);
        assert_eq!(c1.content_hash, c2.content_hash);
        assert_eq!(c1.cache_key, c2.cache_key);
    }
}
