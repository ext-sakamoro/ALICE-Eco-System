//! LoRa bridges — ALICE-LoRa ↔ DB, Cache, Analytics, Monitor, Edge
//!
//! 5 bridges connecting LoRa/LoRaWAN radio management to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LoRa → DB (message log) ───────────────────────────────────

/// LoRa uplink message log record for ALICE-DB persistence.
pub struct LoraDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// DevEUI (64-bit device identifier).
    pub dev_eui: u64,
    /// LoRa spreading factor (7–12).
    pub spreading_factor: u8,
    /// Channel bandwidth in kHz (e.g. 125, 250, 500).
    pub bandwidth_khz: u16,
    /// Signal-to-noise ratio in dB, fixed-point ×10 (e.g. -75 = -7.5 dB).
    pub snr_x10: i16,
    /// RSSI in dBm (signed, stored as i16).
    pub rssi: i16,
    /// Uplink payload size in bytes.
    pub payload_bytes: u16,
    /// Unix timestamp of the received message (seconds).
    pub received_at_ts: u64,
}

/// Convert a LoRa uplink message into an ALICE-DB log record.
#[inline]
#[must_use]
pub fn lora_to_db_record(
    dev_eui: u64,
    spreading_factor: u8,
    bandwidth_khz: u16,
    snr_x10: i16,
    rssi: i16,
    payload_bytes: u16,
    received_at_ts: u64,
) -> LoraDbRecord {
    let mut data = [0u8; 21];
    data[0..8].copy_from_slice(&dev_eui.to_le_bytes());
    data[8] = spreading_factor;
    data[9..11].copy_from_slice(&bandwidth_khz.to_le_bytes());
    data[11..13].copy_from_slice(&snr_x10.to_le_bytes());
    data[13..15].copy_from_slice(&rssi.to_le_bytes());
    data[15..17].copy_from_slice(&payload_bytes.to_le_bytes());
    data[17..21].copy_from_slice(&(received_at_ts as u32).to_le_bytes());
    LoraDbRecord {
        content_hash: fnv1a(&data),
        dev_eui,
        spreading_factor,
        bandwidth_khz,
        snr_x10,
        rssi,
        payload_bytes,
        received_at_ts,
    }
}

// ── Bridge 2: LoRa → Cache (session cache) ──────────────────────────────

/// LoRa device session cache entry for ALICE-Cache.
pub struct LoraCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// DevEUI (64-bit device identifier).
    pub dev_eui: u64,
    /// Network session key identifier (lower 32 bits).
    pub nwk_s_key_id: u32,
    /// Frame counter (uplink).
    pub f_cnt_up: u32,
    /// Last-seen spreading factor.
    pub spreading_factor: u8,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build an ALICE-Cache session entry from LoRa join/session state.
#[inline]
#[must_use]
pub fn lora_to_cache_entry(
    dev_eui: u64,
    nwk_s_key_id: u32,
    f_cnt_up: u32,
    spreading_factor: u8,
    ttl_secs: u32,
) -> LoraCacheEntry {
    let mut data = [0u8; 17];
    data[0..8].copy_from_slice(&dev_eui.to_le_bytes());
    data[8..12].copy_from_slice(&nwk_s_key_id.to_le_bytes());
    data[12..16].copy_from_slice(&f_cnt_up.to_le_bytes());
    data[16] = spreading_factor;
    LoraCacheEntry {
        content_hash: fnv1a(&data),
        dev_eui,
        nwk_s_key_id,
        f_cnt_up,
        spreading_factor,
        ttl_secs,
    }
}

// ── Bridge 3: LoRa → Analytics (radio metrics) ──────────────────────────

/// LoRa radio metrics event for ALICE-Analytics ingestion.
pub struct LoraAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Number of uplink messages received in the observation window.
    pub uplink_count: u32,
    /// Number of downlink messages sent in the observation window.
    pub downlink_count: u32,
    /// Average SNR across uplinks, fixed-point ×10.
    pub avg_snr_x10: i16,
    /// Duty cycle utilisation in basis points (0–10000).
    pub duty_cycle_bps: u16,
    /// Observation window duration in seconds.
    pub window_secs: u32,
}

/// Convert LoRa gateway statistics into an ALICE-Analytics event.
#[inline]
#[must_use]
pub fn lora_to_analytics_event(
    uplink_count: u32,
    downlink_count: u32,
    avg_snr_x10: i16,
    duty_cycle_bps: u16,
    window_secs: u32,
) -> LoraAnalyticsEvent {
    let mut data = [0u8; 12];
    data[0..4].copy_from_slice(&uplink_count.to_le_bytes());
    data[4..8].copy_from_slice(&downlink_count.to_le_bytes());
    data[8..10].copy_from_slice(&avg_snr_x10.to_le_bytes());
    data[10..12].copy_from_slice(&duty_cycle_bps.to_le_bytes());
    LoraAnalyticsEvent {
        content_hash: fnv1a(&data),
        uplink_count,
        downlink_count,
        avg_snr_x10,
        duty_cycle_bps,
        window_secs,
    }
}

// ── Bridge 4: LoRa → Monitor (link health) ──────────────────────────────

