//! Video bridges — Video ↔ DB, Cache, Analytics, Streaming, CDN
//!
//! 5 bridges connecting video encode/decode pipelines to the ALICE ecosystem.
//! Covers metadata persistence, frame caching, encoding metrics,
//! packet streaming, and CDN delivery.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Video → DB (metadata persistence) ─────────────────────────

/// Video metadata record for ALICE-DB persistence.
///
/// One record per encoded video asset. `content_hash` is derived from the
/// codec, resolution, and bitrate so duplicate assets are detectable.
pub struct VideoDbRecord {
    /// FNV-1a hash of codec + resolution + bitrate — deduplication key.
    pub content_hash: u64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Total number of frames in the video.
    pub frame_count: u64,
    /// Bitrate in kilobits per second.
    pub bitrate_kbps: u32,
    /// Keyframe interval (frames between I-frames).
    pub keyframe_interval: u32,
    /// FNV-1a hash of the codec identifier string.
    pub codec_hash: u64,
    /// Duration in milliseconds derived from frame_count and frame_rate_milli_hz.
    pub duration_ms: u64,
}

/// Build a `VideoDbRecord` from raw video parameters.
///
/// `frame_rate_milli_hz` is the frame rate expressed in milli-hertz
/// (e.g. 30000 = 30 Hz) to keep the interface integer-only.
/// `duration_ms` is computed branchlessly — zero frame-rate is guarded by max(1).
#[inline]
#[must_use]
pub fn video_to_db_record(
    width: u32,
    height: u32,
    frame_count: u64,
    bitrate_kbps: u32,
    keyframe_interval: u32,
    codec: &str,
    frame_rate_milli_hz: u32,
) -> VideoDbRecord {
    let codec_hash = fnv1a(codec.as_bytes());
    let mut buf = [0u8; 24];
    buf[0..4].copy_from_slice(&width.to_le_bytes());
    buf[4..8].copy_from_slice(&height.to_le_bytes());
    buf[8..12].copy_from_slice(&bitrate_kbps.to_le_bytes());
    buf[12..20].copy_from_slice(&codec_hash.to_le_bytes());
    buf[20..24].copy_from_slice(&keyframe_interval.to_le_bytes());
    let content_hash = fnv1a(&buf);
    // duration_ms = frame_count * 1000 / (frame_rate_milli_hz / 1000)
    //             = frame_count * 1_000_000 / frame_rate_milli_hz
    let safe_fps = frame_rate_milli_hz.max(1) as u64;
    let duration_ms = frame_count.saturating_mul(1_000_000) / safe_fps;
    VideoDbRecord {
        content_hash,
        width,
        height,
        frame_count,
        bitrate_kbps,
        keyframe_interval,
        codec_hash,
        duration_ms,
    }
}

// ── Bridge 2: Video → Cache (frame cache) ───────────────────────────────

/// Frame cache entry descriptor for ALICE-Cache.
///
/// Describes a decoded frame stored in the cache; the key is
/// `content_hash ^ frame_index` so each frame of a video is individually
/// addressable.
pub struct VideoFrameCacheEntry {
    /// FNV-1a hash of video identity bytes XOR'd with frame_index.
    pub content_hash: u64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Zero-based index of this frame within the video.
    pub frame_index: u64,
    /// Uncompressed frame size in bytes (width * height * bytes_per_pixel).
    pub frame_bytes: u64,
    /// Cache TTL in seconds — keyframes live longer than inter-frames.
    pub ttl_secs: u32,
    /// Whether this frame is a keyframe (I-frame).
    pub is_keyframe: bool,
}

/// Build a `VideoFrameCacheEntry` for a given frame.
///
/// `is_keyframe` is determined by `frame_index % keyframe_interval == 0`.
/// TTL is 300 s for keyframes and 60 s for inter-frames (branchless).
#[inline]
#[must_use]
pub fn video_to_frame_cache_entry(
    video_hash: u64,
    width: u32,
    height: u32,
    frame_index: u64,
    keyframe_interval: u32,
    bytes_per_pixel: u32,
) -> VideoFrameCacheEntry {
    let is_keyframe = keyframe_interval > 0 && frame_index.is_multiple_of(keyframe_interval as u64);
    // Branchless TTL: keyframe=300s, inter=60s.
    let is_key_u32 = is_keyframe as u32;
    let ttl_secs = 60 + is_key_u32 * 240;
    let frame_bytes = width as u64 * height as u64 * bytes_per_pixel as u64;
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&video_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&frame_index.to_le_bytes());
    let content_hash = fnv1a(&buf);
    VideoFrameCacheEntry {
        content_hash,
        width,
        height,
        frame_index,
        frame_bytes,
        ttl_secs,
        is_keyframe,
    }
}

