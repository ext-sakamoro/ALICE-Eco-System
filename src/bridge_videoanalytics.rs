//! VideoAnalytics bridges — ALICE-VideoAnalytics ↔ DB, Cache, Analytics, Streaming, Notify
//!
//! 5 bridges connecting video analysis pipelines to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: VideoAnalytics → DB (detection event log) ──────────────────

/// Detection event record for ALICE-DB persistence.
pub struct VideoAnalyticsDbRecord {
    /// Content hash over stream + frame + timestamp.
    pub content_hash: u64,
    /// FNV-1a hash of the stream identifier.
    pub stream_id_hash: u64,
    /// Frame sequence number within the stream.
    pub frame_seq: u64,
    /// Frame width in pixels.
    pub frame_width: u16,
    /// Frame height in pixels.
    pub frame_height: u16,
    /// Number of objects detected in this frame.
    pub object_count: u16,
    /// Accumulated motion energy (sum of pixel delta magnitudes, fixed-point × 100).
    pub motion_energy_x100: u32,
    /// Event timestamp in nanoseconds.
    pub timestamp_ns: u64,
}

/// Serialize a detection event for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn videoanalytics_to_db_record(
    stream_id: &[u8],
    frame_seq: u64,
    frame_width: u16,
    frame_height: u16,
    object_count: u16,
    motion_energy_x100: u32,
    timestamp_ns: u64,
) -> VideoAnalyticsDbRecord {
    let stream_id_hash = fnv1a(stream_id);
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&stream_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&frame_seq.to_le_bytes());
    key[16..24].copy_from_slice(&timestamp_ns.to_le_bytes());
    VideoAnalyticsDbRecord {
        content_hash: fnv1a(&key),
        stream_id_hash,
        frame_seq,
        frame_width,
        frame_height,
        object_count,
        motion_energy_x100,
        timestamp_ns,
    }
}

// ── Bridge 2: VideoAnalytics → Cache (decoded frame cache) ───────────────

/// Decoded frame cache entry for ALICE-Cache.
pub struct VideoAnalyticsCacheEntry {
    /// Content hash over stream + frame sequence.
    pub content_hash: u64,
    /// FNV-1a hash of the stream identifier.
    pub stream_id_hash: u64,
    /// Frame sequence number.
    pub frame_seq: u64,
    /// Encoded frame size in bytes.
    pub frame_bytes: u32,
    /// Cache TTL in seconds (shortened for high-motion frames).
    pub ttl_secs: u32,
}

/// Build a frame cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 2 s for high-motion frames
/// (motion_energy_x100 > 50000) to ensure stale frames are evicted quickly.
#[inline]
#[must_use]
pub fn videoanalytics_to_cache_entry(
    stream_id: &[u8],
    frame_seq: u64,
    frame_bytes: u32,
    motion_energy_x100: u32,
) -> VideoAnalyticsCacheEntry {
    let stream_id_hash = fnv1a(stream_id);
    // Branchless TTL: 10 s normal, 2 s for high-motion frames.
    let high_motion = (motion_energy_x100 > 50_000) as u32;
    let ttl_secs = 10_u32 - high_motion * 8_u32;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&stream_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&frame_seq.to_le_bytes());
    VideoAnalyticsCacheEntry {
        content_hash: fnv1a(&key),
        stream_id_hash,
        frame_seq,
        frame_bytes,
        ttl_secs,
    }
}

// ── Bridge 3: VideoAnalytics → Analytics (detection metrics) ─────────────

/// Detection metrics for ALICE-Analytics ingestion.
pub struct VideoAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total frames analysed in the reporting window.
    pub frames_analysed: u64,
    /// Total object detections across all frames.
    pub total_detections: u64,
    /// Average objects per frame.
    pub avg_objects_per_frame: f32,
    /// Average motion energy per frame (fixed-point × 100).
    pub avg_motion_energy_x100: u32,
    /// Number of heatmap cells updated in the window.
    pub heatmap_cells_updated: u32,
    /// Window start timestamp in nanoseconds.
    pub window_start_ns: u64,
}

/// Build detection metrics for ALICE-Analytics ingestion.
///
/// Averages use reciprocal multiply against `frames_analysed`.
#[inline]
#[must_use]
pub fn videoanalytics_to_analytics_metrics(
    frames_analysed: u64,
    total_detections: u64,
    sum_motion_energy_x100: u64,
    heatmap_cells_updated: u32,
    window_start_ns: u64,
) -> VideoAnalyticsMetrics {
    let rcp = 1.0 / frames_analysed.max(1) as f64;
    let avg_objects_per_frame = (total_detections as f64 * rcp) as f32;
    let avg_motion_energy_x100 = (sum_motion_energy_x100 as f64 * rcp) as u32;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&frames_analysed.to_le_bytes());
    key[8..16].copy_from_slice(&window_start_ns.to_le_bytes());
    VideoAnalyticsMetrics {
        content_hash: fnv1a(&key),
        frames_analysed,
        total_detections,
        avg_objects_per_frame,
        avg_motion_energy_x100,
        heatmap_cells_updated,
        window_start_ns,
    }
}

// ── Bridge 4: VideoAnalytics → Streaming (annotated frame output) ─────────

/// Annotated frame payload for ALICE-Streaming output.
pub struct VideoAnalyticsStreamFrame {
    /// Content hash over stream + frame sequence.
    pub content_hash: u64,
    /// FNV-1a hash of the stream identifier.
    pub stream_id_hash: u64,
    /// Frame sequence number.
    pub frame_seq: u64,
    /// Frame width in pixels.
    pub frame_width: u16,
    /// Frame height in pixels.
    pub frame_height: u16,
    /// Number of annotation overlays attached.
    pub annotation_count: u16,
    /// Encoded annotation payload size in bytes.
    pub annotation_bytes: u32,
    /// Frame presentation timestamp in nanoseconds.
    pub pts_ns: u64,
}

