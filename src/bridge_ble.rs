//! BLE bridges — ALICE-BLE ↔ DB, Cache, Analytics, Monitor, Edge
//!
//! 5 bridges connecting Bluetooth Low Energy device management to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: BLE → DB (device log) ─────────────────────────────────────

/// BLE device log record for ALICE-DB persistence.
pub struct BleDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// 48-bit Bluetooth device address packed into a u64 (upper 16 bits zero).
    pub device_addr: u64,
    /// Number of BLE devices visible to the local adapter.
    pub device_count: u32,
    /// Received signal strength indicator in dBm (signed, stored as i8 cast to u8).
    pub rssi_raw: u8,
    /// Connection interval in units of 1.25 ms (BLE spec units).
    pub connection_interval_units: u16,
    /// Maximum transmission unit in bytes.
    pub mtu: u16,
    /// Unix timestamp of the log entry (seconds).
    pub logged_at_ts: u64,
}

/// Convert BLE adapter state into an ALICE-DB device log record.
#[inline]
#[must_use]
pub fn ble_to_db_record(
    device_addr: u64,
    device_count: u32,
    rssi_dbm: i8,
    connection_interval_units: u16,
    mtu: u16,
    logged_at_ts: u64,
) -> BleDbRecord {
    let rssi_raw = rssi_dbm as u8;
    let mut data = [0u8; 25];
    data[0..8].copy_from_slice(&device_addr.to_le_bytes());
    data[8..12].copy_from_slice(&device_count.to_le_bytes());
    data[12] = rssi_raw;
    data[13..15].copy_from_slice(&connection_interval_units.to_le_bytes());
    data[15..17].copy_from_slice(&mtu.to_le_bytes());
    data[17..25].copy_from_slice(&logged_at_ts.to_le_bytes());
    BleDbRecord {
        content_hash: fnv1a(&data),
        device_addr,
        device_count,
        rssi_raw,
        connection_interval_units,
        mtu,
        logged_at_ts,
    }
}

// ── Bridge 2: BLE → Cache (connection cache) ────────────────────────────

/// BLE connection cache entry for ALICE-Cache.
pub struct BleCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// 48-bit Bluetooth device address packed into a u64.
    pub device_addr: u64,
    /// Number of active GATT services on this connection.
    pub service_count: u8,
    /// Number of discoverable GATT characteristics.
    pub characteristic_count: u16,
    /// Current connection interval in milliseconds.
    pub connection_interval_ms: u16,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build an ALICE-Cache connection entry from BLE GATT state.
#[inline]
#[must_use]
pub fn ble_to_cache_entry(
    device_addr: u64,
    service_count: u8,
    characteristic_count: u16,
    connection_interval_ms: u16,
    ttl_secs: u32,
) -> BleCacheEntry {
    let mut data = [0u8; 13];
    data[0..8].copy_from_slice(&device_addr.to_le_bytes());
    data[8] = service_count;
    data[9..11].copy_from_slice(&characteristic_count.to_le_bytes());
    data[11..13].copy_from_slice(&connection_interval_ms.to_le_bytes());
    BleCacheEntry {
        content_hash: fnv1a(&data),
        device_addr,
        service_count,
        characteristic_count,
        connection_interval_ms,
        ttl_secs,
    }
}

// ── Bridge 3: BLE → Analytics (connection metrics) ──────────────────────

/// BLE connection metrics event for ALICE-Analytics ingestion.
pub struct BleAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Number of successful connections in the observation window.
    pub connect_count: u32,
    /// Number of dropped connections in the observation window.
    pub disconnect_count: u32,
    /// Average RSSI across all connections in dBm (signed, stored as i8 cast to u8).
    pub avg_rssi_raw: u8,
    /// Average MTU negotiated across connections in bytes.
    pub avg_mtu: u16,
    /// Observation window duration in seconds.
    pub window_secs: u32,
}

/// Convert BLE connection statistics into an ALICE-Analytics event.
#[inline]
#[must_use]
pub fn ble_to_analytics_event(
    connect_count: u32,
    disconnect_count: u32,
    avg_rssi_dbm: i8,
    avg_mtu: u16,
    window_secs: u32,
) -> BleAnalyticsEvent {
    let avg_rssi_raw = avg_rssi_dbm as u8;
    let mut data = [0u8; 11];
    data[0..4].copy_from_slice(&connect_count.to_le_bytes());
    data[4..8].copy_from_slice(&disconnect_count.to_le_bytes());
    data[8] = avg_rssi_raw;
    data[9..11].copy_from_slice(&avg_mtu.to_le_bytes());
    BleAnalyticsEvent {
        content_hash: fnv1a(&data),
        connect_count,
        disconnect_count,
        avg_rssi_raw,
        avg_mtu,
        window_secs,
    }
}

// ── Bridge 4: BLE → Monitor (signal health) ─────────────────────────────

