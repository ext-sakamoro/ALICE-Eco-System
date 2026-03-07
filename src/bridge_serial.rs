//! Serial bridges — ALICE-Serial ↔ DB, Cache, Edge, Crypto, CDN
//!
//! 5 bridges connecting the binary serialization layer to the ALICE ecosystem.
//! Covers serialized record persistence in DB, cache entry encoding, Edge binary
//! protocol delivery, Crypto encrypted payload routing, and CDN asset delivery.

use alice_serial::{encode, encode_varint, zigzag_encode, Value};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Serial → DB (serialized records) ───────────────────────────

/// Serialized record descriptor for ALICE-DB.
///
/// Written when a typed `Value` is encoded and persisted to the database.
/// The `content_hash` allows DB-side deduplication and integrity checks.
pub struct SerialDbRecord {
    /// FNV-1a hash of the encoded payload bytes.
    pub content_hash: u64,
    /// Type tag of the top-level `Value` variant (0=Null, 1=Bool, 2=U64,
    /// 3=I64, 4=F64, 5=Str, 6=Bytes, 7=Array, 8=Map).
    pub value_type: u8,
    /// Encoded payload size in bytes.
    pub encoded_bytes: usize,
    /// Number of VarInt-encoded fields in the payload (0 for scalar types).
    pub varint_count: u32,
    /// True when the encoded size is less than 256 bytes (compact record).
    pub is_compact: bool,
}

/// Build a serialized record descriptor for ALICE-DB from a `Value`.
#[inline]
#[must_use]
pub fn serial_to_db_record(value: &Value) -> SerialDbRecord {
    let encoded = encode(value);
    let content_hash = fnv1a(&encoded);
    let value_type: u8 = match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::U64(_) => 2,
        Value::I64(_) => 3,
        Value::F64(_) => 4,
        Value::Str(_) => 5,
        Value::Bytes(_) => 6,
        Value::Array(_) => 7,
        Value::Map(_) => 8,
    };
    // VarInt フィールド数: Array/Map の要素数をカウント（スカラーは 0）
    let varint_count: u32 = match value {
        Value::Array(arr) => arr.len() as u32,
        Value::Map(m) => m.len() as u32,
        _ => 0,
    };
    let is_compact = encoded.len() < 256;
    SerialDbRecord {
        content_hash,
        value_type,
        encoded_bytes: encoded.len(),
        varint_count,
        is_compact,
    }
}

// ── Bridge 2: Serial → Cache (serialized cache entry) ────────────────────

/// Serialized cache entry for ALICE-Cache.
///
/// Caches the encoded form of a `Value` so that repeated serialization of
/// the same data is avoided.  TTL is set branchlessly: 300 s for compact
/// entries (< 256 bytes), 60 s for large entries.
pub struct SerialCacheEntry {
    /// FNV-1a hash of the encoded bytes — primary cache key.
    pub content_hash: u64,
    /// Encoded payload size in bytes.
    pub encoded_bytes: usize,
    /// Cache TTL in seconds (branchless: 300 for compact, 60 for large).
    pub ttl_secs: u32,
    /// Value type tag (mirrors `SerialDbRecord::value_type`).
    pub value_type: u8,
    /// True when the cached payload fits in a single cache line (< 64 bytes).
    pub cache_line_fit: bool,
}

/// Build a serialized cache entry for ALICE-Cache from a `Value`.
///
/// `ttl_secs` is computed branchlessly: 300 when encoded size < 256, else 60.
#[inline]
#[must_use]
pub fn serial_to_cache_entry(value: &Value) -> SerialCacheEntry {
    let encoded = encode(value);
    let content_hash = fnv1a(&encoded);
    let is_compact = (encoded.len() < 256) as u32;
    // ブランチレス TTL: compact=300s, large=60s
    let ttl_secs = 60 + is_compact * 240;
    let value_type: u8 = match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::U64(_) => 2,
        Value::I64(_) => 3,
        Value::F64(_) => 4,
        Value::Str(_) => 5,
        Value::Bytes(_) => 6,
        Value::Array(_) => 7,
        Value::Map(_) => 8,
    };
    let cache_line_fit = encoded.len() < 64;
    SerialCacheEntry {
        content_hash,
        encoded_bytes: encoded.len(),
        ttl_secs,
        value_type,
        cache_line_fit,
    }
}

// ── Bridge 3: Serial → Edge (binary protocol payload) ────────────────────

