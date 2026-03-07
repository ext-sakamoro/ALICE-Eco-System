//! VectorDB bridges — ALICE-VectorDB ↔ DB, Cache, Analytics, ML, Search
//!
//! 5 bridges connecting vector similarity search to the ALICE ecosystem.

use alice_vectordb::{
    brute_force_knn, cosine_similarity, euclidean_distance_sq, normalize, DistanceMetric,
    HnswIndex, VectorRecord,
};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Hash a `f32` slice to a `u64` via FNV-1a over the raw little-endian bytes.
#[inline(always)]
fn hash_f32_slice(v: &[f32]) -> u64 {
    let bytes: Vec<u8> = v.iter().flat_map(|&x| x.to_le_bytes()).collect();
    fnv1a(&bytes)
}

// ── Bridge 1: VectorDB → DB (index persistence record) ───────────────────

/// Vector index persistence record for ALICE-DB.
///
/// Captures the identity and geometry of a stored vector so the database
/// layer can persist and restore the HNSW index across restarts.
pub struct VectorDbIndexRecord {
    /// FNV-1a hash of the vector bytes (deduplication / primary key).
    pub content_hash: u64,
    /// FNV-1a hash of the record ID string.
    pub id_hash: u64,
    /// Vector dimensionality.
    pub dimension: usize,
    /// L2 norm of the vector (pre-normalisation metric).
    pub l2_norm: f32,
    /// Cosine self-similarity (always 1.0 for non-zero vectors; 0.0 for zero vectors).
    pub self_cosine: f32,
}

/// Build a vector index persistence record for ALICE-DB from a `VectorRecord`.
#[inline]
#[must_use]
pub fn vectordb_to_db_record(record: &VectorRecord) -> VectorDbIndexRecord {
    let content_hash = hash_f32_slice(&record.vector);
    let id_hash = fnv1a(record.id.as_bytes());
    let dimension = record.vector.len();
    // L2 norm = sqrt(euclidean_distance_sq(v, 0)).
    let l2_norm = euclidean_distance_sq(&record.vector, &vec![0.0f32; dimension]).sqrt();
    // Cosine similarity with itself is 1.0 for non-zero, 0.0 for zero.
    let self_cosine = cosine_similarity(&record.vector, &record.vector);
    VectorDbIndexRecord {
        content_hash,
        id_hash,
        dimension,
        l2_norm,
        self_cosine,
    }
}

// ── Bridge 2: VectorDB → Cache (query result cache entry) ─────────────────

/// Vector query result cache entry for ALICE-Cache.
///
/// Caches the top-k search results for a query vector so repeated identical
/// queries are served from cache rather than re-running HNSW search.
/// TTL is shortened branchlessly for high-dimensional vectors.
pub struct VectorDbCacheEntry {
    /// FNV-1a hash of the query vector bytes (cache key).
    pub content_hash: u64,
    /// Top-k result IDs (ordered by ascending distance).
    pub result_ids: Vec<String>,
    /// Corresponding distances for each result.
    pub distances: Vec<f32>,
    /// Number of results returned.
    pub result_count: usize,
    /// Cache TTL in seconds (branchless: shorter for high-dimensional queries).
    pub ttl_secs: u32,
}

/// Cache top-k brute-force KNN results for a query vector.
///
/// TTL is 3600 s for vectors with dimension ≤ 512, and 900 s for higher
/// dimensions (branchless), because high-dimensional query results are more
/// likely to become stale quickly.
#[inline]
#[must_use]
pub fn vectordb_to_cache_entry(
    query: &[f32],
    records: &[VectorRecord],
    k: usize,
    metric: DistanceMetric,
) -> VectorDbCacheEntry {
    let content_hash = hash_f32_slice(query);
    let results = brute_force_knn(query, records, k, metric);
    let result_ids: Vec<String> = results.iter().map(|r| r.id.clone()).collect();
    let distances: Vec<f32> = results.iter().map(|r| r.distance).collect();
    let result_count = result_ids.len();

    // Branchless TTL: dim > 512 → 900 s, else 3600 s.
    let high_dim = (query.len() > 512) as u32;
    let ttl_secs = 3600 - high_dim * 2700;

    VectorDbCacheEntry {
        content_hash,
        result_ids,
        distances,
        result_count,
        ttl_secs,
    }
}

// ── Bridge 3: VectorDB → Analytics (search metrics) ──────────────────────

/// Vector search metrics for ALICE-Analytics.
///
/// Records per-query statistics so the analytics layer can track search
/// quality, latency proxies, and index utilisation over time.
pub struct VectorDbAnalyticsMetrics {
    /// FNV-1a hash of the query vector (analytics stream key).
    pub content_hash: u64,
    /// Number of candidate records searched.
    pub corpus_size: usize,
    /// Number of results returned.
    pub result_count: usize,
    /// Distance metric used (0=Euclidean, 1=Cosine, 2=DotProduct).
    pub metric_code: u8,
    /// Top-1 distance (0.0 if no results).
    pub top1_distance: f32,
    /// Mean distance across all returned results.
    pub mean_distance: f32,
    /// Query vector dimensionality.
    pub dimension: usize,
}

