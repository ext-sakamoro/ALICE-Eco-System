//! Cross-domain bridges — ALICE-Image ↔ SDF, ML, Codec, Analytics, Cache
//!
//! 5 bridges connecting SIMD-accelerated image processing to SDF heightmap
//! generation, ML inference input, Codec frame encoding, analytics events,
//! and cache entries with branchless TTL.

use alice_image::Image;
#[cfg(test)]
use alice_image::Rgba;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Image buffer → SDF heightmap seed data ──────────────────

/// SDF heightmap seed data derived from an image buffer.
///
/// Converts an image's grayscale channel into heightmap metadata so the
/// SDF layer can construct a height field surface without holding the
/// full pixel buffer.
pub struct ImageSdfHeightmap {
    /// FNV-1a hash over width, height, min/max/mean grayscale bytes.
    pub content_hash: u64,
    /// Image width in pixels (heightmap grid X resolution).
    pub width: u32,
    /// Image height in pixels (heightmap grid Z resolution).
    pub height: u32,
    /// Minimum grayscale value (maps to SDF floor).
    pub min_gray: u8,
    /// Maximum grayscale value (maps to SDF ceiling).
    pub max_gray: u8,
    /// Mean grayscale value (SDF midpoint reference).
    pub mean_gray: u8,
    /// Grayscale range (max - min), used for SDF amplitude scaling.
    pub range: u8,
    /// Total pixel count.
    pub pixel_count: u32,
}

/// Convert an image buffer into SDF heightmap seed data.
#[inline]
#[must_use]
pub fn image_buffer_to_sdf_heightmap(img: &Image) -> ImageSdfHeightmap {
    let pixel_count = img.width * img.height;
    let mut min_g: u8 = 255;
    let mut max_g: u8 = 0;
    let mut sum: u64 = 0;

    for px in &img.pixels {
        let g = px.to_grayscale();
        if g < min_g {
            min_g = g;
        }
        if g > max_g {
            max_g = g;
        }
        sum += g as u64;
    }

    let mean_gray = if pixel_count > 0 {
        (sum / pixel_count as u64) as u8
    } else {
        0
    };
    let range = max_g.wrapping_sub(min_g);

    let mut key = [0u8; 13];
    key[0..4].copy_from_slice(&img.width.to_le_bytes());
    key[4..8].copy_from_slice(&img.height.to_le_bytes());
    key[8] = min_g;
    key[9] = max_g;
    key[10] = mean_gray;
    key[11] = range;
    key[12] = 0; // パディング

    ImageSdfHeightmap {
        content_hash: fnv1a(&key),
        width: img.width,
        height: img.height,
        min_gray: min_g,
        max_gray: max_g,
        mean_gray,
        range,
        pixel_count,
    }
}

// ── Bridge 2: Image buffer → ML inference input ───────────────────────

/// ML inference input derived from an image buffer.
///
/// Extracts channel statistics and a compact feature fingerprint from an
/// image so the ML layer can run classification or detection without
/// holding the full pixel array.
pub struct ImageMlInput {
    /// FNV-1a hash over width, height, channel means, channel variances.
    pub content_hash: u64,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Mean red channel value [0.0, 1.0].
    pub mean_r: f32,
    /// Mean green channel value [0.0, 1.0].
    pub mean_g: f32,
    /// Mean blue channel value [0.0, 1.0].
    pub mean_b: f32,
    /// Total pixel count (= input tensor spatial dimension).
    pub pixel_count: u32,
    /// Feature dimensionality: 3 (RGB means).
    pub feature_dim: usize,
}

/// Convert an image buffer into ML inference input.
#[inline]
#[must_use]
pub fn image_buffer_to_ml_input(img: &Image) -> ImageMlInput {
    let pixel_count = img.width * img.height;
    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;

    for px in &img.pixels {
        sum_r += px.r as u64;
        sum_g += px.g as u64;
        sum_b += px.b as u64;
    }

    let n = if pixel_count > 0 { pixel_count as f32 } else { 1.0 };
    let mean_r = sum_r as f32 / (n * 255.0);
    let mean_g = sum_g as f32 / (n * 255.0);
    let mean_b = sum_b as f32 / (n * 255.0);

    let mut key = [0u8; 20];
    key[0..4].copy_from_slice(&img.width.to_le_bytes());
    key[4..8].copy_from_slice(&img.height.to_le_bytes());
    key[8..12].copy_from_slice(&mean_r.to_bits().to_le_bytes());
    key[12..16].copy_from_slice(&mean_g.to_bits().to_le_bytes());
    key[16..20].copy_from_slice(&mean_b.to_bits().to_le_bytes());

    ImageMlInput {
        content_hash: fnv1a(&key),
        width: img.width,
        height: img.height,
        mean_r,
        mean_g,
        mean_b,
        pixel_count,
        feature_dim: 3,
    }
}

