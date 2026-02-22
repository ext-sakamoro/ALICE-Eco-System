//! Codec bridges — ALICE-Codec ↔ Synth, Animation, SDF, View, DB, Analytics
//!
//! 6 bridges connecting 3D wavelet video/audio codec to the ALICE ecosystem.

use alice_codec::rans::FrequencyTable;
use alice_codec::{SubBand3D, Wavelet1D};

// ── Bridge 1: Codec → Synth (wavelet → audio compression) ──────────────

/// Wavelet-compressed audio payload from ALICE-Synth PCM.
pub struct CodecSynthPayload {
    /// Original sample count.
    pub original_samples: usize,
    /// Compressed size estimate in bytes.
    pub compressed_bytes: usize,
    /// Compression ratio vs raw PCM.
    pub compression_ratio: f32,
}

/// Compress Synth PCM via ALICE-Codec wavelet transform.
#[inline]
#[must_use]
pub fn codec_compress_synth_pcm(pcm: &[f32]) -> CodecSynthPayload {
    if pcm.is_empty() {
        return CodecSynthPayload {
            original_samples: 0,
            compressed_bytes: 0,
            compression_ratio: 0.0,
        };
    }
    // Quantize f32 to i32 and apply wavelet
    let n = pcm.len().next_power_of_two();
    let mut signal: Vec<i32> = pcm.iter().map(|&x| (x * 32767.0) as i32).collect();
    signal.resize(n, 0);
    let wavelet = Wavelet1D::cdf97();
    wavelet.forward(&mut signal);
    let raw_bytes = pcm.len() * 4;
    // Estimate: wavelet coefficients quantized to 16-bit
    let compressed_bytes = n * 2;
    CodecSynthPayload {
        original_samples: pcm.len(),
        compressed_bytes,
        compression_ratio: if compressed_bytes > 0 {
            raw_bytes as f32 / compressed_bytes as f32
        } else {
            0.0
        },
    }
}

// ── Bridge 2: Codec → Animation (wavelet3D → episode compression) ───────

/// Compressed anime episode frame for ALICE-Animation.
pub struct CodecAnimFrame {
    /// Number of 3D wavelet sub-bands.
    pub subband_count: usize,
    /// DC sub-band size.
    pub dc_size: usize,
    /// Temporal high-frequency sub-band count.
    pub temporal_high_count: usize,
    /// Recommended quantization strengths per sub-band.
    pub quant_strengths: Vec<u8>,
}

/// Analyze sub-band structure for anime episode compression.
#[inline]
#[must_use]
pub fn codec_animation_frame_analysis(
    width: usize,
    height: usize,
    frames: usize,
) -> CodecAnimFrame {
    let subbands = [
        SubBand3D::LLL,
        SubBand3D::LLH,
        SubBand3D::LHL,
        SubBand3D::LHH,
        SubBand3D::HLL,
        SubBand3D::HLH,
        SubBand3D::HHL,
        SubBand3D::HHH,
    ];
    let temporal_high_count = subbands.iter().filter(|s| s.is_temporal_high()).count();
    let quant_strengths: Vec<u8> = subbands
        .iter()
        .map(alice_codec::SubBand3D::quant_strength)
        .collect();
    let dc_size = (width / 2) * (height / 2) * (frames / 2);
    CodecAnimFrame {
        subband_count: 8,
        dc_size,
        temporal_high_count,
        quant_strengths,
    }
}

// ── Bridge 3: Codec → SDF (rANS → compressed SDF volume) ───────────────

/// rANS-compressed SDF volume data.
pub struct CodecSdfVolume {
    /// Compressed data bytes.
    pub compressed_bytes: usize,
    /// Original voxel count.
    pub voxel_count: usize,
    /// Bits per voxel after compression.
    pub bits_per_voxel: f32,
    /// Compression ratio.
    pub compression_ratio: f32,
}

/// Compress quantized SDF distance field via rANS entropy coding.
#[inline]
#[must_use]
pub fn codec_compress_sdf_volume(quantized_distances: &[u8]) -> CodecSdfVolume {
    if quantized_distances.is_empty() {
        return CodecSdfVolume {
            compressed_bytes: 0,
            voxel_count: 0,
            bits_per_voxel: 0.0,
            compression_ratio: 0.0,
        };
    }
    // Build histogram
    let mut freq = [0u32; 256];
    for &b in quantized_distances {
        freq[b as usize] += 1;
    }
    // Use FrequencyTable from histogram
    let _table = FrequencyTable::from_histogram(&freq);
    // Estimate compressed size from entropy (reciprocal-hoisted)
    let total = quantized_distances.len() as f64;
    let rcp_total = 1.0 / total;
    let mut entropy_bits = 0.0f64;
    for &f in &freq {
        if f > 0 {
            let p = f as f64 * rcp_total;
            entropy_bits += -(p * p.log2()) * f as f64;
        }
    }
    let compressed_bytes = ((entropy_bits * 0.125) as usize).max(1);
    CodecSdfVolume {
        compressed_bytes,
        voxel_count: quantized_distances.len(),
        bits_per_voxel: if quantized_distances.is_empty() {
            0.0
        } else {
            (compressed_bytes * 8) as f32 / quantized_distances.len() as f32
        },
        compression_ratio: if compressed_bytes > 0 {
            quantized_distances.len() as f32 / compressed_bytes as f32
        } else {
            0.0
        },
    }
}

