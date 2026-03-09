//! Feature Store cross-domain bridges — ML ↔ Lakehouse, Cache, DB, Analytics, Edge
//!
//! 5 bridges providing ML feature store semantics by combining existing ALICE
//! crates.  No dedicated Feature Store crate is required — these bridges wire
//! ML feature vectors through the data pipeline for storage, caching, serving,
//! and monitoring.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Feature → Lakehouse (persistence) ─────────────────────────

/// Feature vector record for ALICE-Lakehouse columnar storage.
///
/// Persists versioned feature vectors so that offline training jobs can
/// retrieve point-in-time snapshots without recomputation.
pub struct FeatureLakehouseRecord {
    /// FNV-1a hash of entity_id + feature_name combined.
    pub content_hash: u64,
    /// Number of dimensions in the feature vector.
    pub dimension: u32,
    /// Feature schema version for backwards compatibility.
    pub schema_version: u32,
    /// Total byte size of the serialised feature vector.
    pub payload_bytes: usize,
    /// Unix timestamp in milliseconds when the feature was computed.
    pub timestamp_ms: u64,
}

/// Build a feature record for ALICE-Lakehouse persistence.
///
/// `content_hash` combines entity and feature identifiers via XOR so that
/// a change to either component produces a different key — branchless,
/// single XOR, no allocation.
#[inline]
#[must_use]
pub fn feature_to_lakehouse_record(
    entity_id: &[u8],
    feature_name: &[u8],
    dimension: u32,
    schema_version: u32,
    vector: &[f32],
    timestamp_ms: u64,
) -> FeatureLakehouseRecord {
    let content_hash = fnv1a(entity_id) ^ fnv1a(feature_name);
    let payload_bytes = vector.len() * 4;
    FeatureLakehouseRecord {
        content_hash,
        dimension,
        schema_version,
        payload_bytes,
        timestamp_ms,
    }
}

// ── Bridge 2: Feature → Cache (online serving) ──────────────────────────

/// Cached feature entry for ALICE-Cache online serving.
///
/// Caches the latest feature vector per entity so that inference requests
/// can retrieve pre-computed features without hitting the lakehouse.
/// TTL is shortened for high-dimensional features that change frequently.
pub struct FeatureCacheEntry {
    /// FNV-1a hash of entity_id + feature_name.
    pub content_hash: u64,
    /// Number of dimensions in the feature vector.
    pub dimension: u32,
    /// Time-to-live in seconds for this cache entry.
    pub ttl_seconds: u32,
    /// Whether this entry is worth caching (dimension > 0).
    pub cache_worthy: bool,
    /// Total byte size of the serialised feature vector.
    pub payload_bytes: usize,
}

/// Build a feature cache entry with dimension-adjusted TTL.
///
/// TTL derivation (branchless):
/// - dimension > 512 → 60 s  (high-dim features change fast)
/// - else             → 600 s (stable features)
#[inline]
#[must_use]
pub fn feature_to_cache_entry(
    entity_id: &[u8],
    feature_name: &[u8],
    dimension: u32,
    vector: &[f32],
) -> FeatureCacheEntry {
    let content_hash = fnv1a(entity_id) ^ fnv1a(feature_name);
    let high_dim = (dimension > 512) as u32;
    // Branchless TTL: high-dim=60, stable=600.
    let ttl_seconds = 600 - high_dim * 540;
    let cache_worthy = dimension > 0;
    let payload_bytes = vector.len() * 4;
    FeatureCacheEntry {
        content_hash,
        dimension,
        ttl_seconds,
        cache_worthy,
        payload_bytes,
    }
}

// ── Bridge 3: Feature → DB (metadata) ───────────────────────────────────

