//! Crypto bridges — ALICE-Crypto ↔ Analytics, DB, Cache, Edge
//!
//! 5 bridges connecting ALICE-Crypto key management, secret sharing, and
//! authenticated encryption primitives to the ALICE ecosystem.
//!
//! These bridges focus on Key/Shard/Nonce/Hash *structure* metadata, complementing
//! `bridge_crypto_ext` which operates on raw data payloads.

use alice_crypto::{hash, Hash, Key, Nonce, Shard, TAG_SIZE};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Key → Analytics (key lifecycle event) ─────────────────────

/// Key lifecycle analytics event for ALICE-Analytics.
///
/// Captures metadata about a cryptographic key without exposing the key material.
/// The `key_fingerprint` is a BLAKE3 hash of the key bytes, used as a stable
/// identifier across analytics pipelines.
pub struct CryptoKeyAnalyticsEvent {
    /// FNV-1a content hash over fingerprint and timestamp bytes.
    pub content_hash: u64,
    /// BLAKE3 fingerprint of the key material (safe to log).
    pub key_fingerprint: [u8; 32],
    /// Key size in bytes (always 32 for XChaCha20-Poly1305).
    pub key_size: u16,
    /// Event timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a [`Key`] into an analytics event capturing key metadata.
///
/// The key material is never stored directly; only its BLAKE3 fingerprint
/// is retained for correlation purposes.
#[inline]
#[must_use]
pub fn crypto_key_to_analytics(key: &Key, timestamp_ns: u64) -> CryptoKeyAnalyticsEvent {
    let fingerprint = hash(&key.0);
    let fp_bytes = *fingerprint.as_bytes();

    // Hash input: fingerprint first 16 bytes || timestamp (8 bytes)
    let mut buf = [0u8; 24];
    buf[0..16].copy_from_slice(&fp_bytes[..16]);
    buf[16..24].copy_from_slice(&timestamp_ns.to_le_bytes());

    CryptoKeyAnalyticsEvent {
        content_hash: fnv1a(&buf),
        key_fingerprint: fp_bytes,
        key_size: Key::SIZE as u16,
        timestamp_ns,
    }
}

// ── Bridge 2: Shard → DB (shard persistence record) ─────────────────────

/// SSS shard persistence record for ALICE-DB.
///
/// Stores the shard coordinate and integrity hash without exposing the Y-values.
/// The shard data itself should be encrypted before DB storage via ALICE-Crypto
/// stream cipher.
pub struct CryptoShardDbRecord {
    /// FNV-1a content hash over x-coordinate and shard length bytes.
    pub content_hash: u64,
    /// X-coordinate of the shard (1-255).
    pub x_coordinate: u8,
    /// Number of secret bytes encoded in this shard.
    pub shard_length: u32,
    /// BLAKE3 hash of the Y-values for integrity verification on retrieval.
    pub y_hash: [u8; 32],
}

/// Convert a [`Shard`] into a DB persistence record.
///
/// The Y-values are hashed (not stored) so the DB layer can verify shard
/// integrity without holding the secret shares in plaintext.
#[inline]
#[must_use]
pub fn crypto_shard_to_db(shard: &Shard) -> CryptoShardDbRecord {
    let y_hash = hash(&shard.y);
    let shard_length = shard.y.len() as u32;

    // Hash input: x (1 byte) || shard_length LE (4 bytes)
    let mut buf = [0u8; 5];
    buf[0] = shard.x;
    buf[1..5].copy_from_slice(&shard_length.to_le_bytes());

    CryptoShardDbRecord {
        content_hash: fnv1a(&buf),
        x_coordinate: shard.x,
        shard_length,
        y_hash: *y_hash.as_bytes(),
    }
}

// ── Bridge 3: Hash → Cache (content-addressed lookup) ───────────────────

/// BLAKE3 hash cache entry for ALICE-Cache.
///
/// Provides a content-addressed lookup key derived from the hash bytes.
/// TTL is branchless: known hashes (fingerprint of well-known data) get
/// longer TTL; novel hashes get shorter TTL to allow faster eviction.
pub struct CryptoHashCacheEntry {
    /// FNV-1a content hash of the BLAKE3 hash bytes — used as cache key.
    pub content_hash: u64,
    /// Full 32-byte BLAKE3 hash.
    pub hash_bytes: [u8; 32],
    /// True if the hash matches a known/pinned value.
    pub is_pinned: bool,
    /// Cache TTL in seconds. Branchless: pinned=3600s, unpinned=120s.
    pub ttl_secs: u32,
}

/// Convert a [`alice_crypto::Hash`] into a cache entry.
///
/// `pinned` indicates whether this hash corresponds to a well-known content
/// address that should be retained longer in cache.
///
/// Branchless TTL: `3600 - (1 - pinned as u32) * 3480`.
/// - pinned=true  → 3600 - 0*3480 = 3600s (1 hour)
/// - pinned=false → 3600 - 1*3480 = 120s  (2 minutes)
#[inline]
#[must_use]
pub fn crypto_hash_to_cache(h: &Hash, pinned: bool) -> CryptoHashCacheEntry {
    let hash_bytes = *h.as_bytes();

    // Branchless TTL computation
    let pinned_u32 = pinned as u32;
    let ttl_secs = 3600 - (1 - pinned_u32) * 3480;

    CryptoHashCacheEntry {
        content_hash: fnv1a(&hash_bytes),
        hash_bytes,
        is_pinned: pinned,
        ttl_secs,
    }
}

// ── Bridge 4: Key+Nonce → Edge (encryption context snapshot) ────────────

/// Encryption context snapshot for ALICE-Edge telemetry.
///
/// Captures the fingerprint of the key and the nonce used for a particular
/// encryption operation, along with the payload size, so the edge layer can
/// track encryption throughput and nonce reuse patterns.
pub struct CryptoEdgeEncryptionSnapshot {
    /// FNV-1a content hash over key fingerprint prefix, nonce, and payload size.
    pub content_hash: u64,
    /// BLAKE3 fingerprint of the key (first 8 bytes, truncated for edge bandwidth).
    pub key_fingerprint_short: u64,
    /// Full 24-byte nonce used in this encryption operation.
    pub nonce_bytes: [u8; 24],
    /// Payload size in bytes (plaintext, before encryption overhead).
    pub payload_bytes: u32,
    /// Total ciphertext size including nonce prefix and auth tag.
    pub ciphertext_bytes: u32,
}

/// Convert a [`Key`] and [`Nonce`] pair into an edge encryption snapshot.
///
/// `plaintext_len` is the size of the data encrypted; the ciphertext size
/// is computed as `Nonce::SIZE + plaintext_len + TAG_SIZE`.
#[inline]
#[must_use]
pub fn crypto_key_nonce_to_edge(
    key: &Key,
    nonce: &Nonce,
    plaintext_len: u32,
) -> CryptoEdgeEncryptionSnapshot {
    let fp = hash(&key.0);
    let fp_bytes = fp.as_bytes();

    // Truncated fingerprint: first 8 bytes as u64 LE
    let mut fp_short_buf = [0u8; 8];
    fp_short_buf.copy_from_slice(&fp_bytes[..8]);
    let key_fingerprint_short = u64::from_le_bytes(fp_short_buf);

    let ciphertext_bytes = (Nonce::SIZE as u32) + plaintext_len + (TAG_SIZE as u32);

    // Hash input: fp_short (8) || nonce (24) || plaintext_len LE (4) = 36 bytes
    let mut buf = [0u8; 36];
    buf[0..8].copy_from_slice(&fp_short_buf);
    buf[8..32].copy_from_slice(&nonce.0);
    buf[32..36].copy_from_slice(&plaintext_len.to_le_bytes());

    CryptoEdgeEncryptionSnapshot {
        content_hash: fnv1a(&buf),
        key_fingerprint_short,
        nonce_bytes: nonce.0,
        payload_bytes: plaintext_len,
        ciphertext_bytes,
    }
}

// ── Bridge 5: Seal operation → Analytics (encryption throughput metric) ──

/// Encryption throughput metric for ALICE-Analytics.
///
/// Captures the cost and overhead of a seal (encrypt) operation so the
/// analytics layer can build throughput and overhead-ratio dashboards.
pub struct CryptoSealAnalyticsMetric {
    /// FNV-1a content hash over `plaintext_bytes` and `overhead_bytes`.
    pub content_hash: u64,
    /// Plaintext size in bytes.
    pub plaintext_bytes: u64,
    /// Overhead added by encryption (nonce + auth tag = 40 bytes).
    pub overhead_bytes: u64,
    /// Overhead ratio: overhead / plaintext (0.0 when plaintext is 0).
    pub overhead_ratio: f64,
    /// Event timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Record a seal operation metric for analytics.
///
/// `plaintext_len` is the input size; overhead is always
/// `Nonce::SIZE + TAG_SIZE` (24 + 16 = 40 bytes).
#[inline]
#[must_use]
pub fn crypto_seal_to_analytics(
    plaintext_len: u64,
    timestamp_ns: u64,
) -> CryptoSealAnalyticsMetric {
    let overhead = (Nonce::SIZE + TAG_SIZE) as u64;
    let overhead_ratio = if plaintext_len == 0 {
        0.0
    } else {
        overhead as f64 / plaintext_len as f64
    };

    // Hash input: plaintext_len LE (8) || overhead LE (8)
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&plaintext_len.to_le_bytes());
    buf[8..16].copy_from_slice(&overhead.to_le_bytes());

