//! Image bridges — ALICE-Image ↔ DB, Cache, CDN, Analytics, Edge
//!
//! 5 bridges connecting SIMD-accelerated image processing to the ALICE ecosystem.

use alice_image::{otsu_threshold, resize_bilinear, resize_lanczos3, rgb_to_lab, Image};
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

// ── Bridge 1: Image → DB (image metadata record) ─────────────────────────

/// Image metadata record for ALICE-DB persistence.
///
/// Captures dimension, channel, and perceptual hash information so the
/// database layer can index and deduplicate stored images without holding
/// the full pixel buffer.
pub struct ImageDbRecord {
    /// FNV-1a perceptual hash derived from downsampled pixel data.
    pub content_hash: u64,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Total pixel count (width × height).
    pub pixel_count: u32,
    /// Otsu binarisation threshold computed from the red channel histogram.
    pub otsu_threshold: u8,
    /// Mean red channel value (0–255).
    pub mean_r: u8,
    /// Mean green channel value (0–255).
    pub mean_g: u8,
    /// Mean blue channel value (0–255).
    pub mean_b: u8,
}

/// Build an image metadata record for ALICE-DB from an `Image`.
///
/// Downsamples to 8×8 via nearest-neighbour to produce a stable perceptual
/// hash, then computes per-channel means and an Otsu threshold for the
/// red channel.
#[inline]
#[must_use]
pub fn image_to_db_record(img: &Image) -> ImageDbRecord {
    // 8×8 downsample for perceptual hashing.
    let thumb = resize_bilinear(img, 8, 8);
    let thumb_bytes: Vec<u8> = thumb.pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
    let content_hash = fnv1a(&thumb_bytes);

    // Per-channel means.
    let n = img.pixels.len().max(1);
    let sum_r: u32 = img.pixels.iter().map(|p| p.r as u32).sum();
    let sum_g: u32 = img.pixels.iter().map(|p| p.g as u32).sum();
    let sum_b: u32 = img.pixels.iter().map(|p| p.b as u32).sum();
    let mean_r = (sum_r / n as u32) as u8;
    let mean_g = (sum_g / n as u32) as u8;
    let mean_b = (sum_b / n as u32) as u8;

    // Otsu threshold over red channel.
    let red_data: Vec<u8> = img.pixels.iter().map(|p| p.r).collect();
    let threshold = otsu_threshold(&red_data);

    ImageDbRecord {
        content_hash,
        width: img.width,
        height: img.height,
        pixel_count: img.width * img.height,
        otsu_threshold: threshold,
        mean_r,
        mean_g,
        mean_b,
    }
}

// ── Bridge 2: Image → Cache (image cache entry) ───────────────────────────

/// Image cache entry for ALICE-Cache.
///
/// Stores a compressed thumbnail and metadata for fast cache retrieval.
/// TTL is computed branchlessly: large images get a shorter TTL because
/// they consume more cache memory.
pub struct ImageCacheEntry {
    /// FNV-1a hash of the thumbnail pixel data (cache key).
    pub content_hash: u64,
    /// Thumbnail width in pixels.
    pub thumb_width: u32,
    /// Thumbnail height in pixels.
    pub thumb_height: u32,
    /// Serialised thumbnail pixels (RGBA bytes, thumb_width × thumb_height × 4).
    pub thumb_bytes: Vec<u8>,
    /// Cache TTL in seconds (branchless: shorter for large images).
    pub ttl_secs: u32,
    /// Original image size in pixels (width × height).
    pub original_pixel_count: u32,
}

/// Build an image cache entry from an `Image`.
///
/// Produces a 16×16 Lanczos-3 thumbnail for the cache payload.
/// TTL is 3600 s for images ≤ 1 MP and 1800 s for larger images (branchless).
#[inline]
#[must_use]
pub fn image_to_cache_entry(img: &Image) -> ImageCacheEntry {
    let thumb = resize_lanczos3(img, 16, 16);
    let thumb_bytes: Vec<u8> =
        thumb.pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
    let content_hash = fnv1a(&thumb_bytes);

    // Branchless TTL: large images (> 1 MP) get 1800 s, others 3600 s.
    let large = (img.width * img.height > 1_000_000) as u32;
    let ttl_secs = 3600 - large * 1800;

    ImageCacheEntry {
        content_hash,
        thumb_width: 16,
        thumb_height: 16,
        thumb_bytes,
        ttl_secs,
        original_pixel_count: img.width * img.height,
    }
}

// ── Bridge 3: Image → CDN (image delivery descriptor) ────────────────────

