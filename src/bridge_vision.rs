//! Vision bridges — ALICE-Vision ↔ DB, Cache, Analytics, ML, Render
//!
//! 5 bridges connecting computer vision processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Vision → DB (frame record) ─────────────────────────────────

/// Frame record for ALICE-DB persistence.
pub struct VisionDbRecord {
    /// Content hash over the frame snapshot.
    pub content_hash: u64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Number of colour channels.
    pub channels: u8,
    /// Total number of frames in the sequence.
    pub frame_count: u64,
    /// Hash of the detection model used.
    pub model_hash: u64,
    /// Number of objects detected in the frame.
    pub detection_count: u32,
}

/// Serialize a vision frame for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn vision_to_db_record(
    width: u32,
    height: u32,
    channels: u8,
    frame_count: u64,
    model_hash: u64,
    detection_count: u32,
) -> VisionDbRecord {
    let mut buf = [0u8; 29];
    buf[0..4].copy_from_slice(&width.to_le_bytes());
    buf[4..8].copy_from_slice(&height.to_le_bytes());
    buf[8] = channels;
    buf[9..17].copy_from_slice(&frame_count.to_le_bytes());
    buf[17..25].copy_from_slice(&model_hash.to_le_bytes());
    buf[25..29].copy_from_slice(&detection_count.to_le_bytes());
    VisionDbRecord {
        content_hash: fnv1a(&buf),
        width,
        height,
        channels,
        frame_count,
        model_hash,
        detection_count,
    }
}

// ── Bridge 2: Vision → Cache (decoded frame cache) ────────────────────────

/// Decoded frame cache entry for ALICE-Cache.
pub struct VisionCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Whether this is an I-frame (keyframe).
    pub is_keyframe: bool,
    /// Raw byte size of the cached frame.
    pub frame_bytes: u64,
}

/// Build a decoded frame cache entry for ALICE-Cache.
///
/// Keyframes receive a longer TTL (300 s vs 60 s) because they are
/// required for decoding subsequent P-frames and B-frames.
#[inline]
#[must_use]
pub fn vision_to_cache_entry(
    width: u32,
    height: u32,
    is_keyframe: bool,
    frame_bytes: u64,
) -> VisionCacheEntry {
    let mut buf = [0u8; 17];
    buf[0..4].copy_from_slice(&width.to_le_bytes());
    buf[4..8].copy_from_slice(&height.to_le_bytes());
    buf[8] = is_keyframe as u8;
    buf[9..17].copy_from_slice(&frame_bytes.to_le_bytes());
    let key_flag = is_keyframe as u32;
    let ttl_secs = 60 + key_flag * 240;
    VisionCacheEntry {
        content_hash: fnv1a(&buf),
        width,
        height,
        ttl_secs,
        is_keyframe,
        frame_bytes,
    }
}

// ── Bridge 3: Vision → Analytics (detection event) ───────────────────────

/// Detection analytics event for ALICE-Analytics ingestion.
pub struct VisionAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of objects detected.
    pub detection_count: u32,
    /// Inference latency in microseconds.
    pub inference_time_us: u64,
    /// Detection accuracy in basis points (0–10 000).
    pub accuracy_bps: u16,
    /// Frames per second at the time of the event.
    pub fps: u32,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a detection analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn vision_to_analytics_event(
    detection_count: u32,
    inference_time_us: u64,
    accuracy_bps: u16,
    fps: u32,
    timestamp_ms: u64,
) -> VisionAnalyticsEvent {
    let mut buf = [0u8; 30];
    buf[0..4].copy_from_slice(&detection_count.to_le_bytes());
    buf[4..12].copy_from_slice(&inference_time_us.to_le_bytes());
    buf[12..14].copy_from_slice(&accuracy_bps.to_le_bytes());
    buf[14..18].copy_from_slice(&fps.to_le_bytes());
    buf[18..26].copy_from_slice(&timestamp_ms.to_le_bytes());
    VisionAnalyticsEvent {
        content_hash: fnv1a(&buf),
        detection_count,
        inference_time_us,
        accuracy_bps,
        fps,
        timestamp_ms,
    }
}

// ── Bridge 4: Vision → ML (feature descriptor) ───────────────────────────

/// Feature descriptor for ALICE-ML downstream tasks.
pub struct VisionMlFeatures {
    /// Content hash over the feature snapshot.
    pub content_hash: u64,
    /// Dimensionality of the feature vector.
    pub feature_dim: u32,
    /// Number of bounding boxes in the feature map.
    pub bbox_count: u32,
    /// Average detection confidence in basis points (0–10 000).
    pub confidence_avg_bps: u16,
    /// Model version that extracted the features.
    pub model_version: u32,
}

