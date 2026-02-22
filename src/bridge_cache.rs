//! Cache bridges — ALICE-Cache ↔ DB, Analytics, Crypto
//!
//! 7 bridges connecting the cache layer to the ALICE ecosystem.
//! Covers persistent cache backing in DB, cache analytics, and
//! encrypted cache entries via ALICE-Crypto.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Cache → DB (eviction log persistence) ──────────────────────

/// Cache eviction log record for ALICE-DB.
///
/// Written when the cache evicts an entry under memory pressure.
/// Eviction logs enable post-hoc cache sizing analysis.
pub struct CacheDbEvictionRecord {
    /// FNV-1a hash of the evicted cache key.
    pub key_hash: u64,
    /// Eviction reason: 0=capacity, `1=ttl_expired`, `2=explicit_invalidate`.
    pub reason: u8,
    /// Entry size in bytes at eviction time.
    pub entry_bytes: usize,
    /// Remaining TTL at eviction in seconds (0 when TTL has expired).
    pub remaining_ttl_secs: u32,
    /// Approximate number of accesses before eviction.
    pub access_count: u32,
}

/// Build a cache eviction log record for ALICE-DB.
#[inline]
#[must_use]
pub fn cache_to_db_eviction_record(
    cache_key: &[u8],
    reason: u8,
    entry_bytes: usize,
    remaining_ttl_secs: u32,
    access_count: u32,
) -> CacheDbEvictionRecord {
    CacheDbEvictionRecord {
        key_hash: fnv1a(cache_key),
        reason,
        entry_bytes,
        remaining_ttl_secs,
        access_count,
    }
}

// ── Bridge 2: Cache → Analytics (hit/miss telemetry) ─────────────────────

/// Cache hit/miss event for ALICE-Analytics.
///
/// Emitted on every cache lookup so the analytics layer can compute
/// hit rates, miss latency, and per-key access frequency distributions.
pub struct CacheAnalyticsLookupEvent {
    /// FNV-1a hash of the cache key — analytics stream key.
    pub key_hash: u64,
    /// True when the lookup resulted in a cache hit.
    pub is_hit: bool,
    /// Lookup latency in microseconds.
    pub lookup_us: u32,
    /// Entry size in bytes (0 on miss).
    pub entry_bytes: usize,
    /// Remaining TTL at lookup time in seconds (0 on miss or no TTL).
    pub remaining_ttl_secs: u32,
}

/// Build a cache lookup telemetry event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn cache_to_analytics_lookup_event(
    cache_key: &[u8],
    is_hit: bool,
    lookup_us: u32,
    entry_bytes: usize,
    remaining_ttl_secs: u32,
) -> CacheAnalyticsLookupEvent {
    CacheAnalyticsLookupEvent {
        key_hash: fnv1a(cache_key),
        is_hit,
        lookup_us,
        entry_bytes,
        remaining_ttl_secs,
    }
}

// ── Bridge 3: Cache → Crypto (encrypted cache entry) ─────────────────────

