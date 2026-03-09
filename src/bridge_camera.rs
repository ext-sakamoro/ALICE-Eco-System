//! Camera bridges — Camera ↔ DB, Cache, Analytics, ML, Edge
//!
//! 5 bridges connecting camera capture and ISP pipelines to the ALICE ecosystem.
//! Covers image storage, ISP frame caching, image quality metrics,
//! ML preprocessing descriptors, and edge sensor pipeline configuration.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Camera → DB (image storage) ───────────────────────────────

/// Image storage record for ALICE-DB.
///
/// One record per captured frame committed to persistent storage.
/// `content_hash` is derived from sensor ID, frame sequence, and resolution.
pub struct CameraDbRecord {
    /// FNV-1a hash of sensor_id + frame_seq + resolution — deduplication key.
    pub content_hash: u64,
    /// Unique sensor identifier.
    pub sensor_id: u32,
    /// Monotonic frame sequence number from the sensor.
    pub frame_seq: u64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Total pixel count (width * height).
    pub pixel_count: u64,
    /// Exposure value in units of 1/1000 EV (e.g. 0 = EV 0.000).
    pub exposure_ev_milli: i32,
    /// White balance colour temperature in Kelvin.
    pub white_balance_temp: u32,
    /// Capture timestamp in microseconds since epoch.
    pub capture_us: u64,
}

/// Build a `CameraDbRecord` from raw sensor capture parameters.
#[inline]
#[must_use]
pub fn camera_to_db_record(
    sensor_id: u32,
    frame_seq: u64,
    width: u32,
    height: u32,
    exposure_ev_milli: i32,
    white_balance_temp: u32,
    capture_us: u64,
) -> CameraDbRecord {
    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&sensor_id.to_le_bytes());
    buf[4..12].copy_from_slice(&frame_seq.to_le_bytes());
    buf[12..16].copy_from_slice(&width.to_le_bytes());
    buf[16..20].copy_from_slice(&height.to_le_bytes());
    let content_hash = fnv1a(&buf);
    let pixel_count = width as u64 * height as u64;
    CameraDbRecord {
        content_hash,
        sensor_id,
        frame_seq,
        width,
        height,
        pixel_count,
        exposure_ev_milli,
        white_balance_temp,
        capture_us,
    }
}

// ── Bridge 2: Camera → Cache (ISP frame cache) ──────────────────────────

/// ISP-processed frame cache entry for ALICE-Cache.
///
/// Caches the result of the ISP pipeline (demosaic, denoise, colour correction)
/// so downstream consumers can skip repeated processing of the same raw frame.
pub struct CameraIspCacheEntry {
    /// FNV-1a hash of sensor_id + frame_seq — cache lookup key.
    pub content_hash: u64,
    /// Sensor identifier.
    pub sensor_id: u32,
    /// Frame sequence number.
    pub frame_seq: u64,
    /// Processed frame size in bytes.
    pub frame_bytes: u64,
    /// ISP pipeline stages applied (bitmask).
    pub isp_stage_mask: u32,
    /// Cache TTL in seconds — longer for low-motion scenes.
    pub ttl_secs: u32,
    /// Focus score at time of capture (0 = blurred, 65535 = sharp).
    pub focus_score: u16,
}

/// Build a `CameraIspCacheEntry` for a processed frame.
///
/// TTL is 120 s when `focus_score >= 32768` (sharp frame, high reuse) and
/// 30 s otherwise (branchless).
#[inline]
#[must_use]
pub fn camera_to_isp_cache_entry(
    sensor_id: u32,
    frame_seq: u64,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
    isp_stage_mask: u32,
    focus_score: u16,
) -> CameraIspCacheEntry {
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&sensor_id.to_le_bytes());
    buf[4..12].copy_from_slice(&frame_seq.to_le_bytes());
    let content_hash = fnv1a(&buf);
    let frame_bytes = width as u64 * height as u64 * bytes_per_pixel as u64;
    // Branchless TTL: sharp (focus_score >= 32768) → 120s, blurred → 30s.
    let is_sharp = (focus_score >= 32768) as u32;
    let ttl_secs = 30 + is_sharp * 90;
    CameraIspCacheEntry {
        content_hash,
        sensor_id,
        frame_seq,
        frame_bytes,
        isp_stage_mask,
        ttl_secs,
        focus_score,
    }
}

