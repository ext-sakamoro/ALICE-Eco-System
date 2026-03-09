//! GAN bridges — ALICE-GAN ↔ DB, Cache, Analytics, ML, API
//!
//! 5 bridges connecting generative adversarial network training to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: GAN → DB (training log) ───────────────────────────────────

/// Training log record for ALICE-DB persistence.
pub struct GanDbRecord {
    /// Content hash over the training snapshot.
    pub content_hash: u64,
    /// Current training epoch.
    pub epoch: u64,
    /// Generator loss at this epoch.
    pub generator_loss: f32,
    /// Discriminator loss at this epoch.
    pub discriminator_loss: f32,
    /// Dimensionality of the latent space.
    pub latent_dim: u32,
    /// Batch size used during training.
    pub batch_size: u32,
}

/// Serialize a GAN training snapshot for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn gan_to_db_record(
    epoch: u64,
    generator_loss: f32,
    discriminator_loss: f32,
    latent_dim: u32,
    batch_size: u32,
) -> GanDbRecord {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&epoch.to_le_bytes());
    buf[8..12].copy_from_slice(&generator_loss.to_bits().to_le_bytes());
    buf[12..16].copy_from_slice(&discriminator_loss.to_bits().to_le_bytes());
    buf[16..20].copy_from_slice(&latent_dim.to_le_bytes());
    buf[20..24].copy_from_slice(&batch_size.to_le_bytes());
    GanDbRecord {
        content_hash: fnv1a(&buf),
        epoch,
        generator_loss,
        discriminator_loss,
        latent_dim,
        batch_size,
    }
}

// ── Bridge 2: GAN → Cache (checkpoint cache) ────────────────────────────

/// Checkpoint cache entry for ALICE-Cache.
pub struct GanCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Epoch of the cached checkpoint.
    pub epoch: u64,
    /// Generator loss at the checkpoint.
    pub generator_loss: f32,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Whether this checkpoint is marked as the best so far.
    pub is_best: bool,
}

/// Build a checkpoint cache entry for ALICE-Cache.
///
/// Best checkpoints receive a longer TTL (3600 s vs 600 s) because they
/// are more likely to be retrieved for evaluation or resume.
#[inline]
#[must_use]
pub fn gan_to_cache_entry(epoch: u64, generator_loss: f32, is_best: bool) -> GanCacheEntry {
    let mut buf = [0u8; 13];
    buf[0..8].copy_from_slice(&epoch.to_le_bytes());
    buf[8..12].copy_from_slice(&generator_loss.to_bits().to_le_bytes());
    buf[12] = is_best as u8;
    let best_flag = is_best as u32;
    let ttl_secs = 600 + best_flag * 3000;
    GanCacheEntry {
        content_hash: fnv1a(&buf),
        epoch,
        generator_loss,
        ttl_secs,
        is_best,
    }
}

// ── Bridge 3: GAN → Analytics (training metrics) ────────────────────────

/// Training metrics for ALICE-Analytics ingestion.
pub struct GanAnalyticsMetrics {
    /// Content hash over the metric tuple.
    pub content_hash: u64,
    /// Current training epoch.
    pub epoch: u64,
    /// Generator loss at this epoch.
    pub generator_loss: f32,
    /// Discriminator loss at this epoch.
    pub discriminator_loss: f32,
    /// Frechet Inception Distance score.
    pub fid_score: f32,
    /// Batch size used during training.
    pub batch_size: u32,
}

/// Build training metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn gan_to_analytics_metrics(
    epoch: u64,
    generator_loss: f32,
    discriminator_loss: f32,
    fid_score: f32,
    batch_size: u32,
) -> GanAnalyticsMetrics {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&epoch.to_le_bytes());
    buf[8..12].copy_from_slice(&generator_loss.to_bits().to_le_bytes());
    buf[12..16].copy_from_slice(&discriminator_loss.to_bits().to_le_bytes());
    buf[16..20].copy_from_slice(&fid_score.to_bits().to_le_bytes());
    buf[20..24].copy_from_slice(&batch_size.to_le_bytes());
    GanAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        epoch,
        generator_loss,
        discriminator_loss,
        fid_score,
        batch_size,
    }
}

// ── Bridge 4: GAN → ML (discriminator features) ─────────────────────────