/// Image delivery descriptor for ALICE-CDN.
///
/// Describes the image asset so the CDN layer can select the appropriate
/// origin, set cache-control headers, and apply edge transforms.
pub struct ImageCdnDescriptor {
    /// FNV-1a hash of the 32×32 thumbnail (CDN asset fingerprint).
    pub content_hash: u64,
    /// Original image width in pixels.
    pub width: u32,
    /// Original image height in pixels.
    pub height: u32,
    /// Estimated uncompressed byte size (width × height × 4).
    pub estimated_bytes: usize,
    /// Dominant hue in CIE-L*a*b* `a*` channel (perceptual colour hint).
    pub lab_a_mean: i8,
    /// Dominant hue in CIE-L*a*b* `b*` channel.
    pub lab_b_mean: i8,
    /// Suggested CDN cache TTL in seconds.
    pub cdn_ttl_secs: u32,
}

/// Build a CDN delivery descriptor from an `Image`.
///
/// Downsamples to 32×32 for fingerprinting and computes mean Lab values
/// from the centre 4×4 region for colour-aware CDN routing.
#[inline]
#[must_use]
pub fn image_to_cdn_descriptor(img: &Image) -> ImageCdnDescriptor {
    let thumb = resize_bilinear(img, 32, 32);
    let thumb_bytes: Vec<u8> =
        thumb.pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
    let content_hash = fnv1a(&thumb_bytes);

    // Mean Lab from centre 4×4 of the 32×32 thumbnail.
    let mut sum_a = 0i32;
    let mut sum_b_lab = 0i32;
    let mut count = 0u32;
    for y in 14..18u32 {
        for x in 14..18u32 {
            let p = thumb.get_pixel(x, y);
            let lab = rgb_to_lab(p.r, p.g, p.b);
            sum_a += lab.a as i32;
            sum_b_lab += lab.b as i32;
            count += 1;
        }
    }
    let count_safe = count.max(1) as i32;
    let lab_a_mean = ((sum_a / count_safe).clamp(-128, 127)) as i8;
    let lab_b_mean = ((sum_b_lab / count_safe).clamp(-128, 127)) as i8;

    ImageCdnDescriptor {
        content_hash,
        width: img.width,
        height: img.height,
        estimated_bytes: (img.width * img.height) as usize * 4,
        lab_a_mean,
        lab_b_mean,
        cdn_ttl_secs: 86_400,
    }
}

// ── Bridge 4: Image → Analytics (image processing metrics) ───────────────

/// Image processing metrics for ALICE-Analytics.
///
/// Records perceptual statistics useful for tracking image quality,
/// format distribution, and processing load across the pipeline.
pub struct ImageAnalyticsMetrics {
    /// FNV-1a hash of downsampled image (analytics stream key).
    pub content_hash: u64,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Megapixel count (width × height / 1 000 000), scaled × 100 for integer representation.
    pub megapixels_x100: u32,
    /// Otsu binarisation threshold (image contrast proxy, 0 = flat, 255 = extreme contrast).
    pub contrast_threshold: u8,
    /// Mean luminance (0–255) computed from red-channel approximation.
    pub mean_luminance: u8,
    /// Blur sigma used if Gaussian pre-processing was applied (0 = no blur).
    pub blur_sigma_x10: u8,
}

/// Extract image processing metrics for ALICE-Analytics.
///
/// `blur_sigma` is the Gaussian blur sigma applied before analytics
/// (pass `0.0` if no blur was applied).  The value is stored as
/// `(sigma * 10).round()` truncated to u8.
#[inline]
#[must_use]
pub fn image_to_analytics_metrics(img: &Image, blur_sigma: f64) -> ImageAnalyticsMetrics {
    let thumb = resize_bilinear(img, 8, 8);
    let thumb_bytes: Vec<u8> =
        thumb.pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
    let content_hash = fnv1a(&thumb_bytes);

    let n = img.pixels.len().max(1);
    let sum_r: u32 = img.pixels.iter().map(|p| p.r as u32).sum();
    let mean_luminance = (sum_r / n as u32) as u8;

    let red_data: Vec<u8> = img.pixels.iter().map(|p| p.r).collect();
    let contrast_threshold = otsu_threshold(&red_data);

    let mp = img.width * img.height;
    let megapixels_x100 = mp / 10_000; // (w*h / 1_000_000) * 100 = w*h / 10_000

    let blur_sigma_x10 = ((blur_sigma * 10.0).round() as u32).min(255) as u8;

    ImageAnalyticsMetrics {
        content_hash,
        width: img.width,
        height: img.height,
        megapixels_x100,
        contrast_threshold,
        mean_luminance,
        blur_sigma_x10,
    }
}

// ── Bridge 5: Image → Edge (image processing events) ─────────────────────

/// Image processing event for ALICE-Edge.
///
/// Emitted whenever an image passes through the processing pipeline so that
/// edge nodes can react to visual content (e.g. triggering blur filters for
/// detected sensitive imagery, or routing high-resolution assets to GPU nodes).
pub struct ImageEdgeEvent {
    /// FNV-1a hash of the 8×8 thumbnail (event correlation key).
    pub content_hash: u64,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Event kind: 0=ingest, 1=transform, 2=evict, 3=error.
    pub event_kind: u8,
    /// Recommended edge processing flag: 0=none, 1=apply_blur, 2=gpu_upscale.
    pub processing_hint: u8,
    /// Priority score for edge queue scheduling (higher = process first).
    pub priority: u8,
}

