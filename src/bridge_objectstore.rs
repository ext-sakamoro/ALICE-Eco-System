//! ObjectStore bridges — ALICE-ObjectStore ↔ DB, Cache, Analytics, CDN, Backup
//!
//! 5 bridges connecting object storage metadata and delivery telemetry to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: ObjectStore → DB (metadata) ────────────────────────────────

/// Object metadata record for ALICE-DB persistence.
///
/// Written for every object PUT/DELETE so that DB consumers can track
/// bucket state, versioning history, and storage growth without scanning
/// the live object store.
pub struct ObjectStoreDbMetadata {
    /// FNV-1a hash of the bucket name used to identify this record.
    pub content_hash: u64,
    /// FNV-1a hash of the bucket name (separate for join queries).
    pub bucket_hash: u64,
    /// Total number of objects in the bucket.
    pub object_count: u64,
    /// Total bytes stored across all objects in the bucket.
    pub total_bytes: u64,
    /// Number of object versions retained (0 when versioning is disabled).
    pub version_count: u64,
    /// Number of in-progress multipart upload parts.
    pub part_count: u32,
    /// Unix timestamp in milliseconds of the metadata snapshot.
    pub snapshot_ms: u64,
    /// Storage class identifier (0=standard, 1=infrequent, 2=archive).
    pub storage_class: u8,
}

/// Build an object metadata record for ALICE-DB.
///
/// `content_hash` and `bucket_hash` are both derived from the bucket name,
/// allowing downstream consumers to use either field as a join key without
/// re-hashing — deterministic, allocation-free.
#[inline]
#[must_use]
pub fn objectstore_to_db_metadata(
    bucket: &[u8],
    object_count: u64,
    total_bytes: u64,
    version_count: u64,
    part_count: u32,
    snapshot_ms: u64,
    storage_class: u8,
) -> ObjectStoreDbMetadata {
    let bucket_hash = fnv1a(bucket);
    let content_hash = bucket_hash;
    ObjectStoreDbMetadata {
        content_hash,
        bucket_hash,
        object_count,
        total_bytes,
        version_count,
        part_count,
        snapshot_ms,
        storage_class: storage_class.min(2),
    }
}

// ── Bridge 2: ObjectStore → Cache (object cache) ─────────────────────────

/// Object cache entry for ALICE-Cache.
///
/// Caches object data references so that repeated GET requests for hot
/// objects can be served from cache rather than re-fetching from the backing
/// store.  TTL is extended for versioned objects (immutable) and reduced
/// for objects in active multipart uploads.
pub struct ObjectStoreCacheEntry {
    /// FNV-1a hash of the bucket+key combined, used as the cache key.
    pub content_hash: u64,
    /// FNV-1a hash of the bucket name.
    pub bucket_hash: u64,
    /// Object size in bytes.
    pub total_bytes: u64,
    /// Version count for this object (0 when versioning is off).
    pub version_count: u64,
    /// Number of pending multipart parts (0 for complete objects).
    pub part_count: u32,
    /// Time-to-live in seconds for this cache entry.
    pub ttl_seconds: u32,
    /// Storage class (0=standard, 1=infrequent, 2=archive).
    pub storage_class: u8,
}

/// Build an object cache entry with versioning-aware TTL.
///
/// TTL derivation (branchless):
/// - part_count > 0  → 0 s   (in-flight multipart, don't cache)
/// - version_count > 0 → 600 s (versioned, effectively immutable)
/// - else             → 300 s (standard)
///
/// Uses integer conditions to select TTL without branching.
#[inline]
#[must_use]
pub fn objectstore_to_cache_entry(
    bucket: &[u8],
    key: &[u8],
    total_bytes: u64,
    version_count: u64,
    part_count: u32,
    storage_class: u8,
) -> ObjectStoreCacheEntry {
    let bucket_hash = fnv1a(bucket);
    // Composite hash: XOR bucket and key hashes for unique cache key.
    let content_hash = bucket_hash ^ fnv1a(key);
    // Branchless TTL selection.
    let has_parts = (part_count > 0) as u32;
    let is_versioned = (version_count > 0) as u32;
    // in-flight → 0, versioned → 600, standard → 300
    let ttl_seconds = (1 - has_parts) * (is_versioned * 300 + 300);
    ObjectStoreCacheEntry {
        content_hash,
        bucket_hash,
        total_bytes,
        version_count,
        part_count,
        ttl_seconds,
        storage_class: storage_class.min(2),
    }
}

// ── Bridge 3: ObjectStore → Analytics (storage metrics) ──────────────────