// ── Bridge 3: Video → Analytics (encoding metrics) ──────────────────────

/// Encoding metrics event for ALICE-Analytics.
///
/// Emitted once per encoded segment for bitrate, quality, and timing analysis.
pub struct VideoAnalyticsMetrics {
    /// FNV-1a hash of session_id + segment_index — deduplication key.
    pub content_hash: u64,
    /// Number of frames in the segment.
    pub frame_count: u32,
    /// Segment bitrate in kilobits per second.
    pub bitrate_kbps: u32,
    /// Encoding time for the segment in microseconds.
    pub encode_time_us: u64,
    /// Number of keyframes in the segment.
    pub keyframe_count: u32,
    /// Average quantization parameter (0–51 for H.264/H.265; 0 = lossless).
    pub avg_qp: u8,
    /// Pixel area (width * height) for throughput computation.
    pub pixel_area: u64,
}

/// Build a `VideoAnalyticsMetrics` event for a completed encode segment.
#[inline]
#[must_use]
pub fn video_to_analytics_metrics(
    session_id: &str,
    segment_index: u32,
    frame_count: u32,
    bitrate_kbps: u32,
    encode_time_us: u64,
    keyframe_count: u32,
    avg_qp: u8,
    width: u32,
    height: u32,
) -> VideoAnalyticsMetrics {
    let session_hash = fnv1a(session_id.as_bytes());
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&session_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&segment_index.to_le_bytes());
    let content_hash = fnv1a(&buf);
    let pixel_area = width as u64 * height as u64;
    VideoAnalyticsMetrics {
        content_hash,
        frame_count,
        bitrate_kbps,
        encode_time_us,
        keyframe_count,
        avg_qp,
        pixel_area,
    }
}

// ── Bridge 4: Video → Streaming (packet descriptor) ─────────────────────

/// Streaming packet descriptor for ALICE-Streaming.
///
/// Describes one network packet carrying a video NAL unit or chunk.
/// The receiving streaming layer uses `seq` and `content_hash` to
/// detect loss and reorder packets.
pub struct VideoStreamPacket {
    /// FNV-1a hash of stream_id + seq — packet identity key.
    pub content_hash: u64,
    /// Monotonically increasing packet sequence number.
    pub seq: u64,
    /// Presentation timestamp in microseconds.
    pub pts_us: u64,
    /// Payload size in bytes.
    pub payload_bytes: u32,
    /// NAL unit type (H.264/H.265 value; 0 = unknown).
    pub nal_type: u8,
    /// Whether this packet marks the end of an access unit.
    pub is_end_of_unit: bool,
    /// RTP-style SSRC (synchronization source identifier).
    pub ssrc: u32,
}

/// Build a `VideoStreamPacket` descriptor for one network packet.
#[inline]
#[must_use]
pub fn video_to_stream_packet(
    stream_id: u64,
    seq: u64,
    pts_us: u64,
    payload_bytes: u32,
    nal_type: u8,
    is_end_of_unit: bool,
    ssrc: u32,
) -> VideoStreamPacket {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&stream_id.to_le_bytes());
    buf[8..16].copy_from_slice(&seq.to_le_bytes());
    let content_hash = fnv1a(&buf);
    VideoStreamPacket {
        content_hash,
        seq,
        pts_us,
        payload_bytes,
        nal_type,
        is_end_of_unit,
        ssrc,
    }
}

// ── Bridge 5: Video → CDN (delivery descriptor) ─────────────────────────