// ── Bridge 4: Codec → View (decoded frame → GPU display) ────────────────

/// Decoded frame ready for ALICE-View GPU rendering.
pub struct CodecViewFrame {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// RGB pixel data (width × height × 3).
    pub rgb_pixels: Vec<u8>,
    /// Frame number.
    pub frame_number: u32,
}

/// Reconstruct RGB frame from YCoCg-R wavelet decode for ALICE-View.
#[inline]
#[must_use]
pub fn codec_to_view_frame(
    y: &[i16],
    co: &[i16],
    cg: &[i16],
    width: usize,
    height: usize,
    frame_number: u32,
) -> CodecViewFrame {
    let n = width * height;
    let pixel_count = n.min(y.len()).min(co.len()).min(cg.len());
    let mut rgb = vec![0u8; n * 3];
    // chunks_exact_mut(3) eliminates per-pixel bounds checks (autovec-friendly)
    for (i, chunk) in rgb[..pixel_count * 3].chunks_exact_mut(3).enumerate() {
        let yv = y[i] as i32;
        let cov = co[i] as i32;
        let cgv = cg[i] as i32;
        let tmp = yv - cgv;
        chunk[0] = (tmp + cov).clamp(0, 255) as u8; // R
        chunk[1] = (yv + cgv).clamp(0, 255) as u8; // G
        chunk[2] = (tmp - cov).clamp(0, 255) as u8; // B
    }
    CodecViewFrame {
        width,
        height,
        rgb_pixels: rgb,
        frame_number,
    }
}

// ── Bridge 5: Codec → DB (compressed data persistence) ──────────────────

/// Compressed data record for ALICE-DB persistence.
pub struct CodecDbRecord {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Original voxel count.
    pub voxel_count: usize,
    /// Compressed bytes.
    pub compressed_bytes: usize,
    /// Bits per voxel.
    pub bits_per_voxel: f32,
}

/// Serialize compressed SDF volume metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn codec_to_db_record(quantized: &[u8]) -> CodecDbRecord {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in quantized {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let mut freq = [0u32; 256];
    for &b in quantized {
        freq[b as usize] += 1;
    }
    let total = quantized.len() as f64;
    let rcp_total = 1.0 / total;
    let mut entropy_bits = 0.0f64;
    for &f in &freq {
        if f > 0 {
            let p = f as f64 * rcp_total;
            entropy_bits += -(p * p.log2()) * f as f64;
        }
    }
    let compressed_bytes = ((entropy_bits * 0.125) as usize).max(1);
    CodecDbRecord {
        content_hash: hash,
        voxel_count: quantized.len(),
        compressed_bytes,
        bits_per_voxel: if quantized.is_empty() {
            0.0
        } else {
            (compressed_bytes * 8) as f32 / quantized.len() as f32
        },
    }
}

// ── Bridge 6: Codec → Analytics (compression metrics) ───────────────────

/// Codec compression metrics for ALICE-Analytics.
pub struct CodecAnalyticsMetrics {
    /// Original size in bytes.
    pub original_bytes: usize,
    /// Compressed size estimate.
    pub compressed_bytes: usize,
    /// Compression ratio.
    pub compression_ratio: f32,
    /// Shannon entropy (bits per symbol).
    pub entropy_bps: f32,
}

/// Extract compression metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn codec_to_analytics_metrics(data: &[u8]) -> CodecAnalyticsMetrics {
    if data.is_empty() {
        return CodecAnalyticsMetrics {
            original_bytes: 0,
            compressed_bytes: 0,
            compression_ratio: 0.0,
            entropy_bps: 0.0,
        };
    }
    let mut freq = [0u32; 256];
    for &b in data {
        freq[b as usize] += 1;
    }
    let total = data.len() as f64;
    let rcp_total = 1.0 / total;
    let mut entropy = 0.0f64;
    for &f in &freq {
        if f > 0 {
            let p = f as f64 * rcp_total;
            entropy -= p * p.log2();
        }
    }
    let compressed = ((entropy * total * 0.125) as usize).max(1);
    CodecAnalyticsMetrics {
        original_bytes: data.len(),
        compressed_bytes: compressed,
        compression_ratio: if compressed > 0 {
            data.len() as f32 / compressed as f32
        } else {
            0.0
        },
        entropy_bps: entropy as f32,
    }
}