/// Storage metrics event for ALICE-Analytics ingestion.
///
/// Aggregates per-bucket operation counters so that the analytics layer
/// can build cost-attribution and usage-trend charts without storing raw
/// request payloads.
pub struct ObjectStoreAnalyticsEvent {
    /// FNV-1a hash of the bucket name.
    pub content_hash: u64,
    /// FNV-1a hash of the bucket name (alias for downstream joins).
    pub bucket_hash: u64,
    /// Total objects in the bucket at the time of sampling.
    pub object_count: u64,
    /// Total bytes stored at the time of sampling.
    pub total_bytes: u64,
    /// Number of GET operations in the sampling interval.
    pub version_count: u64,
    /// Number of PUT operations in the sampling interval.
    pub part_count: u32,
    /// Unix timestamp in milliseconds when the interval ended.
    pub timestamp_ms: u64,
    /// Estimated monthly storage cost in microdollars (rate: 23 µ$/GB/month).
    pub cost_microdollars: u64,
}

/// Build a storage metrics event for ALICE-Analytics.
///
/// `cost_microdollars` is estimated at 23 µ$/GB/month:
/// `(total_bytes / 1_073_741_824) * 23` — integer division then multiply,
/// no floating-point, no branches.
#[inline]
#[must_use]
pub fn objectstore_to_analytics_event(
    bucket: &[u8],
    object_count: u64,
    total_bytes: u64,
    get_ops: u64,
    put_ops: u32,
    timestamp_ms: u64,
) -> ObjectStoreAnalyticsEvent {
    let bucket_hash = fnv1a(bucket);
    let content_hash = bucket_hash;
    // Cost estimate: 23 µ$/GB/month — integer, no FP.
    let gb = total_bytes / 1_073_741_824;
    let cost_microdollars = gb * 23;
    ObjectStoreAnalyticsEvent {
        content_hash,
        bucket_hash,
        object_count,
        total_bytes,
        version_count: get_ops,
        part_count: put_ops,
        timestamp_ms,
        cost_microdollars,
    }
}

// ── Bridge 4: ObjectStore → CDN (delivery) ───────────────────────────────

/// CDN delivery record for ALICE-CDN.
///
/// Describes an object to be served via CDN so that the CDN layer can
/// pre-warm its edge caches and apply appropriate cache-control headers
/// without re-inspecting the object store.
pub struct ObjectStoreCdnDelivery {
    /// FNV-1a hash of the bucket+key combined, used as the CDN cache key.
    pub content_hash: u64,
    /// FNV-1a hash of the bucket name.
    pub bucket_hash: u64,
    /// Object size in bytes (used for Content-Length header).
    pub total_bytes: u64,
    /// Number of active versions for this object.
    pub version_count: u64,
    /// Number of pending multipart parts (0 for complete objects).
    pub part_count: u32,
    /// CDN edge TTL in seconds derived from object size and versioning.
    pub edge_ttl_seconds: u32,
    /// Storage class (0=standard, 1=infrequent, 2=archive).
    pub storage_class: u8,
}

/// Build a CDN delivery record for ALICE-CDN.
///
/// `edge_ttl_seconds` derivation:
/// - part_count > 0       → 0   (incomplete object, do not serve)
/// - total_bytes > 10 MB  → 86400 (large object, cache aggressively)
/// - else                 → 3600 (small object, standard edge TTL)
///
/// Branchless: integer condition masks select the TTL bucket.
#[inline]
#[must_use]
pub fn objectstore_to_cdn_delivery(
    bucket: &[u8],
    key: &[u8],
    total_bytes: u64,
    version_count: u64,
    part_count: u32,
    storage_class: u8,
) -> ObjectStoreCdnDelivery {
    let bucket_hash = fnv1a(bucket);
    let content_hash = bucket_hash ^ fnv1a(key);
    let has_parts = (part_count > 0) as u32;
    let is_large = (total_bytes > 10 * 1_048_576) as u32;
    // Branchless TTL: in-flight=0, large=86400, small=3600.
    let edge_ttl_seconds = (1 - has_parts) * (is_large * 82800 + 3600);
    ObjectStoreCdnDelivery {
        content_hash,
        bucket_hash,
        total_bytes,
        version_count,
        part_count,
        edge_ttl_seconds,
        storage_class: storage_class.min(2),
    }
}

// ── Bridge 5: ObjectStore → Backup (replication) ─────────────────────────

