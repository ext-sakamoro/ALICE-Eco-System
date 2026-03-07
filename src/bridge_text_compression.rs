//! Text-Compression bridges — ALICE-Text-Compression ↔ DB, Cache, CDN, Zip, Codec
//!
//! 5 bridges connecting BWT/MTF/RLE/Huffman compression primitives to the ALICE ecosystem.

use alice_text_compression::{
    build_huffman_codes, bwt_encode, compression_ratio, mtf_encode, rle_encode,
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

// ── Bridge 1: TextCompression → DB (compressed text records) ─────────────

/// Compressed text record for ALICE-DB persistence.
///
/// Stores the full BWT+MTF+RLE compressed pipeline output alongside the
/// primary index needed for BWT decoding, so that the database can store
/// and faithfully restore arbitrary text blobs.
pub struct TextCompressionDbRecord {
    /// FNV-1a hash of the compressed payload (deduplication key).
    pub content_hash: u64,
    /// Original input length in bytes.
    pub original_bytes: usize,
    /// Compressed payload: BWT → MTF → RLE.
    pub compressed: Vec<u8>,
    /// Compressed size in bytes.
    pub compressed_bytes: usize,
    /// BWT primary index required for decoding.
    pub bwt_primary_index: usize,
    /// Compression ratio estimate (Shannon entropy / 8).
    pub compression_ratio: f64,
}

/// Compress text data via BWT→MTF→RLE and build a DB record.
///
/// Returns `None` for empty input.
#[inline]
#[must_use]
pub fn text_compression_to_db_record(data: &[u8]) -> Option<TextCompressionDbRecord> {
    if data.is_empty() {
        return None;
    }
    let (bwt, primary_index) = bwt_encode(data);
    let mtf = mtf_encode(&bwt);
    let compressed = rle_encode(&mtf);
    let content_hash = fnv1a(&compressed);
    let ratio = compression_ratio(data);
    Some(TextCompressionDbRecord {
        content_hash,
        original_bytes: data.len(),
        compressed_bytes: compressed.len(),
        bwt_primary_index: primary_index,
        compression_ratio: ratio,
        compressed,
    })
}

// ── Bridge 2: TextCompression → Cache (compressed cache entry) ────────────

/// Compressed cache entry for ALICE-Cache.
///
/// Caches the BWT-compressed form of a text payload.  TTL is shortened
/// branchlessly for large payloads to limit cache memory pressure.
pub struct TextCompressionCacheEntry {
    /// FNV-1a hash of the compressed bytes (cache key).
    pub content_hash: u64,
    /// BWT-compressed payload.
    pub compressed: Vec<u8>,
    /// Original size in bytes.
    pub original_bytes: usize,
    /// Compressed size in bytes.
    pub compressed_bytes: usize,
    /// Cache TTL in seconds (branchless: smaller for large payloads).
    pub ttl_secs: u32,
}

/// Build a compressed cache entry for ALICE-Cache.
///
/// TTL is 7200 s for payloads ≤ 64 KB and 1800 s for larger ones (branchless).
#[inline]
#[must_use]
pub fn text_compression_to_cache_entry(data: &[u8]) -> TextCompressionCacheEntry {
    let (bwt, _) = bwt_encode(data);
    let compressed = rle_encode(&mtf_encode(&bwt));
    let content_hash = fnv1a(&compressed);

    // Branchless TTL: > 64 KB → 1800 s, else 7200 s.
    let large = (data.len() > 65_536) as u32;
    let ttl_secs = 7200 - large * 5400;

    TextCompressionCacheEntry {
        content_hash,
        original_bytes: data.len(),
        compressed_bytes: compressed.len(),
        ttl_secs,
        compressed,
    }
}

// ── Bridge 3: TextCompression → CDN (compressed delivery payload) ─────────

/// Compressed delivery payload for ALICE-CDN.
///
/// Packages BWT-compressed content for CDN edge serving alongside
/// metadata the edge needs to set correct `Content-Encoding` headers.
pub struct TextCompressionCdnPayload {
    /// FNV-1a hash of the compressed bytes (CDN asset fingerprint).
    pub content_hash: u64,
    /// Huffman-code count for the original data (proxy for symbol entropy).
    pub huffman_symbol_count: usize,
    /// Original content length in bytes (for `Content-Length` restoration).
    pub original_bytes: usize,
    /// Compressed payload bytes.
    pub compressed: Vec<u8>,
    /// Compressed size in bytes.
    pub compressed_bytes: usize,
    /// Compression ratio estimate (0.0–1.0).
    pub compression_ratio: f64,
}

/// Build a CDN delivery payload from raw text data.
///
/// Uses BWT→MTF→RLE for the payload and separately computes the Huffman
/// symbol count as an entropy hint for the CDN routing layer.
#[inline]
#[must_use]
pub fn text_compression_to_cdn_payload(data: &[u8]) -> TextCompressionCdnPayload {
    let (bwt, _) = bwt_encode(data);
    let mtf = mtf_encode(&bwt);
    let compressed = rle_encode(&mtf);
    let content_hash = fnv1a(&compressed);
    let huffman_codes = build_huffman_codes(data);
    let ratio = compression_ratio(data);
    TextCompressionCdnPayload {
        content_hash,
        huffman_symbol_count: huffman_codes.len(),
        original_bytes: data.len(),
        compressed_bytes: compressed.len(),
        compression_ratio: ratio,
        compressed,
    }
}

// ── Bridge 4: TextCompression → Zip (compression chaining) ───────────────

/// Compression chaining descriptor for ALICE-Zip.
///
/// Feeds BWT-preprocessed data into the ALICE-Zip pipeline so that
/// the Zip layer can apply LZ77 after BWT+MTF+RLE conditioning.
pub struct TextCompressionZipChain {
    /// FNV-1a hash of the BWT-conditioned bytes (pipeline correlation key).
    pub content_hash: u64,
    /// BWT primary index (required for ALICE-Zip to reconstruct original).
    pub bwt_primary_index: usize,
    /// BWT→MTF→RLE conditioned bytes ready for Zip LZ77 input.
    pub conditioned: Vec<u8>,
    /// Original byte count.
    pub original_bytes: usize,
    /// Conditioned byte count.
    pub conditioned_bytes: usize,
    /// Entropy ratio after BWT conditioning (lower = more Zip-friendly).
    pub entropy_ratio: f64,
}

/// Build a Zip-chaining descriptor by running BWT→MTF→RLE conditioning.
///
/// Returns `None` for empty input.
#[inline]
#[must_use]
pub fn text_compression_to_zip_chain(data: &[u8]) -> Option<TextCompressionZipChain> {
    if data.is_empty() {
        return None;
    }
    let (bwt, bwt_primary_index) = bwt_encode(data);
    let mtf = mtf_encode(&bwt);
    let conditioned = rle_encode(&mtf);
    let content_hash = fnv1a(&conditioned);
    let entropy_ratio = compression_ratio(&conditioned);
    Some(TextCompressionZipChain {
        content_hash,
        bwt_primary_index,
        original_bytes: data.len(),
        conditioned_bytes: conditioned.len(),
        entropy_ratio,
        conditioned,
    })
}

// ── Bridge 5: TextCompression → Codec (encoding pipeline) ─────────────────

/// Encoding pipeline descriptor for ALICE-Codec.
///
/// Exposes the full BWT→MTF→RLE→Huffman symbol table so the Codec layer
/// can embed compressed text streams inside media containers (e.g. subtitle
/// or metadata tracks) using the correct entropy coding parameters.
pub struct TextCompressionCodecDescriptor {
    /// FNV-1a hash of the final compressed stream (Codec asset key).
    pub content_hash: u64,
    /// Original input length in bytes.
    pub original_bytes: usize,
    /// BWT+MTF+RLE compressed stream.
    pub compressed: Vec<u8>,
    /// Compressed stream length in bytes.
    pub compressed_bytes: usize,
    /// Number of distinct symbols in the Huffman code table.
    pub huffman_symbol_count: usize,
    /// Compression ratio (Shannon entropy / 8 for the original data).
    pub compression_ratio: f64,
    /// BWT primary index (stored in Codec container header for decoding).
    pub bwt_primary_index: usize,
}

/// Build a Codec pipeline descriptor from raw input data.
///
/// Computes the full BWT→MTF→RLE pipeline and derives the Huffman symbol
/// table from the original data so the Codec layer has all parameters
/// required to reconstruct the entropy-coding stage.
#[inline]
#[must_use]
pub fn text_compression_to_codec_descriptor(data: &[u8]) -> TextCompressionCodecDescriptor {
    let (bwt, bwt_primary_index) = bwt_encode(data);
    let mtf = mtf_encode(&bwt);
    let compressed = rle_encode(&mtf);
    let content_hash = fnv1a(&compressed);
    let huffman_codes = build_huffman_codes(data);
    let ratio = compression_ratio(data);
    TextCompressionCodecDescriptor {
        content_hash,
        original_bytes: data.len(),
        compressed_bytes: compressed.len(),
        huffman_symbol_count: huffman_codes.len(),
        compression_ratio: ratio,
        bwt_primary_index,
        compressed,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_text_compression::{bwt_decode, mtf_decode, rle_decode};

    const SAMPLE: &[u8] = b"the quick brown fox jumps over the lazy dog";

    #[test]
    fn test_db_record_basic() {
        let rec = text_compression_to_db_record(SAMPLE).unwrap();
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.original_bytes, SAMPLE.len());
        assert!(rec.compressed_bytes > 0);
        assert!(rec.compression_ratio > 0.0);
    }

    #[test]
    fn test_db_record_roundtrip() {
        let rec = text_compression_to_db_record(SAMPLE).unwrap();
        // Decode: RLE → MTF → BWT
        let mtf_dec = rle_decode(&rec.compressed);
        let bwt_dec = mtf_decode(&mtf_dec);
        let original = bwt_decode(&bwt_dec, rec.bwt_primary_index);
        assert_eq!(original, SAMPLE);
    }

    #[test]
    fn test_db_record_empty_returns_none() {
        assert!(text_compression_to_db_record(b"").is_none());
    }

    #[test]
    fn test_db_record_hash_deterministic() {
        let a = text_compression_to_db_record(SAMPLE).unwrap();
        let b = text_compression_to_db_record(SAMPLE).unwrap();
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_cache_entry_small_ttl() {
        let entry = text_compression_to_cache_entry(SAMPLE);
        // SAMPLE < 64 KB → 7200 s
        assert_eq!(entry.ttl_secs, 7200);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_cache_entry_large_ttl() {
        // > 64 KB → 1800 s (branchless)
        let large: Vec<u8> = (0u8..=255).cycle().take(70_000).collect();
        let entry = text_compression_to_cache_entry(&large);
        assert_eq!(entry.ttl_secs, 1800);
    }

    #[test]
    fn test_cdn_payload_basic() {
        let payload = text_compression_to_cdn_payload(SAMPLE);
        assert_ne!(payload.content_hash, 0);
        assert_eq!(payload.original_bytes, SAMPLE.len());
        assert!(payload.huffman_symbol_count > 0);
        assert!(payload.compression_ratio > 0.0);
    }

    #[test]
    fn test_zip_chain_basic() {
        let chain = text_compression_to_zip_chain(SAMPLE).unwrap();
        assert_ne!(chain.content_hash, 0);
        assert_eq!(chain.original_bytes, SAMPLE.len());
        assert!(chain.conditioned_bytes > 0);
    }

    #[test]
    fn test_zip_chain_empty_returns_none() {
        assert!(text_compression_to_zip_chain(b"").is_none());
    }

    #[test]
    fn test_codec_descriptor_basic() {
        let desc = text_compression_to_codec_descriptor(SAMPLE);
        assert_ne!(desc.content_hash, 0);
        assert_eq!(desc.original_bytes, SAMPLE.len());
        assert!(desc.huffman_symbol_count > 0);
        assert!(desc.compressed_bytes > 0);
    }

    #[test]
    fn test_codec_descriptor_different_inputs_differ() {
        let a = text_compression_to_codec_descriptor(b"hello world");
        let b = text_compression_to_codec_descriptor(b"goodbye world");
        assert_ne!(a.content_hash, b.content_hash);
    }
}