/// CDN delivery descriptor for ALICE-CDN.
///
/// Instructs the CDN layer how to cache and serve a video segment,
/// including edge-node hints and cache-control headers.
pub struct VideoCdnDelivery {
    /// FNV-1a hash of asset_id + segment_index — CDN cache key.
    pub content_hash: u64,
    /// Segment index within the video manifest.
    pub segment_index: u32,
    /// Segment duration in milliseconds.
    pub segment_duration_ms: u32,
    /// Recommended CDN edge TTL in seconds.
    pub edge_ttl_secs: u32,
    /// Encoded segment size in bytes.
    pub segment_bytes: u64,
    /// Whether the segment begins with a keyframe (suitable for seek entry).
    pub is_seek_point: bool,
    /// FNV-1a hash of the origin URL string for edge routing.
    pub origin_url_hash: u64,
}

/// Build a `VideoCdnDelivery` descriptor for a video segment.
///
/// `edge_ttl_secs` is 3600 s for seek-point segments (higher reuse) and
/// 300 s for inter-segments (branchless selection).
#[inline]
#[must_use]
pub fn video_to_cdn_delivery(
    asset_id: u64,
    segment_index: u32,
    segment_duration_ms: u32,
    segment_bytes: u64,
    is_seek_point: bool,
    origin_url: &str,
) -> VideoCdnDelivery {
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&asset_id.to_le_bytes());
    buf[8..12].copy_from_slice(&segment_index.to_le_bytes());
    let content_hash = fnv1a(&buf);
    let origin_url_hash = fnv1a(origin_url.as_bytes());
    // Branchless TTL: seek-point = 3600s, inter = 300s.
    let is_seek_u32 = is_seek_point as u32;
    let edge_ttl_secs = 300 + is_seek_u32 * 3300;
    VideoCdnDelivery {
        content_hash,
        segment_index,
        segment_duration_ms,
        edge_ttl_secs,
        segment_bytes,
        is_seek_point,
        origin_url_hash,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_db_record_hash_nonzero() {
        let rec = video_to_db_record(1920, 1080, 1800, 4000, 30, "h264", 30_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.codec_hash, 0);
    }

    #[test]
    fn test_video_db_record_deterministic() {
        let a = video_to_db_record(1280, 720, 600, 2000, 60, "h265", 60_000);
        let b = video_to_db_record(1280, 720, 600, 2000, 60, "h265", 60_000);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.codec_hash, b.codec_hash);
    }

    #[test]
    fn test_video_db_record_duration() {
        // 300 frames at 30 Hz = 10_000 ms
        let rec = video_to_db_record(1920, 1080, 300, 4000, 30, "h264", 30_000);
        assert_eq!(rec.duration_ms, 10_000);
    }

    #[test]
    fn test_video_frame_cache_keyframe_ttl() {
        let entry = video_to_frame_cache_entry(0xdeadbeef, 1920, 1080, 0, 30, 3);
        assert!(entry.is_keyframe);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_video_frame_cache_inter_ttl() {
        let entry = video_to_frame_cache_entry(0xdeadbeef, 1920, 1080, 1, 30, 3);
        assert!(!entry.is_keyframe);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_video_analytics_metrics_fields() {
        let m = video_to_analytics_metrics("sess-1", 0, 300, 4000, 500_000, 10, 28, 1920, 1080);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.frame_count, 300);
        assert_eq!(m.pixel_area, 1920 * 1080);
        assert_eq!(m.avg_qp, 28);
    }

    #[test]
    fn test_video_stream_packet_fields() {
        let pkt = video_to_stream_packet(1, 42, 1_000_000, 1316, 5, true, 0xabc);
        assert_ne!(pkt.content_hash, 0);
        assert_eq!(pkt.seq, 42);
        assert!(pkt.is_end_of_unit);
        assert_eq!(pkt.ssrc, 0xabc);
    }

    #[test]
    fn test_video_cdn_delivery_seek_ttl() {
        let d = video_to_cdn_delivery(99, 0, 2000, 512_000, true, "https://origin.example.com");
        assert!(d.is_seek_point);
        assert_eq!(d.edge_ttl_secs, 3600);
        assert_ne!(d.origin_url_hash, 0);
    }

    #[test]
    fn test_video_cdn_delivery_inter_ttl() {
        let d = video_to_cdn_delivery(99, 1, 2000, 256_000, false, "https://origin.example.com");
        assert!(!d.is_seek_point);
        assert_eq!(d.edge_ttl_secs, 300);
    }
}