// ── Bridge 7: Codec → CDN (codec output → CDN content package) ──────────

/// CDN content package derived from codec output for ALICE-CDN delivery.
pub struct CodecCdnPackage {
    /// Content hash for deduplication and CDN cache keying.
    pub content_hash: u64,
    /// Encoded payload size in bytes.
    pub encoded_bytes: usize,
    /// Codec identifier (0 = wavelet/rANS default).
    pub codec_id: u8,
    /// Estimated bitrate in kilobits per second (assumes 30 fps delivery).
    pub bitrate_kbps: u32,
    /// Whether the payload is suitable for HTTP streaming delivery (always true).
    pub is_streamable: bool,
}

/// Build a CDN content package from encoded codec output data.
///
/// `codec_id` is fixed at 0 (ALICE wavelet+rANS codec).
/// `bitrate_kbps` is estimated from `encoded_bytes` assuming 30 fps delivery.
/// `is_streamable` is always `true`: codec output is always streamable for CDN.
#[inline]
#[must_use]
pub fn codec_to_cdn_package(encoded: &[u8]) -> CodecCdnPackage {
    const RCP_1000: f64 = 1.0 / 1000.0;
    // FNV-1a hash for CDN cache keying (inline — no shared fnv1a in this file).
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in encoded {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    // bitrate_kbps: encoded_bytes * 8 bits / 1000 bits-per-kbit / (1/30 s per frame)
    // = encoded_bytes * 8 * 30 / 1000 = encoded_bytes * 240 / 1000.
    let bitrate_kbps = (encoded.len() as f64 * 240.0 * RCP_1000) as u32;
    CodecCdnPackage {
        content_hash: hash,
        encoded_bytes: encoded.len(),
        codec_id: 0,
        bitrate_kbps,
        is_streamable: true,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_compress_synth_pcm() {
        let pcm: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.01).sin()).collect();
        let result = codec_compress_synth_pcm(&pcm);
        assert!(result.original_samples == 1024);
        assert!(result.compression_ratio > 0.0);
    }

    #[test]
    fn test_codec_animation_frame_analysis() {
        let frame = codec_animation_frame_analysis(1920, 1080, 64);
        assert_eq!(frame.subband_count, 8);
        assert!(frame.temporal_high_count > 0);
        assert_eq!(frame.quant_strengths.len(), 8);
    }

    #[test]
    fn test_codec_compress_sdf_volume() {
        let data: Vec<u8> = (0..1000).map(|i| (i % 128) as u8).collect();
        let result = codec_compress_sdf_volume(&data);
        assert_eq!(result.voxel_count, 1000);
        assert!(result.compressed_bytes > 0);
    }

    #[test]
    fn test_codec_to_view_frame() {
        let y = vec![128i16; 4];
        let co = vec![0i16; 4];
        let cg = vec![0i16; 4];
        let frame = codec_to_view_frame(&y, &co, &cg, 2, 2, 0);
        assert_eq!(frame.rgb_pixels.len(), 12);
        assert_eq!(frame.width, 2);
    }

    #[test]
    fn test_codec_to_db_record() {
        let data: Vec<u8> = (0..500).map(|i| (i % 64) as u8).collect();
        let rec = codec_to_db_record(&data);
        assert_eq!(rec.voxel_count, 500);
        assert!(rec.compressed_bytes > 0);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_codec_to_analytics_metrics() {
        let data: Vec<u8> = (0..1000).map(|i| (i % 128) as u8).collect();
        let m = codec_to_analytics_metrics(&data);
        assert_eq!(m.original_bytes, 1000);
        assert!(m.compression_ratio > 0.0);
        assert!(m.entropy_bps > 0.0);
    }

    #[test]
    fn test_codec_to_cdn_package() {
        let encoded: Vec<u8> = (0..2000).map(|i| (i % 255) as u8).collect();
        let pkg = codec_to_cdn_package(&encoded);
        assert_eq!(pkg.encoded_bytes, 2000);
        assert_eq!(pkg.codec_id, 0);
        assert!(pkg.is_streamable);
        assert!(pkg.bitrate_kbps > 0);
        assert_ne!(pkg.content_hash, 0);
        // Deterministic: same data → same hash.
        let pkg2 = codec_to_cdn_package(&encoded);
        assert_eq!(pkg.content_hash, pkg2.content_hash);
    }

    #[test]
    fn test_codec_to_cdn_package_empty() {
        let pkg = codec_to_cdn_package(&[]);
        assert_eq!(pkg.encoded_bytes, 0);
        assert_eq!(pkg.bitrate_kbps, 0);
        assert!(pkg.is_streamable);
    }
}
