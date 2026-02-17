//! Zip bridges — ALICE-Zip ↔ Edge, DB, Crypto
//!
//! 3 bridges connecting procedural compression to the ALICE ecosystem.

use alice_core::generators;
use alice_core::compression;

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
pub fn zip_edge_fit_sensor(samples: &[f32], max_degree: usize, error_threshold: f64) -> Option<ZipEdgeSensorData> {
    let data: Vec<f32> = samples.to_vec();
    let (coeffs, degree, error) = generators::fit_polynomial(&data, max_degree, error_threshold)?;
    let model_bytes = coeffs.len() * 8;
    let raw_bytes = samples.len() * 4;
    Some(ZipEdgeSensorData {
        coefficients: coeffs,
        degree,
        fit_error: error,
        sample_count: samples.len(),
        compression_ratio: if model_bytes > 0 { raw_bytes as f32 / model_bytes as f32 } else { 0.0 },
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
pub fn zip_db_compress_residual(residual: &[f32]) -> ZipDbResidual {
    let compressed = compression::compress_residual_quantized(residual, 8, 3)
        .unwrap_or_else(|_| Vec::new());
    let raw_bytes = residual.len() * 4;
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &compressed {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    ZipDbResidual {
        original_bytes: raw_bytes,
        compressed_bytes: compressed.len(),
        compression_ratio: if compressed.is_empty() { 0.0 } else { raw_bytes as f32 / compressed.len() as f32 },
        content_hash: hash,
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
pub fn zip_to_crypto_payload(residual: &[f32]) -> ZipCryptoPayload {
    let compressed = compression::compress_residual_quantized(residual, 8, 3)
        .unwrap_or_else(|_| Vec::new());
    let raw_bytes = residual.len() * 4;
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &compressed {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    ZipCryptoPayload {
        content_hash: hash,
        compressed_bytes: compressed.len(),
        original_bytes: raw_bytes,
        compression_ratio: if compressed.is_empty() { 0.0 } else { raw_bytes as f32 / compressed.len() as f32 },
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_edge_fit_sensor() {
        // Linear sensor data: y = 2x + 1
        let samples: Vec<f32> = (0..100).map(|i| 2.0 * i as f32 + 1.0).collect();
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
}