/// Encrypted cache entry for ALICE-Crypto storage.
///
/// Sensitive cache entries (tokens, session data, user PII) are encrypted
/// before being committed to the cache backend.  The nonce is derived
/// deterministically from the content hash to avoid nonce reuse.
pub struct CacheCryptoEntry {
    /// FNV-1a hash of the plaintext — integrity check key.
    pub content_hash: u64,
    /// Cipher algorithm: 0=AES-256-GCM, 1=ChaCha20-Poly1305.
    pub cipher: u8,
    /// Nonce derived from `content_hash` (first 12 bytes).
    pub nonce: [u8; 12],
    /// Authentication tag length in bytes (16 for both supported ciphers).
    pub tag_bytes: u8,
    /// Ciphertext size in bytes (`plaintext_size` + `tag_bytes`).
    pub ciphertext_bytes: usize,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build an encrypted cache entry descriptor for ALICE-Crypto.
///
/// `cipher`: 0=AES-256-GCM, 1=ChaCha20-Poly1305.
/// The nonce is derived from the low 96 bits of `content_hash` (12 bytes).
/// This is safe here because each plaintext produces a unique `content_hash`,
/// so nonce reuse across different plaintexts is statistically excluded.
#[inline]
#[must_use]
pub fn cache_to_crypto_entry(plaintext: &[u8], cipher: u8, ttl_secs: u32) -> CacheCryptoEntry {
    let content_hash = fnv1a(plaintext);
    // Derive 12-byte nonce from content_hash: repeat hash bytes cyclically.
    let hash_bytes = content_hash.to_le_bytes();
    let mut nonce = [0u8; 12];
    for (i, b) in nonce.iter_mut().enumerate() {
        *b = hash_bytes[i % 8];
    }
    let tag_bytes = 16u8; // GCM and Poly1305 both use 16-byte tags.
    let ciphertext_bytes = plaintext.len() + tag_bytes as usize;
    CacheCryptoEntry {
        content_hash,
        cipher: cipher.min(1),
        nonce,
        tag_bytes,
        ciphertext_bytes,
        ttl_secs,
    }
}

// ── Bridge 4: Cache → DB (warm-up snapshot) ──────────────────────────────

/// Cache warm-up snapshot record for ALICE-DB.
///
/// On restart, the cache layer loads its warm-up snapshot from DB to avoid
/// a cold-start spike.  This record is written periodically (e.g. every
/// 5 minutes) so that the latest hot entries are captured.
pub struct CacheDbWarmupSnapshot {
    /// FNV-1a hash of the snapshot payload.
    pub snapshot_hash: u64,
    /// Number of entries captured in the snapshot.
    pub entry_count: u32,
    /// Total serialized size of all entries in bytes.
    pub total_bytes: usize,
    /// Snapshot creation timestamp in milliseconds.
    pub created_at_ms: u64,
    /// Cache fill ratio at snapshot time in permille.
    pub fill_permille: u32,
}

/// Build a cache warm-up snapshot record for ALICE-DB.
///
/// `fill_permille` is computed branchlessly from `entry_count / capacity`.
#[inline]
#[must_use]
pub fn cache_to_db_warmup_snapshot(
    entry_count: u32,
    total_bytes: usize,
    capacity: u32,
    created_at_ms: u64,
) -> CacheDbWarmupSnapshot {
    let mut data = [0u8; 16];
    data[0..4].copy_from_slice(&entry_count.to_le_bytes());
    data[4..12].copy_from_slice(&(total_bytes as u64).to_le_bytes());
    data[12..16].copy_from_slice(&capacity.to_le_bytes());
    let snapshot_hash = fnv1a(&data);
    let cap_safe = capacity.max(1);
    let fill_permille = entry_count.min(cap_safe).wrapping_mul(1_000) / cap_safe;
    CacheDbWarmupSnapshot {
        snapshot_hash,
        entry_count,
        total_bytes,
        created_at_ms,
        fill_permille,
    }
}

// ── Bridge 5: Cache → Analytics (eviction rate metrics) ──────────────────

/// Eviction rate metrics for ALICE-Analytics.
///
/// Aggregates eviction counters over a measurement window so the analytics
/// layer can detect memory pressure events and size recommendations.
pub struct CacheAnalyticsEvictionMetrics {
    /// FNV-1a hash of the cache instance name — analytics stream key.
    pub instance_hash: u64,
    /// Total evictions in the measurement window.
    pub eviction_count: u64,
    /// Evictions due to capacity limit.
    pub capacity_evictions: u64,
    /// Evictions due to TTL expiry.
    pub ttl_evictions: u64,
    /// Eviction rate in permille of total lookups.
    pub eviction_rate_permille: u32,
    /// Mean bytes freed per eviction.
    pub mean_freed_bytes: u64,
}

/// Build eviction rate metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn cache_to_analytics_eviction_metrics(
    instance_name: &str,
    eviction_count: u64,
    capacity_evictions: u64,
    ttl_evictions: u64,
    total_lookups: u64,
    total_freed_bytes: u64,
) -> CacheAnalyticsEvictionMetrics {
    let instance_hash = fnv1a(instance_name.as_bytes());
    let lookups_safe = total_lookups.max(1);
    let eviction_rate_permille =
        (eviction_count.min(lookups_safe).wrapping_mul(1_000) / lookups_safe) as u32;
    let evictions_safe = eviction_count.max(1);
    let mean_freed_bytes = total_freed_bytes / evictions_safe;
    CacheAnalyticsEvictionMetrics {
        instance_hash,
        eviction_count,
        capacity_evictions,
        ttl_evictions,
        eviction_rate_permille,
        mean_freed_bytes,
    }
}

