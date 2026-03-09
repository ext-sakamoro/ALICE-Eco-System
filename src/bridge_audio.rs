//! Audio bridges — ALICE-Audio ↔ DB, Cache, Analytics, Streaming, CDN
//!
//! 5 bridges connecting audio track processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Audio → DB (track storage) ────────────────────────────────

/// Track storage record for ALICE-DB persistence.
pub struct AudioDbRecord {
    /// Content hash over the track metadata.
    pub content_hash: u64,
    /// Sample rate in Hz (e.g. 44100, 48000).
    pub sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo).
    pub channel_count: u8,
    /// Track duration in milliseconds.
    pub duration_ms: u64,
    /// RMS signal level in dBFS (negative value).
    pub rms_level: f32,
    /// Peak signal level in dBFS (negative value, 0.0 = full scale).
    pub peak_level: f32,
}

/// Serialize audio track metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn audio_to_db_record(
    sample_rate: u32,
    channel_count: u8,
    duration_ms: u64,
    rms_level: f32,
    peak_level: f32,
) -> AudioDbRecord {
    let mut buf = [0u8; 21];
    buf[0..4].copy_from_slice(&sample_rate.to_le_bytes());
    buf[4] = channel_count;
    buf[5..13].copy_from_slice(&duration_ms.to_le_bytes());
    buf[13..17].copy_from_slice(&rms_level.to_bits().to_le_bytes());
    buf[17..21].copy_from_slice(&peak_level.to_bits().to_le_bytes());
    AudioDbRecord {
        content_hash: fnv1a(&buf),
        sample_rate,
        channel_count,
        duration_ms,
        rms_level,
        peak_level,
    }
}

// ── Bridge 2: Audio → Cache (buffer cache) ──────────────────────────────

/// Decoded audio buffer cache entry for ALICE-Cache.
pub struct AudioCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Sample rate of the cached buffer.
    pub sample_rate: u32,
    /// Number of channels in the cached buffer.
    pub channel_count: u8,
    /// Duration of the cached buffer in milliseconds.
    pub duration_ms: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Whether the buffer is losslessly encoded.
    pub is_lossless: bool,
}

/// Build a decoded audio buffer cache entry for ALICE-Cache.
///
/// Lossless buffers receive a longer TTL (600 s vs 120 s) because re-decoding
/// them is more expensive than lossy formats.
#[inline]
#[must_use]
pub fn audio_to_cache_entry(
    sample_rate: u32,
    channel_count: u8,
    duration_ms: u64,
    is_lossless: bool,
) -> AudioCacheEntry {
    let mut buf = [0u8; 14];
    buf[0..4].copy_from_slice(&sample_rate.to_le_bytes());
    buf[4] = channel_count;
    buf[5..13].copy_from_slice(&duration_ms.to_le_bytes());
    buf[13] = is_lossless as u8;
    let lossless_flag = is_lossless as u32;
    let ttl_secs = 120 + lossless_flag * 480;
    AudioCacheEntry {
        content_hash: fnv1a(&buf),
        sample_rate,
        channel_count,
        duration_ms,
        ttl_secs,
        is_lossless,
    }
}

// ── Bridge 3: Audio → Analytics (audio metrics) ─────────────────────────

/// Audio quality metrics for ALICE-Analytics ingestion.
pub struct AudioAnalyticsMetrics {
    /// Content hash over the metric tuple.
    pub content_hash: u64,
    /// Sample rate of the track.
    pub sample_rate: u32,
    /// Number of channels.
    pub channel_count: u8,
    /// Track duration in milliseconds.
    pub duration_ms: u64,
    /// RMS level in dBFS.
    pub rms_level: f32,
    /// Peak level in dBFS.
    pub peak_level: f32,
}

/// Build audio quality metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn audio_to_analytics_metrics(
    sample_rate: u32,
    channel_count: u8,
    duration_ms: u64,
    rms_level: f32,
    peak_level: f32,
) -> AudioAnalyticsMetrics {
    let mut buf = [0u8; 21];
    buf[0..4].copy_from_slice(&sample_rate.to_le_bytes());
    buf[4] = channel_count;
    buf[5..13].copy_from_slice(&duration_ms.to_le_bytes());
    buf[13..17].copy_from_slice(&rms_level.to_bits().to_le_bytes());
    buf[17..21].copy_from_slice(&peak_level.to_bits().to_le_bytes());
    AudioAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        sample_rate,
        channel_count,
        duration_ms,
        rms_level,
        peak_level,
    }
}

// ── Bridge 4: Audio → Streaming (packet) ────────────────────────────────

/// Audio streaming packet for ALICE-Streaming.
pub struct AudioStreamingPacket {
    /// Content hash over the packet payload.
    pub content_hash: u64,
    /// Sample rate of the stream.
    pub sample_rate: u32,
    /// Number of channels in the stream.
    pub channel_count: u8,
    /// Number of audio frames in this packet.
    pub frame_count: u32,
    /// Sequence number for ordering.
    pub sequence_number: u64,
    /// Presentation timestamp in microseconds.
    pub pts_us: u64,
}