/// LoRa link health record for ALICE-Monitor.
pub struct LoraMonitorRecord {
    /// Content hash.
    pub content_hash: u64,
    /// DevEUI (64-bit device identifier).
    pub dev_eui: u64,
    /// RSSI in dBm.
    pub rssi: i16,
    /// SNR in dB, fixed-point ×10.
    pub snr_x10: i16,
    /// Number of missed uplinks (frame counter gaps) in the window.
    pub missed_uplinks: u32,
    /// Link health status (0 = ok, 1 = degraded, 2 = critical).
    pub health_status: u8,
}

/// Build an ALICE-Monitor link health record from LoRa radio data.
#[inline]
#[must_use]
pub fn lora_to_monitor_record(
    dev_eui: u64,
    rssi: i16,
    snr_x10: i16,
    missed_uplinks: u32,
    health_status: u8,
) -> LoraMonitorRecord {
    let mut data = [0u8; 17];
    data[0..8].copy_from_slice(&dev_eui.to_le_bytes());
    data[8..10].copy_from_slice(&rssi.to_le_bytes());
    data[10..12].copy_from_slice(&snr_x10.to_le_bytes());
    data[12..16].copy_from_slice(&missed_uplinks.to_le_bytes());
    data[16] = health_status;
    LoraMonitorRecord {
        content_hash: fnv1a(&data),
        dev_eui,
        rssi,
        snr_x10,
        missed_uplinks,
        health_status,
    }
}

// ── Bridge 5: LoRa → Edge (sensor relay) ────────────────────────────────

/// LoRa sensor relay frame for ALICE-Edge forwarding.
pub struct LoraEdgeFrame {
    /// Content hash.
    pub content_hash: u64,
    /// DevEUI (64-bit device identifier).
    pub dev_eui: u64,
    /// Sensor reading value (application-specific, fixed-point ×1000).
    pub sensor_value_x1000: i32,
    /// Sensor type identifier (application-defined).
    pub sensor_type: u16,
    /// Uplink frame counter.
    pub f_cnt_up: u32,
    /// Frame timestamp as Unix timestamp (seconds).
    pub ts: u64,
}

/// Build an ALICE-Edge relay frame from a LoRa uplink sensor payload.
#[inline]
#[must_use]
pub fn lora_to_edge_frame(
    dev_eui: u64,
    sensor_value_x1000: i32,
    sensor_type: u16,
    f_cnt_up: u32,
    ts: u64,
) -> LoraEdgeFrame {
    let mut data = [0u8; 18];
    data[0..8].copy_from_slice(&dev_eui.to_le_bytes());
    data[8..12].copy_from_slice(&sensor_value_x1000.to_le_bytes());
    data[12..14].copy_from_slice(&sensor_type.to_le_bytes());
    data[14..18].copy_from_slice(&f_cnt_up.to_le_bytes());
    LoraEdgeFrame {
        content_hash: fnv1a(&data),
        dev_eui,
        sensor_value_x1000,
        sensor_type,
        f_cnt_up,
        ts,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_record_hash_is_deterministic() {
        let a = lora_to_db_record(0xDEADBEEF00000001, 9, 125, -75, -110, 12, 1_700_000_000);
        let b = lora_to_db_record(0xDEADBEEF00000001, 9, 125, -75, -110, 12, 1_700_000_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_hash_changes_on_spreading_factor() {
        let a = lora_to_db_record(0x0011223344556677, 9, 125, -75, -110, 12, 0);
        let b = lora_to_db_record(0x0011223344556677, 10, 125, -75, -110, 12, 0);
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_spreading_factor_range() {
        let r = lora_to_db_record(1, 12, 500, 50, -90, 51, 0);
        assert!(r.spreading_factor >= 7 && r.spreading_factor <= 12);
    }

    #[test]
    fn cache_entry_hash_is_deterministic() {
        let a = lora_to_cache_entry(0xAABBCCDDEEFF0011, 0xDEAD, 1024, 9, 3600);
        let b = lora_to_cache_entry(0xAABBCCDDEEFF0011, 0xDEAD, 1024, 9, 3600);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn cache_entry_frame_counter_preserved() {
        let e = lora_to_cache_entry(42, 7, 512, 10, 1800);
        assert_eq!(e.f_cnt_up, 512);
        assert_eq!(e.spreading_factor, 10);
    }

    #[test]
    fn analytics_event_duty_cycle_range() {
        let ev = lora_to_analytics_event(200, 10, -50, 500, 3600);
        assert!(ev.duty_cycle_bps <= 10_000);
        assert!(ev.uplink_count >= ev.downlink_count);
    }

    #[test]
    fn monitor_record_health_status_range() {
        let r = lora_to_monitor_record(0x001122334455, -120, -85, 5, 2);
        assert!(r.health_status <= 2);
        assert_eq!(r.missed_uplinks, 5);
    }

    #[test]
    fn edge_frame_sensor_value_round_trip() {
        let f = lora_to_edge_frame(0xFFEEDDCCBBAA9988, -1_500, 2, 99, 1_700_000_000);
        assert_eq!(f.sensor_value_x1000, -1_500);
        assert_eq!(f.f_cnt_up, 99);
        assert_eq!(f.sensor_type, 2);
    }
}