/// Extract vector search metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn vectordb_to_analytics_metrics(
    query: &[f32],
    records: &[VectorRecord],
    k: usize,
    metric: DistanceMetric,
) -> VectorDbAnalyticsMetrics {
    let content_hash = hash_f32_slice(query);
    let results = brute_force_knn(query, records, k, metric);
    let metric_code = match metric {
        DistanceMetric::Euclidean => 0u8,
        DistanceMetric::Cosine => 1u8,
        DistanceMetric::DotProduct => 2u8,
    };
    let top1_distance = results.first().map_or(0.0, |r| r.distance);
    let mean_distance = if results.is_empty() {
        0.0
    } else {
        results.iter().map(|r| r.distance).sum::<f32>() / results.len() as f32
    };
    VectorDbAnalyticsMetrics {
        content_hash,
        corpus_size: records.len(),
        result_count: results.len(),
        metric_code,
        top1_distance,
        mean_distance,
        dimension: query.len(),
    }
}

// ── Bridge 4: VectorDB → ML (embedding model link) ────────────────────────

/// Embedding model linkage record for ALICE-ML.
///
/// Associates a stored vector record with the embedding model that produced
/// it, enabling the ML layer to retrain or fine-tune on corpus subsets.
pub struct VectorDbMlEmbeddingLink {
    /// FNV-1a hash of the vector bytes (vector identity key).
    pub content_hash: u64,
    /// FNV-1a hash of the model name string.
    pub model_hash: u64,
    /// Vector dimensionality.
    pub dimension: usize,
    /// Cosine similarity of the vector with its L2-normalised counterpart
    /// (1.0 = already unit-length, < 1.0 = unnormalised).
    pub normalisation_cosine: f32,
    /// Whether the vector is already L2-unit-normalised (‖v‖ ≈ 1.0).
    pub is_normalised: bool,
}

/// Build an embedding model linkage record for ALICE-ML.
///
/// `model_name` identifies the ALICE-ML model that produced the embedding.
#[inline]
#[must_use]
pub fn vectordb_to_ml_embedding_link(
    record: &VectorRecord,
    model_name: &str,
) -> VectorDbMlEmbeddingLink {
    let content_hash = hash_f32_slice(&record.vector);
    let model_hash = fnv1a(model_name.as_bytes());
    let dimension = record.vector.len();

    // Check normalisation: compute L2 norm and compare with 1.0.
    let mut normalised = record.vector.clone();
    normalize(&mut normalised);
    let normalisation_cosine = cosine_similarity(&record.vector, &normalised);
    let l2_sq: f32 = record.vector.iter().map(|&x| x * x).sum();
    let is_normalised = (l2_sq - 1.0).abs() < 1e-3;

    VectorDbMlEmbeddingLink {
        content_hash,
        model_hash,
        dimension,
        normalisation_cosine,
        is_normalised,
    }
}

// ── Bridge 5: VectorDB → Search (hybrid search integration) ───────────────

/// Hybrid search descriptor for ALICE-Search integration.
///
/// Combines vector similarity results with keyword search metadata so the
/// ALICE-Search layer can blend semantic and lexical ranking signals.
pub struct VectorDbSearchDescriptor {
    /// FNV-1a hash of the query vector (hybrid search correlation key).
    pub content_hash: u64,
    /// Top-k result IDs from vector search.
    pub vector_result_ids: Vec<String>,
    /// Reciprocal rank fusion scores (1 / (k + rank), scaled × 1000 as integer).
    pub rrf_scores_x1000: Vec<u32>,
    /// Number of vector results.
    pub result_count: usize,
    /// Suggested hybrid blend weight for vector scores (0–100 permille).
    pub vector_weight_permille: u32,
}