/// Build an audio streaming packet for ALICE-Streaming.
#[inline]
#[must_use]
pub fn audio_to_streaming_packet(
    sample_rate: u32,
    channel_count: u8,
    frame_count: u32,
    sequence_number: u64,
    pts_us: u64,
) -> AudioStreamingPacket {
    let mut buf = [0u8; 25];
    buf[0..4].copy_from_slice(&sample_rate.to_le_bytes());
    buf[4] = channel_count;
    buf[5..9].copy_from_slice(&frame_count.to_le_bytes());
    buf[9..17].copy_from_slice(&sequence_number.to_le_bytes());
    buf[17..25].copy_from_slice(&pts_us.to_le_bytes());
    AudioStreamingPacket {
        content_hash: fnv1a(&buf),
        sample_rate,
        channel_count,
        frame_count,
        sequence_number,
        pts_us,
    }
}

// ── Bridge 5: Audio → CDN (delivery) ────────────────────────────────────

/// CDN delivery descriptor for ALICE-CDN.
pub struct AudioCdnDelivery {
    /// Content hash used as CDN cache key.
    pub content_hash: u64,
    /// Sample rate of the delivered track.
    pub sample_rate: u32,
    /// Number of channels.
    pub channel_count: u8,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Estimated payload size in bytes.
    pub payload_bytes: u64,
    /// MIME type for the Content-Type header.
    pub mime_type: &'static str,
}

/// Build a CDN delivery descriptor for ALICE-CDN.
///
/// Payload size is estimated as `sample_rate * channel_count * (duration_ms / 1000) * 2`
/// (16-bit PCM baseline) using reciprocal multiply to avoid division.
#[inline]
#[must_use]
pub fn audio_to_cdn_delivery(
    sample_rate: u32,
    channel_count: u8,
    duration_ms: u64,
    mime_type: &'static str,
) -> AudioCdnDelivery {
    // Reciprocal of 1000 to convert ms → s without division in the hot path.
    const RCP_MS_TO_S: f64 = 1.0 / 1_000.0;
    let duration_secs = duration_ms as f64 * RCP_MS_TO_S;
    let payload_bytes = (sample_rate as f64 * channel_count as f64 * duration_secs * 2.0) as u64;
    let mut buf = [0u8; 13];
    buf[0..4].copy_from_slice(&sample_rate.to_le_bytes());
    buf[4] = channel_count;
    buf[5..13].copy_from_slice(&duration_ms.to_le_bytes());
    AudioCdnDelivery {
        content_hash: fnv1a(&buf),
        sample_rate,
        channel_count,
        duration_ms,
        payload_bytes,
        mime_type,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_to_db_record_hash_nonzero() {
        let rec = audio_to_db_record(44_100, 2, 180_000, -18.0, -3.0);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_audio_to_db_record_fields() {
        let rec = audio_to_db_record(48_000, 1, 60_000, -20.0, -1.0);
        assert_eq!(rec.sample_rate, 48_000);
        assert_eq!(rec.channel_count, 1);
        assert_eq!(rec.duration_ms, 60_000);
        assert!((rec.rms_level - (-20.0)).abs() < 1e-5);
        assert!((rec.peak_level - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn test_audio_to_cache_entry_lossy_ttl() {
        let entry = audio_to_cache_entry(44_100, 2, 120_000, false);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 120);
        assert!(!entry.is_lossless);
    }

    #[test]
    fn test_audio_to_cache_entry_lossless_ttl() {
        let entry = audio_to_cache_entry(96_000, 2, 300_000, true);
        assert_eq!(entry.ttl_secs, 600);
        assert!(entry.is_lossless);
    }

    #[test]
    fn test_audio_to_analytics_metrics() {
        let m = audio_to_analytics_metrics(44_100, 2, 240_000, -14.0, -0.5);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.sample_rate, 44_100);
        assert_eq!(m.duration_ms, 240_000);
    }

    #[test]
    fn test_audio_to_streaming_packet() {
        let pkt = audio_to_streaming_packet(48_000, 2, 960, 42, 875_000);
        assert_ne!(pkt.content_hash, 0);
        assert_eq!(pkt.frame_count, 960);
        assert_eq!(pkt.sequence_number, 42);
        assert_eq!(pkt.pts_us, 875_000);
    }

    #[test]
    fn test_audio_to_cdn_delivery_payload_size() {
        // 44100 Hz, stereo, 1000 ms → 44100 * 2 * 1 * 2 = 176400 bytes.
        let d = audio_to_cdn_delivery(44_100, 2, 1_000, "audio/wav");
        assert_ne!(d.content_hash, 0);
        assert_eq!(d.payload_bytes, 176_400);
        assert_eq!(d.mime_type, "audio/wav");
    }

    #[test]
    fn test_audio_to_cdn_delivery_zero_duration() {
        let d = audio_to_cdn_delivery(44_100, 2, 0, "audio/ogg");
        assert_eq!(d.payload_bytes, 0);
    }
}