// ── Bridge 3: Image buffer → Codec frame data ────────────────────────

/// Codec frame data derived from an image buffer.
///
/// Extracts dimension and color space metadata so the Codec layer can
/// set up a `FrameEncoder` with the correct parameters.
pub struct ImageCodecFrame {
    /// FNV-1a hash over width, height, channel info bytes.
    pub content_hash: u64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Total pixel count.
    pub pixel_count: u32,
    /// Bytes per pixel (always 4 for RGBA).
    pub bytes_per_pixel: u8,
    /// Estimated raw frame size in bytes.
    pub raw_frame_bytes: usize,
    /// Whether the image is opaque (all alpha = 255).
    pub is_opaque: bool,
}

/// Convert an image buffer into Codec frame data.
#[inline]
#[must_use]
pub fn image_buffer_to_codec_frame(img: &Image) -> ImageCodecFrame {
    let pixel_count = img.width * img.height;
    let raw_frame_bytes = pixel_count as usize * 4;

    let mut is_opaque = true;
    for px in &img.pixels {
        if px.a != 255 {
            is_opaque = false;
            break;
        }
    }

    let mut key = [0u8; 10];
    key[0..4].copy_from_slice(&img.width.to_le_bytes());
    key[4..8].copy_from_slice(&img.height.to_le_bytes());
    key[8] = 4; // bytes_per_pixel
    key[9] = is_opaque as u8;

    ImageCodecFrame {
        content_hash: fnv1a(&key),
        width: img.width,
        height: img.height,
        pixel_count,
        bytes_per_pixel: 4,
        raw_frame_bytes,
        is_opaque,
    }
}

// ── Bridge 4: Image metadata → Analytics event ───────────────────────

/// Analytics event derived from image metadata.
///
/// Captures dimension, complexity (edge density approximation), and
/// color space statistics for analytics dashboards.
pub struct ImageAnalyticsEvent {
    /// FNV-1a hash over width, height, mean luminance, edge density bytes.
    pub content_hash: u64,
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// Mean luminance [0, 255].
    pub mean_luminance: u8,
    /// Estimated edge density (0.0–1.0): ratio of pixels with high gradient.
    pub edge_density: f32,
    /// Event kind discriminant: 0 = image_processed.
    pub event_kind: u8,
    /// Pixel count.
    pub pixel_count: u32,
}

/// Convert image metadata into an analytics event.
#[inline]
#[must_use]
pub fn image_metadata_to_analytics(img: &Image) -> ImageAnalyticsEvent {
    let pixel_count = img.width * img.height;
    let mut lum_sum: u64 = 0;
    let mut edge_count: u32 = 0;

    for y in 0..img.height {
        for x in 0..img.width {
            let px = img.get_pixel(x, y);
            let g = px.to_grayscale();
            lum_sum += g as u64;

            // 簡易エッジ検出: 右隣ピクセルとの差分
            if x + 1 < img.width {
                let right = img.get_pixel(x + 1, y).to_grayscale();
                let diff = if g > right { g - right } else { right - g };
                if diff > 30 {
                    edge_count += 1;
                }
            }
        }
    }

    let mean_luminance = if pixel_count > 0 {
        (lum_sum / pixel_count as u64) as u8
    } else {
        0
    };
    let edge_density = if pixel_count > 1 {
        edge_count as f32 / (pixel_count - img.height) as f32
    } else {
        0.0
    };

    let mut key = [0u8; 14];
    key[0..4].copy_from_slice(&img.width.to_le_bytes());
    key[4..8].copy_from_slice(&img.height.to_le_bytes());
    key[8] = mean_luminance;
    key[9] = 0; // event_kind = image_processed
    key[10..14].copy_from_slice(&edge_density.to_bits().to_le_bytes());

    ImageAnalyticsEvent {
        content_hash: fnv1a(&key),
        width: img.width,
        height: img.height,
        mean_luminance,
        edge_density,
        event_kind: 0,
        pixel_count,
    }
}

// ── Bridge 5: Image → Cache entry with branchless TTL ─────────────────

/// Cache entry for an image with branchless TTL computation.
///
/// Large images (> 1 Mpx) get a shorter TTL to conserve cache memory.
pub struct ImageCacheEntry {
    /// FNV-1a hash over width, height, pixel data fingerprint.
    pub content_hash: u64,
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// Pixel count.
    pub pixel_count: u32,
    /// TTL in seconds (branchless: base 3600 - large_flag * 2400).
    pub ttl_secs: u32,
    /// Estimated memory footprint in bytes.
    pub memory_bytes: usize,
}