/// Replication record for ALICE-Backup.
///
/// Describes a bucket replication job so that the backup layer can track
/// cross-region copy progress and verify object integrity via hash comparison.
pub struct ObjectStoreBackupReplication {
    /// FNV-1a hash of the bucket name used as the replication job key.
    pub content_hash: u64,
    /// FNV-1a hash of the bucket name (alias).
    pub bucket_hash: u64,
    /// Total objects to replicate.
    pub object_count: u64,
    /// Total bytes to transfer in the replication job.
    pub total_bytes: u64,
    /// Number of object versions included in this replication.
    pub version_count: u64,
    /// Number of multipart parts pending replication.
    pub part_count: u32,
    /// Estimated compressed transfer size in bytes (70% ratio heuristic).
    pub compressed_bytes: u64,
    /// Unix timestamp in milliseconds when the replication job was created.
    pub created_ms: u64,
}

/// Build a replication record for ALICE-Backup.
///
/// `compressed_bytes` is estimated as `total_bytes * 7 / 10` — integer
/// multiply then divide (70% of original), no floating-point, no branches.
#[inline]
#[must_use]
pub fn objectstore_to_backup_replication(
    bucket: &[u8],
    object_count: u64,
    total_bytes: u64,
    version_count: u64,
    part_count: u32,
    created_ms: u64,
) -> ObjectStoreBackupReplication {
    let bucket_hash = fnv1a(bucket);
    let content_hash = bucket_hash;
    // 70% compression ratio heuristic: * 7 / 10.
    let compressed_bytes = total_bytes * 7 / 10;
    ObjectStoreBackupReplication {
        content_hash,
        bucket_hash,
        object_count,
        total_bytes,
        version_count,
        part_count,
        compressed_bytes,
        created_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_objectstore_to_db_metadata_basic() {
        let m = objectstore_to_db_metadata(
            b"my-bucket",
            10_000,
            500_000_000,
            3,
            0,
            1_700_000_000_000,
            0,
        );
        assert_eq!(m.content_hash, fnv1a(b"my-bucket"));
        assert_eq!(m.bucket_hash, fnv1a(b"my-bucket"));
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.object_count, 10_000);
        assert_eq!(m.total_bytes, 500_000_000);
        assert_eq!(m.version_count, 3);
        assert_eq!(m.storage_class, 0);
    }

    #[test]
    fn test_objectstore_to_db_metadata_storage_class_clamped() {
        let m = objectstore_to_db_metadata(b"x", 0, 0, 0, 0, 0, 99);
        assert_eq!(m.storage_class, 2, "storage_class clamped to 2");
    }

    #[test]
    fn test_objectstore_to_cache_entry_standard_ttl() {
        let e = objectstore_to_cache_entry(b"bucket", b"key.bin", 1024, 0, 0, 0);
        assert_eq!(e.ttl_seconds, 300);
        assert_ne!(e.content_hash, 0);
    }

    #[test]
    fn test_objectstore_to_cache_entry_versioned_ttl() {
        let e = objectstore_to_cache_entry(b"bucket", b"key.bin", 1024, 5, 0, 0);
        assert_eq!(e.ttl_seconds, 600);
    }

    #[test]
    fn test_objectstore_to_cache_entry_inflight_no_cache() {
        let e = objectstore_to_cache_entry(b"bucket", b"upload.dat", 0, 0, 7, 0);
        assert_eq!(e.ttl_seconds, 0, "in-flight multipart must not be cached");
    }

    #[test]
    fn test_objectstore_to_analytics_event_cost_estimate() {
        // 2 GB → cost = 2 * 23 = 46 µ$
        let ev = objectstore_to_analytics_event(b"logs", 100, 2 * 1_073_741_824, 50, 10, 0);
        assert_eq!(ev.cost_microdollars, 46);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_objectstore_to_cdn_delivery_edge_ttl() {
        // Small object → 3600 s
        let small = objectstore_to_cdn_delivery(b"b", b"small.css", 4096, 0, 0, 0);
        assert_eq!(small.edge_ttl_seconds, 3600);
        // Large object (> 10 MB) → 86400 s
        let large = objectstore_to_cdn_delivery(b"b", b"video.mp4", 50 * 1_048_576, 0, 0, 0);
        assert_eq!(large.edge_ttl_seconds, 86400);
        // In-flight multipart → 0 s
        let inflight = objectstore_to_cdn_delivery(b"b", b"upload.dat", 0, 0, 3, 0);
        assert_eq!(inflight.edge_ttl_seconds, 0);
    }

    #[test]
    fn test_objectstore_to_backup_replication_compressed_bytes() {
        // compressed_bytes = total_bytes * 7 / 10
        let r = objectstore_to_backup_replication(b"archive-bucket", 500, 1_000_000, 2, 0, 0);
        assert_eq!(r.compressed_bytes, 700_000);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.bucket_hash, fnv1a(b"archive-bucket"));
    }
}