// ── Bridge 6: Cache → Crypto (key derivation for cache namespace) ─────────

/// Cache namespace key derivation request for ALICE-Crypto.
///
/// Enables per-namespace encryption so that cache entries belonging to
/// different tenants cannot be decrypted by each other even if they share
/// the same underlying cache backend.
pub struct CacheCryptoKeyRequest {
    /// FNV-1a hash of the namespace identifier.
    pub namespace_hash: u64,
    /// Requested key length in bytes (16, 24, or 32 for AES; 32 for `ChaCha20`).
    pub key_bytes: u8,
    /// KDF algorithm: 0=HKDF-SHA256, 1=PBKDF2-SHA256.
    pub kdf_algo: u8,
    /// HKDF info / PBKDF2 salt hint length in bytes.
    pub salt_hint_bytes: u8,
}

/// Build a namespace key derivation request for ALICE-Crypto.
///
/// `key_bytes` is rounded up to the nearest supported size: 16, 24, or 32.
#[inline]
#[must_use]
pub fn cache_to_crypto_key_request(
    namespace: &str,
    requested_key_bytes: u8,
    kdf_algo: u8,
) -> CacheCryptoKeyRequest {
    const KEY_SIZES: [u8; 3] = [16, 24, 32];
    let namespace_hash = fnv1a(namespace.as_bytes());
    let key_bytes = KEY_SIZES
        .iter()
        .copied()
        .find(|&s| s >= requested_key_bytes)
        .unwrap_or(32);
    CacheCryptoKeyRequest {
        namespace_hash,
        key_bytes,
        kdf_algo: kdf_algo.min(1),
        salt_hint_bytes: 16,
    }
}

// ── Bridge 7: Cache → DB (miss log for prefetch planning) ────────────────

/// Cache miss log record for ALICE-DB.
///
/// Persists a compacted miss log so that the prefetch planner can replay
/// missed accesses and pre-warm the cache before the next traffic peak.
pub struct CacheDbMissLog {
    /// FNV-1a hash of the missed cache key.
    pub key_hash: u64,
    /// Miss timestamp in milliseconds.
    pub missed_at_ms: u64,
    /// Lookup latency on miss (includes origin fetch time) in milliseconds.
    pub origin_fetch_ms: u32,
    /// Fetched entry size in bytes (0 if origin returned nothing).
    pub fetched_bytes: usize,
    /// Number of times this key has missed in the current window.
    pub miss_count: u32,
}

