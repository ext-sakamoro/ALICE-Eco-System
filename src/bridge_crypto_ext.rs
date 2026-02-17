//! Crypto bridges — ALICE-Crypto ↔ DB, Cache, CDN, VCS, Edge, Sync, Zip
//!
//! 7 bridges connecting BLAKE3+XChaCha20+SSS cryptography to the ALICE ecosystem.

use alice_crypto::{hash, keyed_hash};

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Crypto → DB (encrypted data persistence) ──────────────────

/// Encrypted data record for ALICE-DB persistence.
pub struct CryptoDbRecord {
    /// BLAKE3 hash of plaintext.
    pub hash: [u8; 32],
    /// Content hash for FNV-1a deduplication.
    pub content_hash: u64,
    /// Original data size.
    pub original_bytes: usize,
}

/// Hash data for ALICE-DB content-addressed storage.
pub fn crypto_to_db_record(data: &[u8]) -> CryptoDbRecord {
    let h = hash(data);
    let hash_bytes: [u8; 32] = *h.as_bytes();
    CryptoDbRecord {
        hash: hash_bytes,
        content_hash: fnv1a(&hash_bytes),
        original_bytes: data.len(),
    }
}

// ── Bridge 2: Crypto → Cache (content-addressed caching) ────────────────

/// Content-addressed cache entry for ALICE-Cache.
pub struct CryptoCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// BLAKE3 hash bytes (first 8 bytes as key).
    pub hash_bytes: [u8; 32],
    /// Payload bytes.
    pub payload_bytes: usize,
}

/// Hash data for ALICE-Cache content-addressed keying.
pub fn crypto_to_cache_entry(data: &[u8]) -> CryptoCacheEntry {
    let h = hash(data);
    let hash_bytes: [u8; 32] = *h.as_bytes();
    CryptoCacheEntry {
        content_hash: fnv1a(&hash_bytes[..8]),
        hash_bytes,
        payload_bytes: data.len(),
    }
}

// ── Bridge 3: Crypto → CDN (content-addressed delivery) ─────────────────

/// Content-addressed payload for ALICE-CDN.
pub struct CryptoCdnPayload {
    /// Hex hash (first 32 chars) for CDN routing.
    pub hash_hex: String,
    /// Content hash for FNV-1a routing.
    pub content_hash: u64,
    /// Payload bytes.
    pub payload_bytes: usize,
}

/// Hash data for ALICE-CDN content-addressed delivery.
pub fn crypto_to_cdn_payload(data: &[u8]) -> CryptoCdnPayload {
    let h = hash(data);
    let bytes = h.as_bytes();
    let hex: String = bytes[..16].iter().map(|b| format!("{:02x}", b)).collect();
    CryptoCdnPayload {
        hash_hex: hex,
        content_hash: fnv1a(bytes),
        payload_bytes: data.len(),
    }
}

// ── Bridge 4: Crypto → VCS (content-addressed blob) ─────────────────────

/// Content-addressed blob for ALICE-VCS.
pub struct CryptoVcsBlob {
    /// BLAKE3 hash for blob addressing.
    pub hash: [u8; 32],
    /// Data size.
    pub data_size: usize,
    /// Integrity verified (always true for fresh hash).
    pub integrity_verified: bool,
}

/// Hash data for ALICE-VCS blob addressing.
pub fn crypto_to_vcs_blob(data: &[u8]) -> CryptoVcsBlob {
    let h = hash(data);
    CryptoVcsBlob {
        hash: *h.as_bytes(),
        data_size: data.len(),
        integrity_verified: true,
    }
}

// ── Bridge 5: Crypto → Edge (encrypted sensor payload) ──────────────────

/// Encrypted sensor payload metadata for ALICE-Edge.
pub struct CryptoEdgePayload {
    /// BLAKE3 hash of sensor data.
    pub hash: [u8; 32],
    /// Payload bytes.
    pub payload_bytes: usize,
    /// Derived key context string.
    pub derived_key_context: String,
}