    CryptoSealAnalyticsMetric {
        content_hash: fnv1a(&buf),
        plaintext_bytes: plaintext_len,
        overhead_bytes: overhead,
        overhead_ratio,
        timestamp_ns,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use alice_crypto::sss;

    // ── Bridge 1: Key → Analytics ──────────────────────────────────────

    #[test]
    fn test_key_to_analytics_basic() {
        let key = Key::generate().unwrap();
        let ts = 1_700_000_000_000_000_000u64;
        let ev = crypto_key_to_analytics(&key, ts);

        assert_ne!(ev.content_hash, 0);
        assert_ne!(ev.key_fingerprint, [0u8; 32]);
        assert_eq!(ev.key_size, 32);
        assert_eq!(ev.timestamp_ns, ts);
    }

    #[test]
    fn test_key_to_analytics_deterministic() {
        let key = Key::from_bytes([0xAB; 32]);
        let ts = 12345u64;
        let ev1 = crypto_key_to_analytics(&key, ts);
        let ev2 = crypto_key_to_analytics(&key, ts);
        assert_eq!(ev1.content_hash, ev2.content_hash);
        assert_eq!(ev1.key_fingerprint, ev2.key_fingerprint);
    }

    #[test]
    fn test_key_to_analytics_different_keys_differ() {
        let k1 = Key::from_bytes([0x01; 32]);
        let k2 = Key::from_bytes([0x02; 32]);
        let ts = 100u64;
        let ev1 = crypto_key_to_analytics(&k1, ts);
        let ev2 = crypto_key_to_analytics(&k2, ts);
        assert_ne!(ev1.content_hash, ev2.content_hash);
        assert_ne!(ev1.key_fingerprint, ev2.key_fingerprint);
    }

    // ── Bridge 2: Shard → DB ───────────────────────────────────────────

    #[test]
    fn test_shard_to_db_basic() {
        let shards = sss::split(b"secret_key_material", 5, 3).unwrap();
        let rec = crypto_shard_to_db(&shards[0]);

        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.x_coordinate, 1);
        assert_eq!(rec.shard_length, 19); // "secret_key_material".len()
        assert_ne!(rec.y_hash, [0u8; 32]);
    }