/// Feature metadata record for ALICE-DB.
///
/// Stores feature lineage (who computed it, when, from which source)
/// so that data scientists can audit feature provenance and freshness.
pub struct FeatureDbMetadata {
    /// FNV-1a hash of entity_id + feature_name.
    pub content_hash: u64,
    /// Number of dimensions.
    pub dimension: u32,
    /// Schema version.
    pub schema_version: u32,
    /// Feature freshness in seconds (age since last computation).
    pub freshness_secs: u64,
    /// Whether the feature is stale (freshness > 3600 s).
    pub is_stale: bool,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build feature metadata for ALICE-DB.
///
/// `is_stale` is set branchlessly: freshness > 3600 s (1 hour).
#[inline]
#[must_use]
pub fn feature_to_db_metadata(
    entity_id: &[u8],
    feature_name: &[u8],
    dimension: u32,
    schema_version: u32,
    freshness_secs: u64,
    timestamp_ms: u64,
) -> FeatureDbMetadata {
    let content_hash = fnv1a(entity_id) ^ fnv1a(feature_name);
    let is_stale = freshness_secs > 3600;
    FeatureDbMetadata {
        content_hash,
        dimension,
        schema_version,
        freshness_secs,
        is_stale,
        timestamp_ms,
    }
}

// ── Bridge 4: Feature → Analytics (usage metrics) ───────────────────────

/// Feature usage metrics for ALICE-Analytics.
///
/// Tracks how often each feature is requested for inference so that
/// unused features can be retired and hot features can be prioritised
/// for pre-computation.
pub struct FeatureAnalyticsMetrics {
    /// FNV-1a hash of feature_name.
    pub content_hash: u64,
    /// Number of times this feature was requested in the sampling window.
    pub request_count: u64,
    /// Number of cache hits in the sampling window.
    pub cache_hits: u64,
    /// Cache hit rate in integer percent (0–100).
    pub hit_rate_pct: u8,
    /// Average latency in microseconds for feature retrieval.
    pub avg_latency_us: u64,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build feature usage metrics for ALICE-Analytics.
///
/// `hit_rate_pct` = `cache_hits / request_count * 100` — integer
/// arithmetic, denominator clamped to 1, result clamped to 100.
#[inline]
#[must_use]
pub fn feature_to_analytics_metrics(
    feature_name: &[u8],
    request_count: u64,
    cache_hits: u64,
    avg_latency_us: u64,
    timestamp_ms: u64,
) -> FeatureAnalyticsMetrics {
    let content_hash = fnv1a(feature_name);
    let total = request_count.max(1);
    let hit_rate_pct = ((cache_hits * 100) / total).min(100) as u8;
    FeatureAnalyticsMetrics {
        content_hash,
        request_count,
        cache_hits,
        hit_rate_pct,
        avg_latency_us,
        timestamp_ms,
    }
}

// ── Bridge 5: Feature → Edge (serving payload) ──────────────────────────

/// Compact feature payload for ALICE-Edge inference serving.
///
/// Minimises wire size for edge-deployed models that need feature vectors
/// streamed from the central feature store.
pub struct FeatureEdgePayload {
    /// FNV-1a hash of entity_id + feature_name.
    pub content_hash: u64,
    /// Number of dimensions.
    pub dimension: u32,
    /// Payload size estimate in bytes.
    pub payload_bytes: usize,
    /// Whether compression is recommended (dimension > 128).
    pub compress: bool,
    /// Schema version for wire compatibility.
    pub schema_version: u32,
}

/// Build a compact feature payload for ALICE-Edge.
///
/// `compress` is set branchlessly: dimension > 128 benefits from
/// compression on the wire.
#[inline]
#[must_use]
pub fn feature_to_edge_payload(
    entity_id: &[u8],
    feature_name: &[u8],
    dimension: u32,
    vector: &[f32],
    schema_version: u32,
) -> FeatureEdgePayload {
    let content_hash = fnv1a(entity_id) ^ fnv1a(feature_name);
    let payload_bytes = vector.len() * 4;
    let compress = dimension > 128;
    FeatureEdgePayload {
        content_hash,
        dimension,
        payload_bytes,
        compress,
        schema_version,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_lakehouse_record_basic() {
        let vec = vec![1.0, 2.0, 3.0];
        let rec = feature_to_lakehouse_record(
            b"user:123",
            b"embedding_v2",
            3,
            2,
            &vec,
            1_700_000_000_000,
        );
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.dimension, 3);
        assert_eq!(rec.schema_version, 2);
        assert_eq!(rec.payload_bytes, 12);
    }

    #[test]
    fn test_feature_lakehouse_hash_determinism() {
        let vec = vec![0.5; 10];
        let r1 = feature_to_lakehouse_record(b"e1", b"f1", 10, 1, &vec, 0);
        let r2 = feature_to_lakehouse_record(b"e1", b"f1", 10, 1, &vec, 0);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_feature_cache_entry_stable_ttl() {
        let vec = vec![1.0; 64];
        let e = feature_to_cache_entry(b"user:1", b"profile", 64, &vec);
        assert_eq!(e.ttl_seconds, 600, "low-dim should have 600s TTL");
        assert!(e.cache_worthy);
        assert_eq!(e.payload_bytes, 256);
    }

    #[test]
    fn test_feature_cache_entry_high_dim_ttl() {
        let vec = vec![0.0; 1024];
        let e = feature_to_cache_entry(b"user:1", b"deep_embed", 1024, &vec);
        assert_eq!(e.ttl_seconds, 60, "high-dim should have 60s TTL");
    }

    #[test]
    fn test_feature_cache_entry_zero_dim() {
        let vec: Vec<f32> = vec![];
        let e = feature_to_cache_entry(b"user:1", b"empty", 0, &vec);
        assert!(!e.cache_worthy, "zero-dim should not be cache-worthy");
        assert_eq!(e.payload_bytes, 0);
    }

    #[test]
    fn test_feature_db_metadata_stale() {
        let meta = feature_to_db_metadata(b"e1", b"f1", 128, 3, 7200, 0);
        assert!(meta.is_stale, "7200s freshness should be stale");
        assert_eq!(meta.freshness_secs, 7200);
    }

    #[test]
    fn test_feature_db_metadata_fresh() {
        let meta = feature_to_db_metadata(b"e1", b"f1", 128, 3, 300, 0);
        assert!(!meta.is_stale, "300s freshness should not be stale");
    }

    #[test]
    fn test_feature_analytics_hit_rate() {
        let m = feature_to_analytics_metrics(b"embedding_v2", 100, 75, 500, 0);
        assert_eq!(m.hit_rate_pct, 75);
        assert_ne!(m.content_hash, 0);
        // zero requests → 0% hit rate (clamped denominator)
        let m2 = feature_to_analytics_metrics(b"rare_feature", 0, 0, 0, 0);
        assert_eq!(m2.hit_rate_pct, 0);
    }

    #[test]
    fn test_feature_edge_payload_compress() {
        let vec = vec![0.0; 256];
        let p = feature_to_edge_payload(b"e1", b"large", 256, &vec, 1);
        assert!(p.compress, "256-dim should recommend compression");
        assert_eq!(p.payload_bytes, 1024);

        let small_vec = vec![0.0; 32];
        let p2 = feature_to_edge_payload(b"e1", b"small", 32, &small_vec, 1);
        assert!(!p2.compress, "32-dim should not recommend compression");
    }

    #[test]
    fn test_feature_edge_payload_hash_differs_by_entity() {
        let vec = vec![1.0; 8];
        let p1 = feature_to_edge_payload(b"entity_A", b"feat", 8, &vec, 1);
        let p2 = feature_to_edge_payload(b"entity_B", b"feat", 8, &vec, 1);
        assert_ne!(p1.content_hash, p2.content_hash);
    }
}