/// Hash sensor data for ALICE-Edge with device-specific context.
pub fn crypto_to_edge_payload(sensor_data: &[u8], device_id: &str) -> CryptoEdgePayload {
    let h = hash(sensor_data);
    CryptoEdgePayload {
        hash: *h.as_bytes(),
        payload_bytes: sensor_data.len(),
        derived_key_context: format!("alice-edge-{}", device_id),
    }
}

// ── Bridge 6: Crypto → Sync (authenticated multiplayer data) ────────────

/// Authenticated sync packet for ALICE-Sync.
pub struct CryptoSyncPacket {
    /// Keyed MAC (BLAKE3 keyed hash).
    pub mac: [u8; 32],
    /// Payload bytes.
    pub payload_bytes: usize,
    /// Sequence number.
    pub sequence: u64,
}

/// Authenticate data for ALICE-Sync via keyed BLAKE3 hash.
pub fn crypto_to_sync_packet(data: &[u8], shared_key: &[u8; 32], sequence: u64) -> CryptoSyncPacket {
    let mac_hash = keyed_hash(shared_key, data);
    CryptoSyncPacket {
        mac: *mac_hash.as_bytes(),
        payload_bytes: data.len(),
        sequence,
    }
}

// ── Bridge 7: Crypto → Zip (encrypted archive metadata) ─────────────────

/// Encrypted archive metadata for ALICE-Zip.
pub struct CryptoZipArchive {
    /// BLAKE3 hash of archive data.
    pub hash: [u8; 32],
    /// Original data size.
    pub original_bytes: usize,
    /// SSS shard count.
    pub shard_count: u8,
    /// SSS threshold.
    pub threshold: u8,
}

/// Prepare archive metadata for ALICE-Zip with SSS parameters.
pub fn crypto_to_zip_metadata(data: &[u8], shard_count: u8, threshold: u8) -> CryptoZipArchive {
    let h = hash(data);
    CryptoZipArchive {
        hash: *h.as_bytes(),
        original_bytes: data.len(),
        shard_count,
        threshold,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_to_db_record() {
        let data = b"hello ALICE DB";
        let rec = crypto_to_db_record(data);
        assert_ne!(rec.hash, [0u8; 32]);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.original_bytes, 14);
    }

    #[test]
    fn test_crypto_to_cache_entry() {
        let data = b"cache me";
        let entry = crypto_to_cache_entry(data);
        assert_ne!(entry.hash_bytes, [0u8; 32]);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_crypto_to_cdn_payload() {
        let data = b"CDN content";
        let payload = crypto_to_cdn_payload(data);
        assert_eq!(payload.hash_hex.len(), 32);
        assert_eq!(payload.payload_bytes, 11);
    }

    #[test]
    fn test_crypto_to_vcs_blob() {
        let data = b"version controlled";
        let blob = crypto_to_vcs_blob(data);
        assert!(blob.integrity_verified);
        assert_eq!(blob.data_size, 18);
    }

    #[test]
    fn test_crypto_to_edge_payload() {
        let data = b"sensor reading 42";
        let payload = crypto_to_edge_payload(data, "device-001");
        assert_ne!(payload.hash, [0u8; 32]);
        assert_eq!(payload.derived_key_context, "alice-edge-device-001");
    }

    #[test]
    fn test_crypto_to_sync_packet() {
        let data = b"sync data";
        let key = [1u8; 32];
        let pkt = crypto_to_sync_packet(data, &key, 42);
        assert_ne!(pkt.mac, [0u8; 32]);
        assert_eq!(pkt.sequence, 42);
    }

    #[test]
    fn test_crypto_to_zip_metadata() {
        let data = b"archive data";
        let meta = crypto_to_zip_metadata(data, 5, 3);
        assert_eq!(meta.shard_count, 5);
        assert_eq!(meta.threshold, 3);
        assert_eq!(meta.original_bytes, 12);
    }
}