// ── Bridge 3: Camera → Analytics (image quality metrics) ────────────────

/// Image quality analytics event for ALICE-Analytics.
///
/// Emitted per frame to track sensor health and image pipeline quality.
pub struct CameraAnalyticsQuality {
    /// FNV-1a hash of sensor_id + frame_seq — deduplication key.
    pub content_hash: u64,
    /// Sensor identifier.
    pub sensor_id: u32,
    /// Frame sequence number.
    pub frame_seq: u64,
    /// Focus score (0–65535).
    pub focus_score: u16,
    /// Exposure value in milli-EV.
    pub exposure_ev_milli: i32,
    /// White balance temperature in Kelvin.
    pub white_balance_temp: u32,
    /// Mean luminance of the frame (0–255).
    pub mean_luminance: u8,
    /// Noise estimate in milli-dB SNR (0 = worst).
    pub snr_milli_db: u32,
    /// Whether auto-exposure triggered on this frame.
    pub ae_triggered: bool,
}

/// Build a `CameraAnalyticsQuality` event from ISP output.
#[inline]
#[must_use]
pub fn camera_to_analytics_quality(
    sensor_id: u32,
    frame_seq: u64,
    focus_score: u16,
    exposure_ev_milli: i32,
    white_balance_temp: u32,
    mean_luminance: u8,
    snr_milli_db: u32,
    ae_triggered: bool,
) -> CameraAnalyticsQuality {
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&sensor_id.to_le_bytes());
    buf[4..12].copy_from_slice(&frame_seq.to_le_bytes());
    let content_hash = fnv1a(&buf);
    CameraAnalyticsQuality {
        content_hash,
        sensor_id,
        frame_seq,
        focus_score,
        exposure_ev_milli,
        white_balance_temp,
        mean_luminance,
        snr_milli_db,
        ae_triggered,
    }
}

// ── Bridge 4: Camera → ML (preprocessing descriptor) ────────────────────

/// ML preprocessing descriptor for ALICE-ML.
///
/// Describes how a captured frame should be normalised before being fed
/// into an inference pipeline (object detection, segmentation, etc.).
pub struct CameraMlPreprocess {
    /// FNV-1a hash of sensor_id + frame_seq + target_size — identity key.
    pub content_hash: u64,
    /// Source frame width in pixels.
    pub src_width: u32,
    /// Source frame height in pixels.
    pub src_height: u32,
    /// Target inference width in pixels.
    pub target_width: u32,
    /// Target inference height in pixels.
    pub target_height: u32,
    /// Channel count (1 = greyscale, 3 = RGB, 4 = RGBA).
    pub channels: u8,
    /// Normalisation mean per channel (fixed-point: actual = value / 1000).
    pub norm_mean_milli: [i32; 3],
    /// Normalisation std-dev per channel (fixed-point: actual = value / 1000).
    pub norm_std_milli: [u32; 3],
    /// Whether to apply horizontal flip augmentation.
    pub apply_hflip: bool,
}

/// Build a `CameraMlPreprocess` descriptor for ML pipeline ingestion.
#[inline]
#[must_use]
pub fn camera_to_ml_preprocess(
    sensor_id: u32,
    frame_seq: u64,
    src_width: u32,
    src_height: u32,
    target_width: u32,
    target_height: u32,
    channels: u8,
    norm_mean_milli: [i32; 3],
    norm_std_milli: [u32; 3],
    apply_hflip: bool,
) -> CameraMlPreprocess {
    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&sensor_id.to_le_bytes());
    buf[4..12].copy_from_slice(&frame_seq.to_le_bytes());
    buf[12..16].copy_from_slice(&target_width.to_le_bytes());
    buf[16..20].copy_from_slice(&target_height.to_le_bytes());
    let content_hash = fnv1a(&buf);
    CameraMlPreprocess {
        content_hash,
        src_width,
        src_height,
        target_width,
        target_height,
        channels,
        norm_mean_milli,
        norm_std_milli,
        apply_hflip,
    }
}

// ── Bridge 5: Camera → Edge (sensor pipeline config) ────────────────────