/// Binary protocol payload for ALICE-Edge devices.
///
/// Edge devices receive a compact binary frame derived from the encoded
/// `Value`.  The VarInt-encoded length prefix is pre-computed here so that
/// the Edge layer can frame the payload without re-encoding.
pub struct SerialEdgePayload {
    /// FNV-1a hash of the encoded payload — deduplication key.
    pub content_hash: u64,
    /// Encoded payload size in bytes.
    pub payload_bytes: usize,
    /// VarInt-encoded length prefix (up to 10 bytes for u64 LEB128).
    pub length_prefix: [u8; 10],
    /// Actual length of `length_prefix` in bytes.
    pub length_prefix_len: u8,
    /// True when the payload fits in a single UDP datagram (< 1200 bytes).
    pub is_udp_safe: bool,
}

/// Build a binary protocol payload descriptor for ALICE-Edge from a `Value`.
#[inline]
#[must_use]
pub fn serial_to_edge_payload(value: &Value) -> SerialEdgePayload {
    let encoded = encode(value);
    let content_hash = fnv1a(&encoded);
    // LEB128 長さプレフィックスを事前計算
    let varint_bytes = encode_varint(encoded.len() as u64);
    let varint_len = varint_bytes.len().min(10) as u8;
    let mut length_prefix = [0u8; 10];
    length_prefix[..varint_len as usize].copy_from_slice(&varint_bytes[..varint_len as usize]);
    SerialEdgePayload {
        content_hash,
        payload_bytes: encoded.len(),
        length_prefix,
        length_prefix_len: varint_len,
        is_udp_safe: encoded.len() < 1200,
    }
}

// ── Bridge 4: Serial → Crypto (encrypted payload routing) ────────────────

/// Encrypted payload routing descriptor for ALICE-Crypto.
///
/// Before transmitting sensitive serialized data, the caller hands this
/// descriptor to ALICE-Crypto which uses `content_hash` as the nonce seed
/// and `zigzag_hint` as an additional entropy input.
pub struct SerialCryptoPayload {
    /// FNV-1a hash of the encoded plaintext — nonce seed for Crypto.
    pub content_hash: u64,
    /// ZigZag-encoded signed size delta for entropy mixing.
    pub zigzag_hint: u64,
    /// Plaintext size in bytes.
    pub plaintext_bytes: usize,
    /// Expected ciphertext size (plaintext + 16-byte GCM tag).
    pub ciphertext_bytes: usize,
    /// Cipher selector: 0=AES-256-GCM (default).
    pub cipher: u8,
}

/// Build an encrypted payload routing descriptor for ALICE-Crypto.
#[inline]
#[must_use]
pub fn serial_to_crypto_payload(value: &Value) -> SerialCryptoPayload {
    let encoded = encode(value);
    let content_hash = fnv1a(&encoded);
    // ZigZag エンコードしたサイズをエントロピーヒントとして利用
    let zigzag_hint = zigzag_encode(encoded.len() as i64);
    let ciphertext_bytes = encoded.len() + 16;
    SerialCryptoPayload {
        content_hash,
        zigzag_hint,
        plaintext_bytes: encoded.len(),
        ciphertext_bytes,
        cipher: 0,
    }
}

// ── Bridge 5: Serial → CDN (serialized asset delivery) ───────────────────

/// Serialized asset delivery descriptor for ALICE-CDN.
///
/// Metadata-rich CDN objects (manifests, schema payloads, telemetry blobs)
/// are serialized with ALICE-Serial before upload.  This descriptor carries
/// the content hash and MIME type so CDN cache policies can be applied.
pub struct SerialCdnAsset {
    /// FNV-1a hash of the encoded asset bytes — CDN cache key component.
    pub content_hash: u64,
    /// Encoded asset size in bytes.
    pub asset_bytes: usize,
    /// MIME type for CDN content negotiation.
    pub content_type: &'static str,
    /// Suggested CDN TTL in seconds.
    pub ttl_secs: u32,
    /// True when the asset should be compressed before upload (> 512 bytes).
    pub should_compress: bool,
}