/// Convert an image into a cache entry with branchless TTL.
#[inline]
#[must_use]
pub fn image_to_cache(img: &Image) -> ImageCacheEntry {
    let pixel_count = img.width * img.height;
    let memory_bytes = pixel_count as usize * 4;

    // Branchless TTL: 大画像 (>1Mpx) は短いTTL
    let is_large = (pixel_count > 1_000_000) as u32;
    let ttl_secs = 3600 - is_large * 2400;

    // 高速フィンガープリント: 先頭64ピクセルのハッシュ
    let sample_count = if img.pixels.len() > 64 { 64 } else { img.pixels.len() };
    let mut fp_key = [0u8; 264]; // 8 (w+h) + 64*4 = 264
    fp_key[0..4].copy_from_slice(&img.width.to_le_bytes());
    fp_key[4..8].copy_from_slice(&img.height.to_le_bytes());
    for i in 0..sample_count {
        let px = &img.pixels[i];
        let off = 8 + i * 4;
        fp_key[off] = px.r;
        fp_key[off + 1] = px.g;
        fp_key[off + 2] = px.b;
        fp_key[off + 3] = px.a;
    }
    let hash_len = 8 + sample_count * 4;

    ImageCacheEntry {
        content_hash: fnv1a(&fp_key[..hash_len]),
        width: img.width,
        height: img.height,
        pixel_count,
        ttl_secs,
        memory_bytes,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image() -> Image {
        let mut img = Image::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                let v = ((x + y * 4) * 16) as u8;
                img.set_pixel(x, y, Rgba::rgb(v, v, v));
            }
        }
        img
    }

    fn large_image() -> Image {
        // 1024x1024 = 1Mpx超
        Image::new(1024, 1024)
    }

    // ── Bridge 1: Image → SDF heightmap ─────────────────────────────

    #[test]
    fn test_image_to_sdf_heightmap() {
        let img = test_image();
        let hm = image_buffer_to_sdf_heightmap(&img);
        assert_ne!(hm.content_hash, 0);
        assert_eq!(hm.width, 4);
        assert_eq!(hm.height, 4);
        assert_eq!(hm.pixel_count, 16);
        assert!(hm.max_gray >= hm.min_gray);
        assert_eq!(hm.range, hm.max_gray - hm.min_gray);
    }

    #[test]
    fn test_image_to_sdf_heightmap_deterministic() {
        let img = test_image();
        let a = image_buffer_to_sdf_heightmap(&img);
        let b = image_buffer_to_sdf_heightmap(&img);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Bridge 2: Image → ML input ──────────────────────────────────

    #[test]
    fn test_image_to_ml_input() {
        let img = test_image();
        let ml = image_buffer_to_ml_input(&img);
        assert_ne!(ml.content_hash, 0);
        assert_eq!(ml.width, 4);
        assert_eq!(ml.height, 4);
        assert_eq!(ml.feature_dim, 3);
        // グレースケール画像なので R=G=B
        assert!((ml.mean_r - ml.mean_g).abs() < 0.01);
    }

    #[test]
    fn test_image_to_ml_input_deterministic() {
        let img = test_image();
        let a = image_buffer_to_ml_input(&img);
        let b = image_buffer_to_ml_input(&img);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Bridge 3: Image → Codec frame ───────────────────────────────

    #[test]
    fn test_image_to_codec_frame() {
        let img = test_image();
        let frame = image_buffer_to_codec_frame(&img);
        assert_ne!(frame.content_hash, 0);
        assert_eq!(frame.width, 4);
        assert_eq!(frame.height, 4);
        assert_eq!(frame.bytes_per_pixel, 4);
        assert_eq!(frame.raw_frame_bytes, 64); // 4*4*4
        assert!(frame.is_opaque); // Rgba::rgb sets alpha=255
    }

    // ── Bridge 4: Image → Analytics ─────────────────────────────────

    #[test]
    fn test_image_to_analytics() {
        let img = test_image();
        let evt = image_metadata_to_analytics(&img);
        assert_ne!(evt.content_hash, 0);
        assert_eq!(evt.width, 4);
        assert_eq!(evt.height, 4);
        assert_eq!(evt.event_kind, 0);
        assert_eq!(evt.pixel_count, 16);
    }

    // ── Bridge 5: Image → Cache ─────────────────────────────────────

    #[test]
    fn test_image_to_cache_small() {
        let img = test_image();
        let cache = image_to_cache(&img);
        assert_ne!(cache.content_hash, 0);
        assert_eq!(cache.ttl_secs, 3600); // 小画像 → 長いTTL
        assert_eq!(cache.memory_bytes, 64);
    }

    #[test]
    fn test_image_to_cache_large() {
        let img = large_image();
        let cache = image_to_cache(&img);
        assert_eq!(cache.pixel_count, 1024 * 1024);
        // 1Mpx超 → branchless TTL: 3600 - 1*2400 = 1200
        assert_eq!(cache.ttl_secs, 1200);
    }

    #[test]
    fn test_image_to_cache_deterministic() {
        let img = test_image();
        let a = image_to_cache(&img);
        let b = image_to_cache(&img);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
