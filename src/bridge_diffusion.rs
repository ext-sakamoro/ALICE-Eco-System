//! Diffusion bridges — ALICE-Diffusion ↔ DB, Cache, Analytics, ML, CDN
//!
//! 5 bridges connecting diffusion model generation to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Diffusion → DB (generation record) ─────────────────────────

/// Generation record for ALICE-DB persistence.
pub struct DiffusionDbRecord {
    /// Content hash over the generation parameters.
    pub content_hash: u64,
    /// Output image width in pixels.
    pub width: u32,
    /// Output image height in pixels.
    pub height: u32,
    /// Number of denoising steps.
    pub steps: u32,
    /// Hash of the diffusion model checkpoint.
    pub model_hash: u64,
    /// Random seed used for generation.
    pub seed: u64,
    /// Classifier-free guidance scale multiplied by 100 (e.g. 750 = 7.5).
    pub guidance_scale_x100: u32,
}

/// Serialize a diffusion generation record for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn diffusion_to_db_record(
    width: u32,
    height: u32,
    steps: u32,
    model_hash: u64,
    seed: u64,
    guidance_scale_x100: u32,
) -> DiffusionDbRecord {
    let mut buf = [0u8; 36];
    buf[0..4].copy_from_slice(&width.to_le_bytes());
    buf[4..8].copy_from_slice(&height.to_le_bytes());
    buf[8..12].copy_from_slice(&steps.to_le_bytes());
    buf[12..20].copy_from_slice(&model_hash.to_le_bytes());
    buf[20..28].copy_from_slice(&seed.to_le_bytes());
    buf[28..32].copy_from_slice(&guidance_scale_x100.to_le_bytes());
    DiffusionDbRecord {
        content_hash: fnv1a(&buf[..32]),
        width,
        height,
        steps,
        model_hash,
        seed,
        guidance_scale_x100,
    }
}

// ── Bridge 2: Diffusion → Cache (generated image cache) ──────────────────

/// Generated image cache entry for ALICE-Cache.
pub struct DiffusionCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of denoising steps.
    pub steps: u32,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Byte size of the cached image.
    pub image_bytes: u64,
    /// Model version that produced the image.
    pub model_version: u32,
}

/// Build a generated image cache entry for ALICE-Cache.
///
/// High-step generations receive a longer TTL (1800 s vs 300 s) because
/// they are expensive to reproduce and more likely to be reused.
#[inline]
#[must_use]
pub fn diffusion_to_cache_entry(
    steps: u32,
    image_bytes: u64,
    model_version: u32,
) -> DiffusionCacheEntry {
    let mut buf = [0u8; 16];
    buf[0..4].copy_from_slice(&steps.to_le_bytes());
    buf[4..12].copy_from_slice(&image_bytes.to_le_bytes());
    buf[12..16].copy_from_slice(&model_version.to_le_bytes());
    let high_quality = (steps >= 50) as u32;
    let ttl_secs = 300 + high_quality * 1500;
    DiffusionCacheEntry {
        content_hash: fnv1a(&buf),
        steps,
        ttl_secs,
        image_bytes,
        model_version,
    }
}

// ── Bridge 3: Diffusion → Analytics (generation event) ───────────────────

/// Generation analytics event for ALICE-Analytics ingestion.
pub struct DiffusionAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of denoising steps.
    pub steps: u32,
    /// Total generation time in milliseconds.
    pub gen_time_ms: u64,
    /// Number of images in the batch.
    pub batch_size: u32,
    /// VRAM consumed in megabytes.
    pub vram_mb: u32,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a generation analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn diffusion_to_analytics_event(
    steps: u32,
    gen_time_ms: u64,
    batch_size: u32,
    vram_mb: u32,
    timestamp_ms: u64,
) -> DiffusionAnalyticsEvent {
    let mut buf = [0u8; 28];
    buf[0..4].copy_from_slice(&steps.to_le_bytes());
    buf[4..12].copy_from_slice(&gen_time_ms.to_le_bytes());
    buf[12..16].copy_from_slice(&batch_size.to_le_bytes());
    buf[16..20].copy_from_slice(&vram_mb.to_le_bytes());
    buf[20..28].copy_from_slice(&timestamp_ms.to_le_bytes());
    DiffusionAnalyticsEvent {
        content_hash: fnv1a(&buf),
        steps,
        gen_time_ms,
        batch_size,
        vram_mb,
        timestamp_ms,
    }
}

// ── Bridge 4: Diffusion → ML (model configuration) ───────────────────────

/// Model configuration for ALICE-ML fine-tuning pipelines.
pub struct DiffusionMlConfig {
    /// Content hash over the configuration.
    pub content_hash: u64,
    /// Hash of the base diffusion model.
    pub model_hash: u64,
    /// Hash of the noise scheduler.
    pub scheduler_hash: u64,
    /// Number of inference steps.
    pub steps: u32,
    /// Classifier-free guidance scale multiplied by 100.
    pub guidance_scale_x100: u32,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
}