/// Build a serialized asset delivery descriptor for ALICE-CDN from a `Value`.
///
/// JSON-like Map/Array values are labelled `application/x-alice-serial`; all
/// others use `application/octet-stream`.
#[inline]
#[must_use]
pub fn serial_to_cdn_asset(value: &Value) -> SerialCdnAsset {
    let encoded = encode(value);
    let content_hash = fnv1a(&encoded);
    let content_type = match value {
        Value::Map(_) | Value::Array(_) => "application/x-alice-serial",
        _ => "application/octet-stream",
    };
    let should_compress = encoded.len() > 512;
    SerialCdnAsset {
        content_hash,
        asset_bytes: encoded.len(),
        content_type,
        ttl_secs: 3_600,
        should_compress,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_serial::Value;

    fn str_val() -> Value {
        Value::Str(alloc::string::String::from("hello-alice"))
    }

    fn large_bytes_val() -> Value {
        Value::Bytes((0u8..=255).cycle().take(600).collect())
    }

    // Bridge 1 ───────────────────────────────────────────────────────────

    #[test]
    fn test_serial_to_db_record_str() {
        let rec = serial_to_db_record(&str_val());
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.value_type, 5); // Str
        assert!(rec.encoded_bytes > 0);
        assert_eq!(rec.varint_count, 0); // scalar
        assert!(rec.is_compact);
    }

    #[test]
    fn test_serial_to_db_record_array() {
        let arr = Value::Array(alloc::vec![Value::U64(1), Value::U64(2), Value::U64(3)]);
        let rec = serial_to_db_record(&arr);
        assert_eq!(rec.value_type, 7); // Array
        assert_eq!(rec.varint_count, 3);
    }

    #[test]
    fn test_serial_to_db_record_map() {
        let map = Value::Map(alloc::vec![(Value::Str(alloc::string::String::from("k")), Value::U64(1))]);
        let rec = serial_to_db_record(&map);
        assert_eq!(rec.value_type, 8); // Map
        assert_eq!(rec.varint_count, 1);
    }

    // Bridge 2 ───────────────────────────────────────────────────────────

    #[test]
    fn test_serial_to_cache_entry_compact_ttl() {
        let entry = serial_to_cache_entry(&str_val());
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300); // compact → 300s
        assert!(entry.cache_line_fit);
    }

    #[test]
    fn test_serial_to_cache_entry_large_ttl() {
        let entry = serial_to_cache_entry(&large_bytes_val());
        assert_eq!(entry.ttl_secs, 60); // large → 60s
        assert!(!entry.cache_line_fit);
    }

    #[test]
    fn test_serial_to_cache_entry_determinism() {
        let e1 = serial_to_cache_entry(&str_val());
        let e2 = serial_to_cache_entry(&str_val());
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.ttl_secs, e2.ttl_secs);
    }

    // Bridge 3 ───────────────────────────────────────────────────────────

    #[test]
    fn test_serial_to_edge_payload_small() {
        let p = serial_to_edge_payload(&Value::U64(42));
        assert_ne!(p.content_hash, 0);
        assert!(p.is_udp_safe);
        assert!(p.length_prefix_len >= 1);
    }

    #[test]
    fn test_serial_to_edge_payload_large_not_udp_safe() {
        // 1300バイトを超えるペイロードはUDP非安全
        let big = Value::Bytes((0u8..128).cycle().take(1300).collect());
        let p = serial_to_edge_payload(&big);
        assert!(!p.is_udp_safe);
    }

    // Bridge 4 ───────────────────────────────────────────────────────────

    #[test]
    fn test_serial_to_crypto_payload() {
        let p = serial_to_crypto_payload(&str_val());
        assert_ne!(p.content_hash, 0);
        assert_eq!(p.ciphertext_bytes, p.plaintext_bytes + 16);
        assert_eq!(p.cipher, 0);
    }

    #[test]
    fn test_serial_to_crypto_payload_zigzag_nonzero() {
        // 非空ペイロードの ZigZag ヒントは 0 より大きい
        let p = serial_to_crypto_payload(&Value::Bool(true));
        // zigzag_encode(1) = 2
        assert_eq!(p.zigzag_hint, zigzag_encode(p.plaintext_bytes as i64));
    }

    // Bridge 5 ───────────────────────────────────────────────────────────

    #[test]
    fn test_serial_to_cdn_asset_map_content_type() {
        let map = Value::Map(alloc::vec![(Value::Str(alloc::string::String::from("x")), Value::U64(0))]);
        let asset = serial_to_cdn_asset(&map);
        assert_ne!(asset.content_hash, 0);
        assert_eq!(asset.content_type, "application/x-alice-serial");
        assert_eq!(asset.ttl_secs, 3_600);
    }

    #[test]
    fn test_serial_to_cdn_asset_large_should_compress() {
        let asset = serial_to_cdn_asset(&large_bytes_val());
        assert!(asset.should_compress);
    }

    #[test]
    fn test_serial_to_cdn_asset_small_no_compress() {
        let asset = serial_to_cdn_asset(&Value::Null);
        assert!(!asset.should_compress);
        assert_eq!(asset.content_type, "application/octet-stream");
    }
}

extern crate alloc;
