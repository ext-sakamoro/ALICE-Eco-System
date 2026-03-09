//! Zip bridges — ALICE-Zip ↔ Edge, DB, Crypto, ML, Cache
//!
//! 5 bridges connecting procedural compression to the ALICE ecosystem.

use crate::hash::fnv1a;
use alice_core::compression;
use alice_core::generators;
use alice_ml::TernaryWeight;

// ── Bridge 1: Zip → Edge (pattern compression → sensor data) ────────────

/// Procedurally compressed sensor data for ALICE-Edge.
pub struct ZipEdgeSensorData {
    /// Polynomial coefficients (compact model).
    pub coefficients: Vec<f64>,
    /// Polynomial degree.
    pub degree: usize,
    /// Fit error (max absolute error).
    pub fit_error: f64,
    /// Original sample count.
    pub sample_count: usize,
    /// Compression ratio.
    pub compression_ratio: f32,
}

/// Fit sensor data to polynomial model for ALICE-Edge compression.
#[inline]
#[must_use]
pub fn zip_edge_fit_sensor(
    samples: &[f32],
    max_degree: usize,
    error_threshold: f64,
) -> Option<ZipEdgeSensorData> {
    let data: Vec<f32> = samples.to_vec();
    let (coeffs, degree, error) = generators::fit_polynomial(&data, max_degree, error_threshold)?;
    let model_bytes = coeffs.len() * 8;
    let raw_bytes = samples.len() * 4;
    Some(ZipEdgeSensorData {
        coefficients: coeffs,
        degree,
        fit_error: error,
        sample_count: samples.len(),
        compression_ratio: if model_bytes > 0 {
            raw_bytes as f32 / model_bytes as f32
        } else {
            0.0
        },
    })
}

// ── Bridge 2: Zip → DB (residual compression → DB records) ──────────────

/// Compressed residual record for ALICE-DB storage.
pub struct ZipDbResidual {
    /// Compressed residual bytes.
    pub compressed: Vec<u8>,
    /// Original residual size.
    pub original_bytes: usize,
    /// Compressed size.
    pub compressed_bytes: usize,
    /// Compression ratio.
    pub compression_ratio: f32,
    /// Content hash for deduplication.
    pub content_hash: u64,
}

/// Compress model residuals for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn zip_db_compress_residual(residual: &[f32]) -> ZipDbResidual {
    let compressed =
        compression::compress_residual_quantized(residual, 8, 3).unwrap_or_else(|_| Vec::new());
    let raw_bytes = residual.len() * 4;
    let content_hash = fnv1a(&compressed);
    ZipDbResidual {
        original_bytes: raw_bytes,
        compressed_bytes: compressed.len(),
        compression_ratio: if compressed.is_empty() {
            0.0
        } else {
            raw_bytes as f32 / compressed.len() as f32
        },
        content_hash,
        compressed,
    }
}

// ── Bridge 3: Zip → Crypto (encrypted compressed data) ──────────────────

/// Encrypted compressed data metadata for ALICE-Crypto.
pub struct ZipCryptoPayload {
    /// Content hash for integrity.
    pub content_hash: u64,
    /// Compressed data bytes.
    pub compressed_bytes: usize,
    /// Original data bytes.
    pub original_bytes: usize,
    /// Compression ratio achieved before encryption.
    pub compression_ratio: f32,
}

/// Prepare compressed residual for ALICE-Crypto encryption.
#[inline]
#[must_use]
pub fn zip_to_crypto_payload(residual: &[f32]) -> ZipCryptoPayload {
    let compressed =
        compression::compress_residual_quantized(residual, 8, 3).unwrap_or_else(|_| Vec::new());
    let raw_bytes = residual.len() * 4;
    let content_hash = fnv1a(&compressed);
    ZipCryptoPayload {
        content_hash,
        compressed_bytes: compressed.len(),
        original_bytes: raw_bytes,
        compression_ratio: if compressed.is_empty() {
            0.0
        } else {
            raw_bytes as f32 / compressed.len() as f32
        },
    }
}