/// Build an image processing event for ALICE-Edge.
///
/// `event_kind`: 0=ingest, 1=transform, 2=evict, 3=error.
/// `processing_hint` is derived branchlessly: high-resolution images
/// (> 4 MP) receive `gpu_upscale` hint (2), others receive `none` (0).
#[inline]
#[must_use]
pub fn image_to_edge_event(img: &Image, event_kind: u8) -> ImageEdgeEvent {
    let thumb = resize_bilinear(img, 8, 8);
    let thumb_bytes: Vec<u8> =
        thumb.pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
    let content_hash = fnv1a(&thumb_bytes);

    // Branchless processing hint: > 4 MP → gpu_upscale (2), else none (0).
    let high_res = (img.width * img.height > 4_000_000) as u8;
    let processing_hint = high_res * 2;

    // Priority: large images get higher priority (capped at 200).
    let mp = (img.width * img.height / 10_000).min(200) as u8;
    let priority = mp;

    ImageEdgeEvent {
        content_hash,
        width: img.width,
        height: img.height,
        event_kind: event_kind.min(3),
        processing_hint,
        priority,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_image(w: u32, h: u32) -> Image {
        let mut img = Image::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let v = ((x + y) % 256) as u8;
                img.set_pixel(x, y, Rgba::rgb(v, (v / 2).wrapping_add(30), v.wrapping_sub(20)));
            }
        }
        img
    }

    #[test]
    fn test_image_to_db_record_basic() {
        let img = make_image(64, 64);
        let rec = image_to_db_record(&img);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.width, 64);
        assert_eq!(rec.height, 64);
        assert_eq!(rec.pixel_count, 64 * 64);
    }

    #[test]
    fn test_image_to_db_record_hash_deterministic() {
        let img = make_image(32, 32);
        let a = image_to_db_record(&img);
        let b = image_to_db_record(&img);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_image_to_cache_entry_small_ttl() {
        // < 1 MP → 3600 s
        let img = make_image(100, 100);
        let entry = image_to_cache_entry(&img);
        assert_eq!(entry.ttl_secs, 3600);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.thumb_width, 16);
        assert_eq!(entry.thumb_height, 16);
        assert_eq!(entry.thumb_bytes.len(), 16 * 16 * 4);
    }

    #[test]
    fn test_image_to_cache_entry_large_ttl() {
        // > 1 MP → 1800 s (branchless TTL)
        let img = make_image(1024, 1024);
        let entry = image_to_cache_entry(&img);
        assert_eq!(entry.ttl_secs, 1800);
    }

    #[test]
    fn test_image_to_cdn_descriptor_basic() {
        let img = make_image(64, 64);
        let desc = image_to_cdn_descriptor(&img);
        assert_ne!(desc.content_hash, 0);
        assert_eq!(desc.width, 64);
        assert_eq!(desc.height, 64);
        assert_eq!(desc.estimated_bytes, 64 * 64 * 4);
        assert_eq!(desc.cdn_ttl_secs, 86_400);
    }

    #[test]
    fn test_image_to_cdn_descriptor_hash_deterministic() {
        let img = make_image(48, 48);
        let a = image_to_cdn_descriptor(&img);
        let b = image_to_cdn_descriptor(&img);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_image_to_analytics_metrics_no_blur() {
        let img = make_image(64, 64);
        let m = image_to_analytics_metrics(&img, 0.0);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.width, 64);
        assert_eq!(m.height, 64);
        assert_eq!(m.blur_sigma_x10, 0);
    }

    #[test]
    fn test_image_to_analytics_metrics_blur_sigma() {
        let img = make_image(64, 64);
        let m = image_to_analytics_metrics(&img, 2.5);
        assert_eq!(m.blur_sigma_x10, 25);
    }

    #[test]
    fn test_image_to_edge_event_ingest() {
        let img = make_image(64, 64);
        let ev = image_to_edge_event(&img, 0);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.event_kind, 0);
        // 64×64 = 4096 pixels < 4 MP → no gpu hint.
        assert_eq!(ev.processing_hint, 0);
    }

    #[test]
    fn test_image_to_edge_event_high_res_hint() {
        // 2048×2048 = 4 194 304 > 4 MP → gpu_upscale hint.
        let img = make_image(2048, 2048);
        let ev = image_to_edge_event(&img, 1);
        assert_eq!(ev.processing_hint, 2);
        assert_eq!(ev.event_kind, 1);
    }

    #[test]
    fn test_image_to_edge_event_kind_clamped() {
        let img = make_image(8, 8);
        let ev = image_to_edge_event(&img, 99);
        assert_eq!(ev.event_kind, 3);
    }
}