/// Extract feature descriptors for ALICE-ML downstream tasks.
#[inline]
#[must_use]
pub fn vision_to_ml_features(
    feature_dim: u32,
    bbox_count: u32,
    confidence_avg_bps: u16,
    model_version: u32,
) -> VisionMlFeatures {
    let mut buf = [0u8; 14];
    buf[0..4].copy_from_slice(&feature_dim.to_le_bytes());
    buf[4..8].copy_from_slice(&bbox_count.to_le_bytes());
    buf[8..10].copy_from_slice(&confidence_avg_bps.to_le_bytes());
    buf[10..14].copy_from_slice(&model_version.to_le_bytes());
    VisionMlFeatures {
        content_hash: fnv1a(&buf),
        feature_dim,
        bbox_count,
        confidence_avg_bps,
        model_version,
    }
}

// ── Bridge 5: Vision → Render (overlay output) ───────────────────────────

/// Rendered overlay output for ALICE-Render.
pub struct VisionRenderOutput {
    /// Content hash over the render payload.
    pub content_hash: u64,
    /// Output frame width in pixels.
    pub width: u32,
    /// Output frame height in pixels.
    pub height: u32,
    /// Number of annotation overlays drawn.
    pub overlay_count: u32,
    /// Render latency in microseconds.
    pub render_time_us: u64,
    /// Byte size of the rendered frame.
    pub frame_bytes: u64,
}

/// Build a rendered overlay output for ALICE-Render.
#[inline]
#[must_use]
pub fn vision_to_render_output(
    width: u32,
    height: u32,
    overlay_count: u32,
    render_time_us: u64,
    frame_bytes: u64,
) -> VisionRenderOutput {
    let mut buf = [0u8; 28];
    buf[0..4].copy_from_slice(&width.to_le_bytes());
    buf[4..8].copy_from_slice(&height.to_le_bytes());
    buf[8..12].copy_from_slice(&overlay_count.to_le_bytes());
    buf[12..20].copy_from_slice(&render_time_us.to_le_bytes());
    buf[20..28].copy_from_slice(&frame_bytes.to_le_bytes());
    VisionRenderOutput {
        content_hash: fnv1a(&buf),
        width,
        height,
        overlay_count,
        render_time_us,
        frame_bytes,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_db_record_hash_nonzero() {
        let rec = vision_to_db_record(1920, 1080, 3, 300, 0xdead_beef, 5);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_vision_db_record_fields() {
        let rec = vision_to_db_record(640, 480, 1, 60, 0x1234_5678, 2);
        assert_eq!(rec.width, 640);
        assert_eq!(rec.height, 480);
        assert_eq!(rec.channels, 1);
        assert_eq!(rec.detection_count, 2);
    }

    #[test]
    fn test_vision_db_record_determinism() {
        let a = vision_to_db_record(1280, 720, 3, 120, 0xaaaa, 10);
        let b = vision_to_db_record(1280, 720, 3, 120, 0xaaaa, 10);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_vision_cache_entry_non_keyframe_ttl() {
        let entry = vision_to_cache_entry(1280, 720, false, 2_764_800);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 60);
        assert!(!entry.is_keyframe);
    }

    #[test]
    fn test_vision_cache_entry_keyframe_ttl() {
        let entry = vision_to_cache_entry(1920, 1080, true, 6_220_800);
        assert_eq!(entry.ttl_secs, 300);
        assert!(entry.is_keyframe);
    }

    #[test]
    fn test_vision_analytics_event() {
        let ev = vision_to_analytics_event(12, 8_500, 9_200, 30, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.detection_count, 12);
        assert_eq!(ev.fps, 30);
    }

    #[test]
    fn test_vision_ml_features() {
        let f = vision_to_ml_features(2_048, 20, 8_750, 5);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.feature_dim, 2_048);
        assert_eq!(f.confidence_avg_bps, 8_750);
    }

    #[test]
    fn test_vision_render_output() {
        let out = vision_to_render_output(1920, 1080, 8, 16_500, 6_220_800);
        assert_ne!(out.content_hash, 0);
        assert_eq!(out.overlay_count, 8);
        assert_eq!(out.frame_bytes, 6_220_800);
    }

    #[test]
    fn test_vision_render_output_determinism() {
        let a = vision_to_render_output(3840, 2160, 3, 50_000, 24_883_200);
        let b = vision_to_render_output(3840, 2160, 3, 50_000, 24_883_200);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