/// Build an annotated frame payload for ALICE-Streaming.
#[inline]
#[must_use]
pub fn videoanalytics_to_stream_frame(
    stream_id: &[u8],
    frame_seq: u64,
    frame_width: u16,
    frame_height: u16,
    annotation_count: u16,
    annotation_bytes: u32,
    pts_ns: u64,
) -> VideoAnalyticsStreamFrame {
    let stream_id_hash = fnv1a(stream_id);
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&stream_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&frame_seq.to_le_bytes());
    VideoAnalyticsStreamFrame {
        content_hash: fnv1a(&key),
        stream_id_hash,
        frame_seq,
        frame_width,
        frame_height,
        annotation_count,
        annotation_bytes,
        pts_ns,
    }
}

// ── Bridge 5: VideoAnalytics → Notify (detection alerts) ─────────────────

/// Detection alert payload for ALICE-Notify.
pub struct VideoAnalyticsNotifyAlert {
    /// Content hash over stream + frame + alert type.
    pub content_hash: u64,
    /// FNV-1a hash of the stream identifier.
    pub stream_id_hash: u64,
    /// Frame sequence number where the alert was triggered.
    pub frame_seq: u64,
    /// Alert type: 0=intrusion, 1=motion_threshold, 2=object_class, 3=crowd_density.
    pub alert_type: u8,
    /// Severity: 0=info, 1=warning, 2=critical.
    pub severity: u8,
    /// Number of objects that triggered the alert.
    pub trigger_count: u16,
    /// Alert timestamp in nanoseconds.
    pub timestamp_ns: u64,
}

/// Build a detection alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn videoanalytics_to_notify_alert(
    stream_id: &[u8],
    frame_seq: u64,
    alert_type: u8,
    severity: u8,
    trigger_count: u16,
    timestamp_ns: u64,
) -> VideoAnalyticsNotifyAlert {
    let stream_id_hash = fnv1a(stream_id);
    let mut key = [0u8; 18];
    key[0..8].copy_from_slice(&stream_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&frame_seq.to_le_bytes());
    key[16] = alert_type;
    key[17] = severity;
    VideoAnalyticsNotifyAlert {
        content_hash: fnv1a(&key),
        stream_id_hash,
        frame_seq,
        alert_type,
        severity,
        trigger_count,
        timestamp_ns,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const STREAM: &[u8] = b"stream:cam-01";

    #[test]
    fn test_videoanalytics_to_db_record_hash_nonzero() {
        let rec = videoanalytics_to_db_record(STREAM, 1, 1920, 1080, 3, 12_000, 1_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.stream_id_hash, 0);
    }

    #[test]
    fn test_videoanalytics_to_db_record_fields() {
        let rec = videoanalytics_to_db_record(STREAM, 42, 1280, 720, 7, 30_000, 2_000_000_000);
        assert_eq!(rec.frame_seq, 42);
        assert_eq!(rec.frame_width, 1280);
        assert_eq!(rec.frame_height, 720);
        assert_eq!(rec.object_count, 7);
        assert_eq!(rec.motion_energy_x100, 30_000);
    }

    #[test]
    fn test_videoanalytics_to_cache_entry_normal_ttl() {
        let entry = videoanalytics_to_cache_entry(STREAM, 1, 614_400, 10_000);
        assert_eq!(entry.ttl_secs, 10);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_videoanalytics_to_cache_entry_high_motion_ttl() {
        // motion_energy_x100 > 50000 → TTL = 2 s.
        let entry = videoanalytics_to_cache_entry(STREAM, 2, 614_400, 80_000);
        assert_eq!(entry.ttl_secs, 2);
    }

    #[test]
    fn test_videoanalytics_to_analytics_metrics_avg() {
        // 100 frames, 300 detections → avg 3.0 objects/frame.
        let m = videoanalytics_to_analytics_metrics(100, 300, 500_000, 1_024, 0);
        assert_ne!(m.content_hash, 0);
        assert!((m.avg_objects_per_frame - 3.0).abs() < 0.01);
        assert_eq!(m.heatmap_cells_updated, 1_024);
    }

    #[test]
    fn test_videoanalytics_to_analytics_metrics_zero_frames() {
        let m = videoanalytics_to_analytics_metrics(0, 0, 0, 0, 0);
        assert_eq!(m.frames_analysed, 0);
        assert_eq!(m.avg_objects_per_frame, 0.0);
    }

    #[test]
    fn test_videoanalytics_to_stream_frame_fields() {
        let f = videoanalytics_to_stream_frame(STREAM, 99, 1920, 1080, 5, 2_048, 33_333_333);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.frame_width, 1920);
        assert_eq!(f.frame_height, 1080);
        assert_eq!(f.annotation_count, 5);
        assert_eq!(f.pts_ns, 33_333_333);
    }

    #[test]
    fn test_videoanalytics_to_notify_alert_deterministic() {
        let a = videoanalytics_to_notify_alert(STREAM, 7, 0, 2, 4, 500_000_000);
        let b = videoanalytics_to_notify_alert(STREAM, 7, 0, 2, 4, 500_000_000);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.alert_type, 0);
        assert_eq!(a.severity, 2);
        assert_eq!(a.trigger_count, 4);
    }
}
