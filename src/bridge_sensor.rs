//! Sensor bridges — ALICE-Sensor ↔ DB, Cache, Analytics, Edge, Monitor
//!
//! 5 bridges connecting sensor sample and channel data (extracted as
//! primitives) to the ALICE ecosystem. No external crate types are imported;
//! all fields use primitive types derived from serialised sensor state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Sensor → DB (sample snapshot persistence) ───────────────────

/// Sensor sample snapshot for ALICE-DB persistence.
pub struct SensorDbRecord {
    /// Content hash over sensor_hash and sample_count.
    pub content_hash: u64,
    /// Opaque sensor identifier hash.
    pub sensor_hash: u64,
    /// Total samples recorded since the sensor started.
    pub sample_count: u64,
    /// Number of measurement channels on this sensor.
    pub channel_count: u8,
    /// ADC resolution in bits.
    pub resolution_bits: u8,
    /// Configured sample rate in hertz.
    pub sample_rate_hz: u32,
}

/// Build a DB persistence record from extracted sensor configuration data.
#[inline]
#[must_use]
pub fn sensor_to_db_record(
    sensor_id: &[u8],
    sample_count: u64,
    channel_count: u8,
    resolution_bits: u8,
    sample_rate_hz: u32,
) -> SensorDbRecord {
    let sensor_hash = fnv1a(sensor_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&sensor_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&sample_count.to_le_bytes());
    SensorDbRecord {
        content_hash: fnv1a(&buf),
        sensor_hash,
        sample_count,
        channel_count,
        resolution_bits,
        sample_rate_hz,
    }
}

// ── Bridge 2: Sensor → Cache (live sample buffer caching) ─────────────────

/// Cached sensor live-state entry for ALICE-Cache.
pub struct SensorCacheEntry {
    /// Content hash over sensor_hash and sample_count.
    pub content_hash: u64,
    /// Hashed sensor identifier used as cache key.
    pub sensor_hash: u64,
    /// TTL in seconds for this cache entry.
    pub ttl_secs: u32,
    /// Total samples recorded at cache time.
    pub sample_count: u64,
    /// Serialised sample buffer size in bytes.
    pub buffer_bytes: u64,
}

/// Build a cache entry for a sensor's live sample buffer.
///
/// TTL is 5 s by default; reduced to 1 s when `sample_rate_hz` exceeds 1000
/// to keep high-frequency sensor data current.
#[inline]
#[must_use]
pub fn sensor_to_cache_entry(
    sensor_id: &[u8],
    sample_count: u64,
    buffer_bytes: u64,
    sample_rate_hz: u32,
) -> SensorCacheEntry {
    let sensor_hash = fnv1a(sensor_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&sensor_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&sample_count.to_le_bytes());
    let high_rate = (sample_rate_hz > 1_000) as u32;
    let ttl_secs = 5 - high_rate * 4;
    SensorCacheEntry {
        content_hash: fnv1a(&buf),
        sensor_hash,
        ttl_secs,
        sample_count,
        buffer_bytes,
    }
}

// ── Bridge 3: Sensor → Analytics (signal quality ingestion) ───────────────

/// Sensor signal quality event for ALICE-Analytics ingestion.
pub struct SensorAnalyticsEvent {
    /// Content hash over sensor_hash and timestamp_ms.
    pub content_hash: u64,
    /// Total samples recorded at event time.
    pub sample_count: u64,
    /// Number of anomalous samples detected since power-on.
    pub anomaly_count: u32,
    /// Sensor drift multiplied by 1000 (unit/s × 1000).
    pub drift_x1000: u32,
    /// Signal-to-noise ratio multiplied by 100 (dB × 100).
    pub snr_x100: u32,
    /// Unix timestamp in milliseconds when the event was recorded.
    pub timestamp_ms: u64,
}