/// Build a model configuration for ALICE-ML fine-tuning pipelines.
#[inline]
#[must_use]
pub fn diffusion_to_ml_config(
    model_hash: u64,
    scheduler_hash: u64,
    steps: u32,
    guidance_scale_x100: u32,
    width: u32,
    height: u32,
) -> DiffusionMlConfig {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&model_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&scheduler_hash.to_le_bytes());
    buf[16..20].copy_from_slice(&steps.to_le_bytes());
    buf[20..24].copy_from_slice(&guidance_scale_x100.to_le_bytes());
    buf[24..28].copy_from_slice(&width.to_le_bytes());
    buf[28..32].copy_from_slice(&height.to_le_bytes());
    DiffusionMlConfig {
        content_hash: fnv1a(&buf),
        model_hash,
        scheduler_hash,
        steps,
        guidance_scale_x100,
        width,
        height,
    }
}

// ── Bridge 5: Diffusion → CDN (edge delivery descriptor) ─────────────────

/// Edge delivery descriptor for ALICE-CDN distribution.
pub struct DiffusionCdnDelivery {
    /// Content hash over the delivery payload.
    pub content_hash: u64,
    /// Byte size of the image to deliver.
    pub image_bytes: u64,
    /// Edge node TTL in seconds.
    pub edge_ttl_secs: u32,
    /// Hash of the origin storage location.
    pub origin_hash: u64,
    /// Whether this is a thumbnail variant.
    pub is_thumbnail: bool,
}

/// Build an edge delivery descriptor for ALICE-CDN.
///
/// Thumbnails receive a longer edge TTL (86 400 s vs 3 600 s) because
/// they are cheap to store and accessed far more frequently than originals.
#[inline]
#[must_use]
pub fn diffusion_to_cdn_delivery(
    image_bytes: u64,
    origin_hash: u64,
    is_thumbnail: bool,
) -> DiffusionCdnDelivery {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&image_bytes.to_le_bytes());
    buf[8..16].copy_from_slice(&origin_hash.to_le_bytes());
    buf[16] = is_thumbnail as u8;
    let thumb_flag = is_thumbnail as u32;
    let edge_ttl_secs = 3_600 + thumb_flag * 82_800;
    DiffusionCdnDelivery {
        content_hash: fnv1a(&buf),
        image_bytes,
        edge_ttl_secs,
        origin_hash,
        is_thumbnail,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diffusion_db_record_hash_nonzero() {
        let rec = diffusion_to_db_record(512, 512, 50, 0x5344_584c, 42, 750);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_diffusion_db_record_fields() {
        let rec = diffusion_to_db_record(1024, 1024, 30, 0x1234, 0, 700);
        assert_eq!(rec.width, 1024);
        assert_eq!(rec.height, 1024);
        assert_eq!(rec.steps, 30);
        assert_eq!(rec.guidance_scale_x100, 700);
    }

    #[test]
    fn test_diffusion_db_record_determinism() {
        let a = diffusion_to_db_record(512, 512, 20, 0xbeef, 99, 750);
        let b = diffusion_to_db_record(512, 512, 20, 0xbeef, 99, 750);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_diffusion_cache_entry_low_steps_ttl() {
        let entry = diffusion_to_cache_entry(20, 786_432, 2);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_diffusion_cache_entry_high_steps_ttl() {
        let entry = diffusion_to_cache_entry(100, 786_432, 2);
        assert_eq!(entry.ttl_secs, 1800);
        assert_eq!(entry.steps, 100);
    }

    #[test]
    fn test_diffusion_analytics_event() {
        let ev = diffusion_to_analytics_event(50, 12_000, 1, 8_192, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.vram_mb, 8_192);
        assert_eq!(ev.batch_size, 1);
    }

    #[test]
    fn test_diffusion_ml_config() {
        let cfg = diffusion_to_ml_config(0x5344_3135, 0x4444_494d, 50, 750, 512, 512);
        assert_ne!(cfg.content_hash, 0);
        assert_eq!(cfg.steps, 50);
        assert_eq!(cfg.width, 512);
    }

    #[test]
    fn test_diffusion_cdn_delivery_original_ttl() {
        let d = diffusion_to_cdn_delivery(786_432, 0x6f72_6967, false);
        assert_ne!(d.content_hash, 0);
        assert_eq!(d.edge_ttl_secs, 3_600);
        assert!(!d.is_thumbnail);
    }

    #[test]
    fn test_diffusion_cdn_delivery_thumbnail_ttl() {
        let d = diffusion_to_cdn_delivery(8_192, 0x6f72_6967, true);
        assert_eq!(d.edge_ttl_secs, 86_400);
        assert!(d.is_thumbnail);
    }
}