/// Discriminator feature vector for ALICE-ML downstream tasks.
pub struct GanMlFeatures {
    /// Content hash over the feature snapshot.
    pub content_hash: u64,
    /// Epoch at which the features were extracted.
    pub epoch: u64,
    /// Discriminator loss.
    pub discriminator_loss: f32,
    /// Latent dimensionality.
    pub latent_dim: u32,
    /// FID score (lower is better).
    pub fid_score: f32,
    /// Number of discriminator layers.
    pub layer_count: u32,
}

/// Extract discriminator features for ALICE-ML downstream tasks.
#[inline]
#[must_use]
pub fn gan_to_ml_features(
    epoch: u64,
    discriminator_loss: f32,
    latent_dim: u32,
    fid_score: f32,
    layer_count: u32,
) -> GanMlFeatures {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&epoch.to_le_bytes());
    buf[8..12].copy_from_slice(&discriminator_loss.to_bits().to_le_bytes());
    buf[12..16].copy_from_slice(&latent_dim.to_le_bytes());
    buf[16..20].copy_from_slice(&fid_score.to_bits().to_le_bytes());
    buf[20..24].copy_from_slice(&layer_count.to_le_bytes());
    GanMlFeatures {
        content_hash: fnv1a(&buf),
        epoch,
        discriminator_loss,
        latent_dim,
        fid_score,
        layer_count,
    }
}

// ── Bridge 5: GAN → API (generation service) ────────────────────────────

/// Generation service response for ALICE-API.
pub struct GanApiResponse {
    /// Content hash over the response payload.
    pub content_hash: u64,
    /// Epoch of the model used for generation.
    pub epoch: u64,
    /// Latent dimensionality.
    pub latent_dim: u32,
    /// Number of samples generated.
    pub sample_count: u32,
    /// Generation latency in microseconds.
    pub latency_us: u64,
    /// HTTP status code.
    pub status_code: u16,
}

/// Build a generation service response for ALICE-API.
#[inline]
#[must_use]
pub fn gan_to_api_response(
    epoch: u64,
    latent_dim: u32,
    sample_count: u32,
    latency_us: u64,
) -> GanApiResponse {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&epoch.to_le_bytes());
    buf[8..12].copy_from_slice(&latent_dim.to_le_bytes());
    buf[12..16].copy_from_slice(&sample_count.to_le_bytes());
    buf[16..24].copy_from_slice(&latency_us.to_le_bytes());
    let status_code = if sample_count > 0 { 200 } else { 204 };
    GanApiResponse {
        content_hash: fnv1a(&buf),
        epoch,
        latent_dim,
        sample_count,
        latency_us,
        status_code,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gan_to_db_record_hash_nonzero() {
        let rec = gan_to_db_record(10, 0.5, 0.4, 128, 64);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_gan_to_db_record_fields() {
        let rec = gan_to_db_record(5, 0.8, 0.6, 64, 32);
        assert_eq!(rec.epoch, 5);
        assert_eq!(rec.latent_dim, 64);
        assert_eq!(rec.batch_size, 32);
        assert!((rec.generator_loss - 0.8).abs() < 1e-6);
        assert!((rec.discriminator_loss - 0.6).abs() < 1e-6);
    }

    #[test]
    fn test_gan_to_cache_entry_normal_ttl() {
        let entry = gan_to_cache_entry(1, 0.9, false);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 600);
        assert!(!entry.is_best);
    }

    #[test]
    fn test_gan_to_cache_entry_best_ttl() {
        let entry = gan_to_cache_entry(20, 0.3, true);
        assert_eq!(entry.ttl_secs, 3600);
        assert!(entry.is_best);
    }

    #[test]
    fn test_gan_to_analytics_metrics() {
        let m = gan_to_analytics_metrics(100, 0.4, 0.45, 12.5, 64);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.epoch, 100);
        assert_eq!(m.batch_size, 64);
        assert!((m.fid_score - 12.5).abs() < 1e-5);
    }

    #[test]
    fn test_gan_to_ml_features() {
        let f = gan_to_ml_features(50, 0.5, 128, 15.0, 6);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.latent_dim, 128);
        assert_eq!(f.layer_count, 6);
    }

    #[test]
    fn test_gan_to_api_response_ok() {
        let resp = gan_to_api_response(30, 128, 4, 80_000);
        assert_ne!(resp.content_hash, 0);
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.sample_count, 4);
    }

    #[test]
    fn test_gan_to_api_response_no_samples() {
        let resp = gan_to_api_response(30, 128, 0, 0);
        assert_eq!(resp.status_code, 204);
    }
}