/// BLE signal health record for ALICE-Monitor.
pub struct BleMonitorRecord {
    /// Content hash.
    pub content_hash: u64,
    /// 48-bit Bluetooth device address packed into a u64.
    pub device_addr: u64,
    /// RSSI in dBm (signed, stored as i8 cast to u8).
    pub rssi_raw: u8,
    /// Link quality indicator (0–255, higher is better).
    pub link_quality: u8,
    /// Number of retransmissions in the last interval.
    pub retransmit_count: u32,
    /// Health status (0 = ok, 1 = degraded, 2 = critical).
    pub health_status: u8,
}

/// Build an ALICE-Monitor signal health record from BLE radio data.
#[inline]
#[must_use]
pub fn ble_to_monitor_record(
    device_addr: u64,
    rssi_dbm: i8,
    link_quality: u8,
    retransmit_count: u32,
    health_status: u8,
) -> BleMonitorRecord {
    let rssi_raw = rssi_dbm as u8;
    let mut data = [0u8; 14];
    data[0..8].copy_from_slice(&device_addr.to_le_bytes());
    data[8] = rssi_raw;
    data[9] = link_quality;
    data[10..14].copy_from_slice(&retransmit_count.to_le_bytes());
    BleMonitorRecord {
        content_hash: fnv1a(&data),
        device_addr,
        rssi_raw,
        link_quality,
        retransmit_count,
        health_status,
    }
}

// ── Bridge 5: BLE → Edge (sensor relay) ─────────────────────────────────

/// BLE sensor relay frame for ALICE-Edge forwarding.
pub struct BleEdgeFrame {
    /// Content hash.
    pub content_hash: u64,
    /// 48-bit Bluetooth device address packed into a u64.
    pub device_addr: u64,
    /// Sensor reading value (application-specific, fixed-point ×1000).
    pub sensor_value_x1000: i32,
    /// Sensor type identifier (application-defined).
    pub sensor_type: u16,
    /// Frame sequence number.
    pub seq: u32,
    /// Frame timestamp as Unix timestamp (seconds).
    pub ts: u64,
}

/// Build an ALICE-Edge relay frame from a BLE sensor notification.
#[inline]
#[must_use]
pub fn ble_to_edge_frame(
    device_addr: u64,
    sensor_value_x1000: i32,
    sensor_type: u16,
    seq: u32,
    ts: u64,
) -> BleEdgeFrame {
    let mut data = [0u8; 18];
    data[0..8].copy_from_slice(&device_addr.to_le_bytes());
    data[8..12].copy_from_slice(&sensor_value_x1000.to_le_bytes());
    data[12..14].copy_from_slice(&sensor_type.to_le_bytes());
    data[14..18].copy_from_slice(&seq.to_le_bytes());
    BleEdgeFrame {
        content_hash: fnv1a(&data),
        device_addr,
        sensor_value_x1000,
        sensor_type,
        seq,
        ts,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_record_hash_is_deterministic() {
        let a = ble_to_db_record(0xAABBCC112233, 5, -70, 24, 247, 1_700_000_000);
        let b = ble_to_db_record(0xAABBCC112233, 5, -70, 24, 247, 1_700_000_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_hash_changes_on_device_count() {
        let a = ble_to_db_record(0xAABBCC112233, 5, -70, 24, 247, 1_700_000_000);
        let b = ble_to_db_record(0xAABBCC112233, 6, -70, 24, 247, 1_700_000_000);
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_rssi_round_trip() {
        let r = ble_to_db_record(0x001122334455, 3, -85, 16, 185, 0);
        assert_eq!(r.rssi_raw as i8, -85i8);
    }

    #[test]
    fn cache_entry_hash_is_deterministic() {
        let a = ble_to_cache_entry(0x001122334455, 3, 12, 50, 300);
        let b = ble_to_cache_entry(0x001122334455, 3, 12, 50, 300);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn cache_entry_fields_preserved() {
        let e = ble_to_cache_entry(0xDEADBEEF0000, 4, 20, 75, 600);
        assert_eq!(e.service_count, 4);
        assert_eq!(e.characteristic_count, 20);
        assert_eq!(e.connection_interval_ms, 75);
    }

    #[test]
    fn analytics_event_disconnect_lte_connect() {
        let ev = ble_to_analytics_event(100, 15, -65, 247, 60);
        assert!(ev.disconnect_count <= ev.connect_count);
    }

    #[test]
    fn monitor_record_health_status_range() {
        let r = ble_to_monitor_record(0xAABBCC001122, -90, 40, 10, 2);
        assert!(r.health_status <= 2);
        assert_eq!(r.link_quality, 40);
    }

    #[test]
    fn edge_frame_seq_preserved() {
        let f = ble_to_edge_frame(0x112233445566, 23_500, 1, 42, 1_700_000_000);
        assert_eq!(f.seq, 42);
        assert_eq!(f.sensor_value_x1000, 23_500);
    }
}
