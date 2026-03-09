//! Haptic bridges — ALICE-Haptic ↔ DB, Cache, Analytics, Edge, Render
//!
//! 5 bridges connecting haptic feedback processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Haptic → DB (pattern storage) ──────────────────────────────

/// Haptic pattern storage record for ALICE-DB persistence.
pub struct HapticDbRecord {
    /// Content hash over the pattern metadata.
    pub content_hash: u64,
    /// Number of haptic channels.
    pub channel_count: u8,
    /// Sample rate in Hz.
    pub sample_rate_hz: u32,
    /// Pattern duration in milliseconds.
    pub duration_ms: u64,
    /// Hash of the haptic pattern identifier.
    pub pattern_hash: u64,
    /// Maximum amplitude scaled by 1000.
    pub amplitude_max_x1000: u32,
}

/// Serialize haptic pattern metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn haptic_to_db_record(
    channel_count: u8,
    sample_rate_hz: u32,
    duration_ms: u64,
    pattern_hash: u64,
    amplitude_max_x1000: u32,
) -> HapticDbRecord {
    let mut buf = [0u8; 25];
    buf[0] = channel_count;
    buf[1..5].copy_from_slice(&sample_rate_hz.to_le_bytes());
    buf[5..13].copy_from_slice(&duration_ms.to_le_bytes());
    buf[13..21].copy_from_slice(&pattern_hash.to_le_bytes());
    buf[21..25].copy_from_slice(&amplitude_max_x1000.to_le_bytes());
    HapticDbRecord {
        content_hash: fnv1a(&buf),
        channel_count,
        sample_rate_hz,
        duration_ms,
        pattern_hash,
        amplitude_max_x1000,
    }
}

// ── Bridge 2: Haptic → Cache (waveform cache) ────────────────────────────

/// Haptic waveform cache entry for ALICE-Cache.
pub struct HapticCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Hash of the haptic pattern identifier.
    pub pattern_hash: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Waveform buffer size in bytes.
    pub waveform_bytes: u64,
    /// Number of haptic channels.
    pub channel_count: u8,
}

/// Build a haptic waveform cache entry for ALICE-Cache.
///
/// Multi-channel patterns (channel_count > 1) get a longer TTL (300 s) due
/// to higher synthesis cost; single-channel patterns get 60 s.
#[inline]
#[must_use]
pub fn haptic_to_cache_entry(
    pattern_hash: u64,
    waveform_bytes: u64,
    channel_count: u8,
) -> HapticCacheEntry {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&pattern_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&waveform_bytes.to_le_bytes());
    buf[16] = channel_count;
    let multichan = (channel_count > 1) as u32;
    let ttl_secs = 60 + multichan * 240;
    HapticCacheEntry {
        content_hash: fnv1a(&buf),
        pattern_hash,
        ttl_secs,
        waveform_bytes,
        channel_count,
    }
}

// ── Bridge 3: Haptic → Analytics (feedback event) ────────────────────────

/// Haptic feedback analytics event for ALICE-Analytics.
pub struct HapticAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Total number of haptic events fired.
    pub event_count: u64,
    /// Average actuator latency in microseconds.
    pub latency_us: u64,
    /// Average amplitude scaled by 1000.
    pub amplitude_avg_x1000: u32,
    /// Dominant frequency in Hz.
    pub frequency_hz: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a haptic feedback analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn haptic_to_analytics_event(
    event_count: u64,
    latency_us: u64,
    amplitude_avg_x1000: u32,
    frequency_hz: u32,
    timestamp_ms: u64,
) -> HapticAnalyticsEvent {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&event_count.to_le_bytes());
    buf[8..16].copy_from_slice(&latency_us.to_le_bytes());
    buf[16..20].copy_from_slice(&amplitude_avg_x1000.to_le_bytes());
    buf[20..24].copy_from_slice(&frequency_hz.to_le_bytes());
    buf[24..32].copy_from_slice(&timestamp_ms.to_le_bytes());
    HapticAnalyticsEvent {
        content_hash: fnv1a(&buf),
        event_count,
        latency_us,
        amplitude_avg_x1000,
        frequency_hz,
        timestamp_ms,
    }
}

// ── Bridge 4: Haptic → Edge (telemetry) ──────────────────────────────────

/// Edge device haptic telemetry for ALICE-Edge.
pub struct HapticEdgeTelemetry {
    /// Content hash over the telemetry tuple.
    pub content_hash: u64,
    /// Number of haptic channels on the device.
    pub channel_count: u8,
    /// Current sample rate in Hz.
    pub sample_rate_hz: u32,
    /// Buffer fill percentage (0-100).
    pub buffer_fill_pct: u8,
    /// Number of dropped samples since last report.
    pub dropped_count: u32,
}