/// Edge sensor pipeline configuration for ALICE-Edge.
///
/// Instructs the edge node how to configure its capture → ISP → encode
/// pipeline for a given sensor, including transport and QoS settings.
pub struct CameraEdgePipeline {
    /// FNV-1a hash of sensor_id + pipeline_version — config identity key.
    pub content_hash: u64,
    /// Sensor identifier.
    pub sensor_id: u32,
    /// Pipeline configuration version (bump to invalidate cached configs).
    pub pipeline_version: u32,
    /// Target capture frame rate in milli-Hz (e.g. 30000 = 30 fps).
    pub frame_rate_milli_hz: u32,
    /// Requested output bitrate in kilobits per second.
    pub output_bitrate_kbps: u32,
    /// ISP pipeline stage bitmask to enable.
    pub isp_stage_mask: u32,
    /// Maximum allowed capture latency in microseconds.
    pub max_latency_us: u32,
    /// Transport protocol identifier (0 = RTP/UDP, 1 = RTSP, 2 = WebRTC).
    pub transport_proto: u8,
    /// Whether hardware encoder acceleration is required.
    pub require_hw_encode: bool,
}

/// Build a `CameraEdgePipeline` configuration descriptor.
#[inline]
#[must_use]
pub fn camera_to_edge_pipeline(
    sensor_id: u32,
    pipeline_version: u32,
    frame_rate_milli_hz: u32,
    output_bitrate_kbps: u32,
    isp_stage_mask: u32,
    max_latency_us: u32,
    transport_proto: u8,
    require_hw_encode: bool,
) -> CameraEdgePipeline {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&sensor_id.to_le_bytes());
    buf[4..8].copy_from_slice(&pipeline_version.to_le_bytes());
    let content_hash = fnv1a(&buf);
    CameraEdgePipeline {
        content_hash,
        sensor_id,
        pipeline_version,
        frame_rate_milli_hz,
        output_bitrate_kbps,
        isp_stage_mask,
        max_latency_us,
        transport_proto,
        require_hw_encode,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_db_record_hash_nonzero() {
        let rec = camera_to_db_record(1, 0, 1920, 1080, 0, 5600, 0);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_camera_db_record_pixel_count() {
        let rec = camera_to_db_record(1, 0, 1920, 1080, 0, 5600, 0);
        assert_eq!(rec.pixel_count, 1920 * 1080);
    }

    #[test]
    fn test_camera_db_record_deterministic() {
        let a = camera_to_db_record(2, 10, 640, 480, -500, 4200, 1_000_000);
        let b = camera_to_db_record(2, 10, 640, 480, -500, 4200, 1_000_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_camera_isp_cache_sharp_ttl() {
        let entry = camera_to_isp_cache_entry(1, 0, 1920, 1080, 3, 0b111, 40000);
        assert_eq!(entry.ttl_secs, 120);
    }

    #[test]
    fn test_camera_isp_cache_blurred_ttl() {
        let entry = camera_to_isp_cache_entry(1, 0, 1920, 1080, 3, 0b111, 1000);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_camera_analytics_quality_fields() {
        let q = camera_to_analytics_quality(3, 7, 50000, 200, 6500, 128, 42000, true);
        assert_ne!(q.content_hash, 0);
        assert_eq!(q.sensor_id, 3);
        assert!(q.ae_triggered);
        assert_eq!(q.mean_luminance, 128);
    }

    #[test]
    fn test_camera_ml_preprocess_fields() {
        let p = camera_to_ml_preprocess(
            1,
            5,
            1920,
            1080,
            640,
            640,
            3,
            [485, 456, 406],
            [229, 224, 225],
            false,
        );
        assert_ne!(p.content_hash, 0);
        assert_eq!(p.target_width, 640);
        assert_eq!(p.channels, 3);
        assert!(!p.apply_hflip);
    }

    #[test]
    fn test_camera_edge_pipeline_fields() {
        let cfg = camera_to_edge_pipeline(1, 2, 30_000, 4000, 0b1111, 33_333, 0, true);
        assert_ne!(cfg.content_hash, 0);
        assert_eq!(cfg.pipeline_version, 2);
        assert!(cfg.require_hw_encode);
        assert_eq!(cfg.transport_proto, 0);
    }
}