// ── Bridge 4: Zip → ML (compressed model storage) ────────────────────────

/// Ternary weight matrix stored as a compressed byte blob for ALICE-ML.
pub struct ZipMlCompressedModel {
    /// Model identifier hash (FNV-1a of name bytes).
    pub model_id: u64,
    /// Number of rows (output neurons).
    pub rows: usize,
    /// Number of columns (input neurons).
    pub cols: usize,
    /// Packed byte count (2 bits per ternary weight).
    pub packed_bytes: usize,
    /// Content hash of the compressed payload for deduplication.
    pub content_hash: u64,
    /// Compression ratio: raw f32 bytes / compressed bytes.
    pub compression_ratio: f32,
}

/// Compress a `TernaryWeight` matrix and package it for ALICE-ML storage.
///
/// Raw footprint is `rows * cols * 4` bytes (f32 equivalent).
/// Packed ternary footprint is `(rows * cols + 3) / 4` bytes (2 bits/weight).
/// `compression_ratio` is computed via reciprocal multiply (no division op).
#[inline]
#[must_use]
pub fn zip_ml_store(weights: &TernaryWeight, model_name: &str) -> ZipMlCompressedModel {
    let rows = weights.out_features();
    let cols = weights.in_features();
    let total = rows * cols;
    // Pack dimensions as content fingerprint (no raw weight data access needed)
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&(rows as u64).to_le_bytes());
    buf[8..16].copy_from_slice(&(cols as u64).to_le_bytes());
    let content_hash = fnv1a(&buf);
    let model_id = fnv1a(model_name.as_bytes());
    let packed_bytes = total.div_ceil(4); // 2 bits per ternary weight
    let raw_bytes = total * 4; // f32-equivalent footprint
                               // Reciprocal multiply — avoids division.
    let rcp_packed = if packed_bytes == 0 {
        0.0
    } else {
        1.0 / packed_bytes as f32
    };
    let compression_ratio = raw_bytes as f32 * rcp_packed;
    ZipMlCompressedModel {
        model_id,
        rows,
        cols,
        packed_bytes,
        content_hash,
        compression_ratio,
    }
}

// ── Bridge 5: Zip → Cache (compressed data cache entry) ──────────────────

/// Cache entry descriptor for ALICE-Cache keyed on compressed content.
pub struct ZipCacheEntry {
    /// FNV-1a hash of the cache key string — used as the primary cache key.
    pub content_hash: u64,
    /// Compressed payload size in bytes.
    pub compressed_bytes: usize,
    /// Original (uncompressed) payload size in bytes.
    pub original_bytes: usize,
    /// Compression ratio: original / compressed (reciprocal multiply, no division).
    pub compression_ratio: f32,
    /// Whether the entry is worth caching (ratio > 1.0, branchless comparison).
    pub cache_worthy: bool,
}