/// Build a hybrid search descriptor from HNSW search results.
///
/// `vector_weight_permille` (0–1000) controls the blend between vector and
/// keyword scores in the ALICE-Search hybrid ranker.  Reciprocal rank fusion
/// scores are precomputed as `floor(1000 / (60 + rank + 1))` following the
/// standard RRF-60 formula.
#[inline]
#[must_use]
pub fn vectordb_to_search_descriptor(
    query: &[f32],
    index: &HnswIndex,
    k: usize,
    vector_weight_permille: u32,
) -> VectorDbSearchDescriptor {
    let content_hash = hash_f32_slice(query);
    let results = index.search(query, k);
    let vector_result_ids: Vec<String> = results.iter().map(|r| r.id.clone()).collect();

    // RRF-60: score = 1000 / (60 + rank + 1)
    let rrf_scores_x1000: Vec<u32> = (0..results.len())
        .map(|rank| 1000 / (61 + rank as u32))
        .collect();

    let result_count = vector_result_ids.len();
    let weight = vector_weight_permille.min(1000);

    VectorDbSearchDescriptor {
        content_hash,
        vector_result_ids,
        rrf_scores_x1000,
        result_count,
        vector_weight_permille: weight,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_records(n: usize) -> Vec<VectorRecord> {
        (0..n)
            .map(|i| VectorRecord {
                id: format!("vec-{i}"),
                vector: vec![i as f32, (i * 2) as f32, (i * 3) as f32],
            })
            .collect()
    }

    #[test]
    fn test_db_record_basic() {
        let rec_src = VectorRecord {
            id: String::from("embed-001"),
            vector: vec![1.0, 0.5, 0.25],
        };
        let db_rec = vectordb_to_db_record(&rec_src);
        assert_ne!(db_rec.content_hash, 0);
        assert_ne!(db_rec.id_hash, 0);
        assert_eq!(db_rec.dimension, 3);
        assert!(db_rec.l2_norm > 0.0);
    }

    #[test]
    fn test_db_record_hash_deterministic() {
        let rec = VectorRecord {
            id: String::from("x"),
            vector: vec![1.0, 2.0, 3.0],
        };
        let a = vectordb_to_db_record(&rec);
        let b = vectordb_to_db_record(&rec);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.id_hash, b.id_hash);
    }

    #[test]
    fn test_cache_entry_small_ttl() {
        // dim ≤ 512 → 3600 s
        let query = vec![1.0f32; 3];
        let records = make_records(5);
        let entry = vectordb_to_cache_entry(&query, &records, 3, DistanceMetric::Euclidean);
        assert_eq!(entry.ttl_secs, 3600);
        assert_ne!(entry.content_hash, 0);
        assert!(entry.result_count <= 3);
    }

    #[test]
    fn test_cache_entry_large_dim_ttl() {
        // dim > 512 → 900 s (branchless)
        let query = vec![1.0f32; 513];
        let records: Vec<VectorRecord> = vec![VectorRecord {
            id: String::from("big"),
            vector: vec![0.0f32; 513],
        }];
        let entry = vectordb_to_cache_entry(&query, &records, 1, DistanceMetric::Euclidean);
        assert_eq!(entry.ttl_secs, 900);
    }

    #[test]
    fn test_analytics_metrics_basic() {
        let query = vec![5.0f32, 0.0, 0.0];
        let records = make_records(10);
        let m = vectordb_to_analytics_metrics(&query, &records, 3, DistanceMetric::Euclidean);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.corpus_size, 10);
        assert_eq!(m.metric_code, 0);
        assert!(m.result_count <= 3);
    }

    #[test]
    fn test_analytics_metrics_cosine_code() {
        let query = vec![1.0f32, 0.0];
        let records = vec![VectorRecord {
            id: String::from("a"),
            vector: vec![1.0f32, 0.0],
        }];
        let m = vectordb_to_analytics_metrics(&query, &records, 1, DistanceMetric::Cosine);
        assert_eq!(m.metric_code, 1);
    }

    #[test]
    fn test_ml_embedding_link_basic() {
        let rec = VectorRecord {
            id: String::from("embed-xyz"),
            vector: vec![0.6f32, 0.8f32],
        };
        let link = vectordb_to_ml_embedding_link(&rec, "alice-bert-v1");
        assert_ne!(link.content_hash, 0);
        assert_ne!(link.model_hash, 0);
        assert_eq!(link.dimension, 2);
    }

    #[test]
    fn test_ml_embedding_link_normalised_vector() {
        // [0.6, 0.8] has L2 norm 1.0 — should be flagged as normalised.
        let rec = VectorRecord {
            id: String::from("unit"),
            vector: vec![0.6f32, 0.8f32],
        };
        let link = vectordb_to_ml_embedding_link(&rec, "model");
        assert!(link.is_normalised, "0.6²+0.8²=1.0 so must be normalised");
    }

    #[test]
    fn test_search_descriptor_basic() {
        let mut index = HnswIndex::new(DistanceMetric::Euclidean, 4);
        for rec in make_records(5) {
            index.insert(rec);
        }
        let query = vec![2.0f32, 4.0, 6.0];
        let desc = vectordb_to_search_descriptor(&query, &index, 3, 700);
        assert_ne!(desc.content_hash, 0);
        assert_eq!(desc.vector_weight_permille, 700);
        assert!(desc.result_count <= 3);
    }

    #[test]
    fn test_search_descriptor_weight_clamped() {
        let index = HnswIndex::new(DistanceMetric::Euclidean, 4);
        let desc = vectordb_to_search_descriptor(&[1.0f32], &index, 3, 9999);
        assert_eq!(desc.vector_weight_permille, 1000);
    }
}