    #[test]
    fn test_shard_to_db_deterministic() {
        let shards = sss::split(b"test", 3, 2).unwrap();
        let rec1 = crypto_shard_to_db(&shards[0]);
        let rec2 = crypto_shard_to_db(&shards[0]);
        assert_eq!(rec1.content_hash, rec2.content_hash);
        assert_eq!(rec1.y_hash, rec2.y_hash);
    }

    #[test]
    fn test_shard_to_db_different_shards_differ() {
        let shards = sss::split(b"abc", 3, 2).unwrap();
        let rec0 = crypto_shard_to_db(&shards[0]);
        let rec1 = crypto_shard_to_db(&shards[1]);
        // Different x-coordinates produce different content hashes.
        assert_ne!(rec0.content_hash, rec1.content_hash);
        assert_ne!(rec0.x_coordinate, rec1.x_coordinate);
    }

    // ── Bridge 3: Hash → Cache ─────────────────────────────────────────

    #[test]
    fn test_hash_to_cache_pinned() {
        let h = hash(b"well-known content");
        let entry = crypto_hash_to_cache(&h, true);

        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.hash_bytes, *h.as_bytes());
        assert!(entry.is_pinned);
        assert_eq!(entry.ttl_secs, 3600);
    }

    #[test]
    fn test_hash_to_cache_unpinned() {
        let h = hash(b"ephemeral content");
        let entry = crypto_hash_to_cache(&h, false);

        assert!(!entry.is_pinned);
        assert_eq!(entry.ttl_secs, 120);
    }