/// Build an analytics ingestion event from sensor signal quality metrics.
#[inline]
#[must_use]
pub fn sensor_to_analytics_event(
    sensor_id: &[u8],
    sample_count: u64,
    anomaly_count: u32,
    drift_x1000: u32,
    snr_x100: u32,
    timestamp_ms: u64,
) -> SensorAnalyticsEvent {
    let sensor_hash = fnv1a(sensor_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&sensor_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    SensorAnalyticsEvent {
        content_hash: fnv1a(&buf),
        sample_count,
        anomaly_count,
        drift_x1000,
        snr_x100,
        timestamp_ms,
    }
}

// ── Bridge 4: Sensor → Edge (on-device telemetry) ─────────────────────────

/// Sensor on-device telemetry for ALICE-Edge transmission.
pub struct SensorEdgeTelemetry {
    /// Content hash over sensor_hash and sample_rate_hz.
    pub content_hash: u64,
    /// Hashed sensor identifier.
    pub sensor_hash: u64,
    /// Current sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Remaining battery charge as a percentage (0–100).
    pub battery_pct: u8,
    /// Received signal strength indicator in dBm (signed).
    pub rssi_dbm: i8,
}

/// Build an edge telemetry packet from sensor radio and power data.
#[inline]
#[must_use]
pub fn sensor_to_edge_telemetry(
    sensor_id: &[u8],
    sample_rate_hz: u32,
    battery_pct: u8,
    rssi_dbm: i8,
) -> SensorEdgeTelemetry {
    let sensor_hash = fnv1a(sensor_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&sensor_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&sample_rate_hz.to_le_bytes());
    SensorEdgeTelemetry {
        content_hash: fnv1a(&buf),
        sensor_hash,
        sample_rate_hz,
        battery_pct,
        rssi_dbm,
    }
}

// ── Bridge 5: Sensor → Monitor (online health status) ─────────────────────

/// Sensor health status for ALICE-Monitor.
pub struct SensorMonitorStatus {
    /// Content hash over sensor_hash and timestamp_ms.
    pub content_hash: u64,
    /// Hashed sensor identifier.
    pub sensor_hash: u64,
    /// Whether the sensor is currently online and sampling.
    pub is_online: bool,
    /// Unix timestamp in milliseconds of the last received sample.
    pub last_sample_ts: u64,
    /// Error count since the sensor last came online.
    pub error_count: u32,
    /// Unix timestamp in milliseconds of this status report.
    pub timestamp_ms: u64,
}

/// Build a monitor health status from sensor connectivity data.
#[inline]
#[must_use]
pub fn sensor_to_monitor_status(
    sensor_id: &[u8],
    is_online: bool,
    last_sample_ts: u64,
    error_count: u32,
    timestamp_ms: u64,
) -> SensorMonitorStatus {
    let sensor_hash = fnv1a(sensor_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&sensor_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    SensorMonitorStatus {
        content_hash: fnv1a(&buf),
        sensor_hash,
        is_online,
        last_sample_ts,
        error_count,
        timestamp_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = sensor_to_db_record(b"sensor-01", 100_000, 4, 16, 500);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.sensor_hash, 0);
    }

    #[test]
    fn db_record_fields_preserved() {
        let rec = sensor_to_db_record(b"s", 50_000, 2, 12, 1_000);
        assert_eq!(rec.sample_count, 50_000);
        assert_eq!(rec.channel_count, 2);
        assert_eq!(rec.resolution_bits, 12);
        assert_eq!(rec.sample_rate_hz, 1_000);
    }

    #[test]
    fn db_record_hash_deterministic() {
        let a = sensor_to_db_record(b"sx", 0, 1, 8, 100);
        let b = sensor_to_db_record(b"sx", 0, 1, 8, 100);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_low_rate_long_ttl() {
        let entry = sensor_to_cache_entry(b"s1", 1_000, 4_096, 100);
        assert_eq!(entry.ttl_secs, 5);
    }

    #[test]
    fn cache_entry_high_rate_short_ttl() {
        let entry = sensor_to_cache_entry(b"s2", 10_000, 8_192, 5_000);
        assert_eq!(entry.ttl_secs, 1);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_fields_and_hash() {
        let ev = sensor_to_analytics_event(b"sn-42", 1_000_000, 7, 250, 7_500, 5_000_000);
        assert_eq!(ev.sample_count, 1_000_000);
        assert_eq!(ev.anomaly_count, 7);
        assert_eq!(ev.drift_x1000, 250);
        assert_eq!(ev.snr_x100, 7_500);
        assert_ne!(ev.content_hash, 0);
    }

    // ── Edge telemetry tests ──────────────────────────────────────────────

    #[test]
    fn edge_telemetry_signed_rssi_preserved() {
        let tel = sensor_to_edge_telemetry(b"se", 250, 85, -72_i8);
        assert_eq!(tel.rssi_dbm, -72);
        assert_eq!(tel.battery_pct, 85);
        assert_ne!(tel.content_hash, 0);
    }

    // ── Monitor status tests ──────────────────────────────────────────────

    #[test]
    fn monitor_status_offline_flag_preserved() {
        let st = sensor_to_monitor_status(b"sm", false, 0, 10, 99_000);
        assert!(!st.is_online);
        assert_eq!(st.error_count, 10);
        assert_ne!(st.content_hash, 0);
    }

    #[test]
    fn different_sensors_produce_different_hashes() {
        let a = sensor_to_db_record(b"sensor-A", 0, 1, 8, 100);
        let b = sensor_to_db_record(b"sensor-B", 0, 1, 8, 100);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