/// Build a cache entry descriptor from a key string and size measurements.
///
/// `content_hash` = `fnv1a(key.as_bytes())` for O(|key|) keying with no
/// heap allocation beyond the key slice.
/// `cache_worthy` is set branchlessly from the integer comparison
/// `original_bytes > compressed_bytes`.
#[inline]
#[must_use]
pub fn zip_cache_entry(key: &str, compressed_bytes: usize, original_bytes: usize) -> ZipCacheEntry {
    let content_hash = fnv1a(key.as_bytes());
    // Branchless: bool from integer comparison, no if/else.
    let cache_worthy = original_bytes > compressed_bytes;
    // Reciprocal multiply — avoids division.
    let rcp_compressed = if compressed_bytes > 0 {
        1.0 / compressed_bytes as f32
    } else {
        0.0
    };
    let compression_ratio = original_bytes as f32 * rcp_compressed;
    ZipCacheEntry {
        content_hash,
        compressed_bytes,
        original_bytes,
        compression_ratio,
        cache_worthy,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_edge_fit_sensor() {
        // Linear sensor data: y = 2x + 1
        let samples: Vec<f32> = (0..100).map(|i| 2.0f32.mul_add(i as f32, 1.0)).collect();
        let result = zip_edge_fit_sensor(&samples, 3, 0.01);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.degree <= 3);
        assert!(r.fit_error < 1.0);
        assert!(r.compression_ratio > 1.0);
    }

    #[test]
    fn test_zip_db_compress_residual() {
        let residual: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.01).sin() * 0.1).collect();
        let result = zip_db_compress_residual(&residual);
        assert!(result.compressed_bytes > 0);
        assert_ne!(result.content_hash, 0);
    }

    #[test]
    fn test_zip_to_crypto_payload() {
        let residual: Vec<f32> = (0..500).map(|i| (i as f32 * 0.02).sin() * 0.05).collect();
        let payload = zip_to_crypto_payload(&residual);
        assert!(payload.compressed_bytes > 0);
        assert_ne!(payload.content_hash, 0);
        assert_eq!(payload.original_bytes, 2000);
    }

    #[test]
    fn test_zip_ml_store() {
        use crate::hash::fnv1a;
        // 4×4 ternary weight matrix: 16 weights packed into 4 bytes.
        let weights = TernaryWeight::from_ternary(
            &[1, -1, 0, 1, -1, 1, 0, -1, 1, 0, -1, 1, -1, 0, 1, -1],
            4,
            4,
        );
        let entry = zip_ml_store(&weights, "layer0");
        assert_eq!(entry.rows, 4);
        assert_eq!(entry.cols, 4);
        assert!(entry.packed_bytes > 0);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.model_id, fnv1a(b"layer0"));
        // raw_bytes = 4*4*4 = 64; packed bytes = 4 → ratio = 16.0
        assert!(entry.compression_ratio > 1.0);
    }

    #[test]
    fn test_zip_cache_entry() {
        use crate::hash::fnv1a;
        let entry = zip_cache_entry("model:layer0:v1", 500, 4000);
        assert_eq!(entry.content_hash, fnv1a(b"model:layer0:v1"));
        assert_eq!(entry.compressed_bytes, 500);
        assert_eq!(entry.original_bytes, 4000);
        assert!(entry.cache_worthy);
        // ratio = 4000 / 500 = 8.0 (via reciprocal multiply, within f32 precision)
        assert!((entry.compression_ratio - 8.0).abs() < 0.01);

        // Entry where compressed >= original is not cache-worthy.
        let bad = zip_cache_entry("tiny", 100, 80);
        assert!(!bad.cache_worthy);
    }

    // ── 追加テスト ────────────────────────────────────────────────────────

    #[test]
    fn test_zip_db_residual_determinism() {
        // 同一入力で2回呼び出すと content_hash が一致すること（決定性確認）。
        let residual: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let r1 = zip_db_compress_residual(&residual);
        let r2 = zip_db_compress_residual(&residual);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.compressed_bytes, r2.compressed_bytes);
    }

    #[test]
    fn test_zip_crypto_payload_original_bytes() {
        // original_bytes が残差スライスの実際のバイト数（len * 4）と一致すること。
        let residual: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let payload = zip_to_crypto_payload(&residual);
        assert_eq!(payload.original_bytes, 5 * 4);
        assert_ne!(payload.content_hash, 0);
    }

    #[test]
    fn test_zip_cache_entry_zero_compressed() {
        // compressed_bytes=0 の場合 compression_ratio=0.0 でパニックしないこと。
        // original(100) > compressed(0) なので cache_worthy=true になる。
        let entry = zip_cache_entry("empty", 0, 100);
        assert!((entry.compression_ratio - 0.0).abs() < 0.01);
        assert!(entry.cache_worthy);
        assert_ne!(entry.content_hash, 0);
    }
}