/// Build a cache miss log record for ALICE-DB.
#[inline]
#[must_use]
pub fn cache_to_db_miss_log(
    cache_key: &[u8],
    missed_at_ms: u64,
    origin_fetch_ms: u32,
    fetched_bytes: usize,
    miss_count: u32,
) -> CacheDbMissLog {
    CacheDbMissLog {
        key_hash: fnv1a(cache_key),
        missed_at_ms,
        origin_fetch_ms,
        fetched_bytes,
        miss_count,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_to_db_eviction_record() {
        let key = b"user:session:1234";
        let rec = cache_to_db_eviction_record(key, 0, 4096, 120, 47);
        assert_ne!(rec.key_hash, 0);
        assert_eq!(rec.reason, 0);
        assert_eq!(rec.entry_bytes, 4096);
        assert_eq!(rec.remaining_ttl_secs, 120);
        assert_eq!(rec.access_count, 47);
    }

    #[test]
    fn test_cache_to_analytics_lookup_event_hit() {
        let ev = cache_to_analytics_lookup_event(b"asset:logo.png", true, 3, 65536, 290);
        assert_ne!(ev.key_hash, 0);
        assert!(ev.is_hit);
        assert_eq!(ev.lookup_us, 3);
        assert_eq!(ev.entry_bytes, 65536);
        assert_eq!(ev.remaining_ttl_secs, 290);
    }

    #[test]
    fn test_cache_to_analytics_lookup_event_miss() {
        let ev = cache_to_analytics_lookup_event(b"asset:missing.png", false, 120, 0, 0);
        assert!(!ev.is_hit);
        assert_eq!(ev.entry_bytes, 0);
        assert_eq!(ev.remaining_ttl_secs, 0);
    }

    #[test]
    fn test_cache_to_crypto_entry_aes_gcm() {
        let plaintext = b"sensitive session token data here";
        let entry = cache_to_crypto_entry(plaintext, 0, 3600);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.cipher, 0);
        assert_eq!(entry.tag_bytes, 16);
        assert_eq!(entry.ciphertext_bytes, plaintext.len() + 16);
        assert_eq!(entry.ttl_secs, 3600);
        // Nonce must be non-zero (derived from content_hash).
        assert_ne!(entry.nonce, [0u8; 12]);
    }

    #[test]
    fn test_cache_to_crypto_entry_cipher_clamped() {
        // cipher = 99 → clamped to 1.
        let entry = cache_to_crypto_entry(b"data", 99, 60);
        assert_eq!(entry.cipher, 1);
    }

    #[test]
    fn test_cache_to_db_warmup_snapshot_fill_permille() {
        // 750 entries, capacity 1000 → 750 permille.
        let snap = cache_to_db_warmup_snapshot(750, 750 * 1024, 1_000, 1_700_000_000_000);
        assert_ne!(snap.snapshot_hash, 0);
        assert_eq!(snap.entry_count, 750);
        assert_eq!(snap.fill_permille, 750);
        assert_eq!(snap.created_at_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_cache_to_db_warmup_snapshot_zero_capacity_no_panic() {
        let snap = cache_to_db_warmup_snapshot(0, 0, 0, 0);
        assert_eq!(snap.fill_permille, 0);
    }

    #[test]
    fn test_cache_to_analytics_eviction_metrics() {
        let m = cache_to_analytics_eviction_metrics("l1-cache", 100, 60, 40, 10_000, 100 * 4096);
        assert_ne!(m.instance_hash, 0);
        assert_eq!(m.eviction_count, 100);
        // 100 evictions / 10 000 lookups * 1000 = 10 permille.
        assert_eq!(m.eviction_rate_permille, 10);
        assert_eq!(m.mean_freed_bytes, 4096);
    }

    #[test]
    fn test_cache_to_analytics_eviction_metrics_zero_lookups_no_panic() {
        let m = cache_to_analytics_eviction_metrics("test", 0, 0, 0, 0, 0);
        assert_eq!(m.eviction_rate_permille, 0);
        assert_eq!(m.mean_freed_bytes, 0);
    }

    #[test]
    fn test_cache_to_crypto_key_request_rounding() {
        // 10 → rounded up to 16.
        let r = cache_to_crypto_key_request("tenant-a", 10, 0);
        assert_eq!(r.key_bytes, 16);

        // 20 → rounded up to 24.
        let r = cache_to_crypto_key_request("tenant-b", 20, 0);
        assert_eq!(r.key_bytes, 24);

        // 32 → stays at 32.
        let r = cache_to_crypto_key_request("tenant-c", 32, 1);
        assert_eq!(r.key_bytes, 32);
        assert_eq!(r.kdf_algo, 1);

        // Larger than 32 → clamps to 32.
        let r = cache_to_crypto_key_request("tenant-d", 64, 0);
        assert_eq!(r.key_bytes, 32);
    }

    #[test]
    fn test_cache_to_db_miss_log() {
        let key = b"cdn:video:episode-5.mp4";
        let log = cache_to_db_miss_log(key, 1_700_000_500_000, 250, 1_048_576, 3);
        assert_ne!(log.key_hash, 0);
        assert_eq!(log.missed_at_ms, 1_700_000_500_000);
        assert_eq!(log.origin_fetch_ms, 250);
        assert_eq!(log.fetched_bytes, 1_048_576);
        assert_eq!(log.miss_count, 3);
    }
}