    #[test]
    fn test_hash_to_cache_deterministic() {
        let h = hash(b"deterministic");
        let e1 = crypto_hash_to_cache(&h, true);
        let e2 = crypto_hash_to_cache(&h, true);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 4: Key+Nonce → Edge ─────────────────────────────────────

    #[test]
    fn test_key_nonce_to_edge_basic() {
        let key = Key::from_bytes([0x42; 32]);
        let nonce = Nonce::from_bytes([0xCD; 24]);
        let snap = crypto_key_nonce_to_edge(&key, &nonce, 1024);

        assert_ne!(snap.content_hash, 0);
        assert_ne!(snap.key_fingerprint_short, 0);
        assert_eq!(snap.nonce_bytes, [0xCD; 24]);
        assert_eq!(snap.payload_bytes, 1024);
        // ciphertext = 24 (nonce) + 1024 (payload) + 16 (tag) = 1064
        assert_eq!(snap.ciphertext_bytes, 1064);
    }

    #[test]
    fn test_key_nonce_to_edge_deterministic() {
        let key = Key::from_bytes([0x11; 32]);
        let nonce = Nonce::from_bytes([0x22; 24]);
        let s1 = crypto_key_nonce_to_edge(&key, &nonce, 256);
        let s2 = crypto_key_nonce_to_edge(&key, &nonce, 256);
        assert_eq!(s1.content_hash, s2.content_hash);
        assert_eq!(s1.key_fingerprint_short, s2.key_fingerprint_short);
    }

    #[test]
    fn test_key_nonce_to_edge_different_nonces_differ() {
        let key = Key::from_bytes([0x33; 32]);
        let n1 = Nonce::from_bytes([0x01; 24]);
        let n2 = Nonce::from_bytes([0x02; 24]);
        let s1 = crypto_key_nonce_to_edge(&key, &n1, 100);
        let s2 = crypto_key_nonce_to_edge(&key, &n2, 100);
        assert_ne!(s1.content_hash, s2.content_hash);
    }

    // ── Bridge 5: Seal → Analytics ─────────────────────────────────────

    #[test]
    fn test_seal_to_analytics_basic() {
        let ts = 9_999_999u64;
        let metric = crypto_seal_to_analytics(1000, ts);

        assert_ne!(metric.content_hash, 0);
        assert_eq!(metric.plaintext_bytes, 1000);
        assert_eq!(metric.overhead_bytes, 40); // 24 + 16
        assert_eq!(metric.timestamp_ns, ts);
        // overhead_ratio = 40 / 1000 = 0.04
        assert!((metric.overhead_ratio - 0.04).abs() < 1e-12);
    }

    #[test]
    fn test_seal_to_analytics_zero_plaintext() {
        let metric = crypto_seal_to_analytics(0, 0);
        assert_eq!(metric.plaintext_bytes, 0);
        assert_eq!(metric.overhead_bytes, 40);
        assert_eq!(metric.overhead_ratio, 0.0);
    }

    #[test]
    fn test_seal_to_analytics_deterministic() {
        let m1 = crypto_seal_to_analytics(500, 1000);
        let m2 = crypto_seal_to_analytics(500, 1000);
        assert_eq!(m1.content_hash, m2.content_hash);
    }

    #[test]
    fn test_seal_to_analytics_different_sizes_differ() {
        let m1 = crypto_seal_to_analytics(100, 0);
        let m2 = crypto_seal_to_analytics(200, 0);
        assert_ne!(m1.content_hash, m2.content_hash);
    }
}