/// Build haptic edge device telemetry for ALICE-Edge.
#[inline]
#[must_use]
pub fn haptic_to_edge_telemetry(
    channel_count: u8,
    sample_rate_hz: u32,
    buffer_fill_pct: u8,
    dropped_count: u32,
) -> HapticEdgeTelemetry {
    let mut buf = [0u8; 10];
    buf[0] = channel_count;
    buf[1..5].copy_from_slice(&sample_rate_hz.to_le_bytes());
    buf[5] = buffer_fill_pct;
    buf[6..10].copy_from_slice(&dropped_count.to_le_bytes());
    HapticEdgeTelemetry {
        content_hash: fnv1a(&buf),
        channel_count,
        sample_rate_hz,
        buffer_fill_pct,
        dropped_count,
    }
}

// ── Bridge 5: Haptic → Render (output descriptor) ────────────────────────

/// Haptic render output descriptor for ALICE-Render.
pub struct HapticRenderOutput {
    /// Content hash over the render output.
    pub content_hash: u64,
    /// Number of output channels rendered.
    pub channel_count: u8,
    /// Total samples produced.
    pub sample_count: u64,
    /// Render time in microseconds.
    pub render_time_us: u64,
    /// Total output buffer size in bytes.
    pub output_bytes: u64,
}

/// Build a haptic render output descriptor for ALICE-Render.
#[inline]
#[must_use]
pub fn haptic_to_render_output(
    channel_count: u8,
    sample_count: u64,
    render_time_us: u64,
    output_bytes: u64,
) -> HapticRenderOutput {
    let mut buf = [0u8; 25];
    buf[0] = channel_count;
    buf[1..9].copy_from_slice(&sample_count.to_le_bytes());
    buf[9..17].copy_from_slice(&render_time_us.to_le_bytes());
    buf[17..25].copy_from_slice(&output_bytes.to_le_bytes());
    HapticRenderOutput {
        content_hash: fnv1a(&buf),
        channel_count,
        sample_count,
        render_time_us,
        output_bytes,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_haptic_to_db_record_hash_nonzero() {
        let rec = haptic_to_db_record(2, 48_000, 500, 0x1234, 800_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_haptic_to_db_record_fields() {
        let rec = haptic_to_db_record(4, 96_000, 1_000, 0xabcd, 1_000_000);
        assert_eq!(rec.channel_count, 4);
        assert_eq!(rec.sample_rate_hz, 96_000);
        assert_eq!(rec.duration_ms, 1_000);
        assert_eq!(rec.pattern_hash, 0xabcd);
        assert_eq!(rec.amplitude_max_x1000, 1_000_000);
    }

    #[test]
    fn test_haptic_to_db_record_deterministic() {
        let a = haptic_to_db_record(1, 2, 3, 4, 5);
        let b = haptic_to_db_record(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_haptic_to_cache_entry_single_channel_ttl() {
        let entry = haptic_to_cache_entry(0x5678, 4_096, 1);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 60);
        assert_eq!(entry.channel_count, 1);
    }

    #[test]
    fn test_haptic_to_cache_entry_multi_channel_ttl() {
        let entry = haptic_to_cache_entry(0x9abc, 16_384, 4);
        assert_eq!(entry.ttl_secs, 300);
        assert_eq!(entry.channel_count, 4);
    }

    #[test]
    fn test_haptic_to_analytics_event() {
        let ev = haptic_to_analytics_event(200, 800, 750_000, 200, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.event_count, 200);
        assert_eq!(ev.latency_us, 800);
        assert_eq!(ev.frequency_hz, 200);
    }

    #[test]
    fn test_haptic_to_edge_telemetry() {
        let tel = haptic_to_edge_telemetry(2, 48_000, 75, 3);
        assert_ne!(tel.content_hash, 0);
        assert_eq!(tel.buffer_fill_pct, 75);
        assert_eq!(tel.dropped_count, 3);
    }

    #[test]
    fn test_haptic_to_render_output() {
        let out = haptic_to_render_output(2, 4_800, 500, 9_600);
        assert_ne!(out.content_hash, 0);
        assert_eq!(out.sample_count, 4_800);
        assert_eq!(out.render_time_us, 500);
        assert_eq!(out.output_bytes, 9_600);
    }
}
