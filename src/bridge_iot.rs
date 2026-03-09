//! IoT bridges — ALICE-IoT ↔ DB, Cache, Analytics, Edge, Monitor
//!
//! 5 bridges connecting IoT sensor data and device state to the ALICE ecosystem.

use alice_iot::{DeviceType, SensorData};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: IoT → DB (sensor record) ──────────────────────────────────

/// IoT sensor record for ALICE-DB persistence.
///
/// Stores each sensor reading with device metadata so that historical
/// trends (temperature, humidity, light) can be queried per device.
pub struct IotDbSensorRecord {
    /// FNV-1a hash of the device ID.
    pub content_hash: u64,
    /// Device type encoded as u8.
    pub device_type: u8,
    /// Temperature in milli-degrees (×1000), or 0 if absent.
    pub temperature_mdeg: i64,
    /// Humidity percentage (0–100), or -1 if absent.
    pub humidity_pct: i32,
    /// Light level raw value, or -1 if absent.
    pub light_level: i32,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build an IoT sensor record for ALICE-DB.
///
/// Temperature is stored as milli-degrees to avoid floating-point in the
/// persistence layer — integer multiply, no division.
#[inline]
#[must_use]
pub fn iot_sensor_to_db_record(
    device_id: &str,
    device_type: &DeviceType,
    sensor: &SensorData,
    timestamp_ms: u64,
) -> IotDbSensorRecord {
    let content_hash = fnv1a(device_id.as_bytes());
    let dtype = match device_type {
        DeviceType::Hub2 => 0,
        DeviceType::ColorBulb => 1,
        DeviceType::SmartLockPro => 2,
        DeviceType::KeypadTouch => 3,
        DeviceType::MotionSensor => 4,
        DeviceType::ContactSensor => 5,
        DeviceType::OutdoorCamera => 6,
        DeviceType::IrAirConditioner => 7,
        DeviceType::IrTv => 8,
        DeviceType::Unknown => 255,
    };
    let temperature_mdeg = sensor.temperature.map_or(0, |t| (t * 1000.0) as i64);
    let humidity_pct = sensor.humidity.unwrap_or(-1);
    let light_level = sensor.light_level.unwrap_or(-1);
    IotDbSensorRecord {
        content_hash,
        device_type: dtype,
        temperature_mdeg,
        humidity_pct,
        light_level,
        timestamp_ms,
    }
}

// ── Bridge 2: IoT → Cache (device state) ────────────────────────────────

/// Cached device state for ALICE-Cache.
///
/// Caches the latest sensor reading per device so that dashboard queries
/// do not hit the database on every refresh.  TTL is shortened for motion
/// and contact sensors whose state changes rapidly.
pub struct IotCacheDeviceState {
    /// FNV-1a hash of the device ID used as the cache key.
    pub content_hash: u64,
    /// Device type encoded as u8.
    pub device_type: u8,
    /// Temperature in milli-degrees (×1000), or 0 if absent.
    pub temperature_mdeg: i64,
    /// Humidity percentage (0–100), or -1 if absent.
    pub humidity_pct: i32,
    /// Time-to-live in seconds for this cache entry.
    pub ttl_seconds: u32,
}

/// Build an IoT device state cache entry with sensor-type-adjusted TTL.
///
/// TTL derivation (branchless):
/// - MotionSensor / ContactSensor → 30 s  (fast-changing state)
/// - else                         → 300 s (slow-changing sensor)
#[inline]
#[must_use]
pub fn iot_sensor_to_cache_state(
    device_id: &str,
    device_type: &DeviceType,
    sensor: &SensorData,
) -> IotCacheDeviceState {
    let content_hash = fnv1a(device_id.as_bytes());
    let dtype = match device_type {
        DeviceType::Hub2 => 0,
        DeviceType::ColorBulb => 1,
        DeviceType::SmartLockPro => 2,
        DeviceType::KeypadTouch => 3,
        DeviceType::MotionSensor => 4,
        DeviceType::ContactSensor => 5,
        DeviceType::OutdoorCamera => 6,
        DeviceType::IrAirConditioner => 7,
        DeviceType::IrTv => 8,
        DeviceType::Unknown => 255,
    };
    let is_fast_sensor = matches!(
        device_type,
        DeviceType::MotionSensor | DeviceType::ContactSensor
    ) as u32;
    // Branchless TTL: fast=30, slow=300.
    let ttl_seconds = 300 - is_fast_sensor * 270;
    let temperature_mdeg = sensor.temperature.map_or(0, |t| (t * 1000.0) as i64);
    let humidity_pct = sensor.humidity.unwrap_or(-1);
    IotCacheDeviceState {
        content_hash,
        device_type: dtype,
        temperature_mdeg,
        humidity_pct,
        ttl_seconds,
    }
}

// ── Bridge 3: IoT → Analytics (telemetry event) ─────────────────────────

/// Telemetry event for ALICE-Analytics ingestion.
///
/// Aggregates sensor readings into a single analytics event so that
/// dashboards can chart temperature/humidity trends per device type.
pub struct IotAnalyticsTelemetryEvent {
    /// FNV-1a hash of the device ID.
    pub content_hash: u64,
    /// Device type encoded as u8.
    pub device_type: u8,
    /// Temperature in milli-degrees (×1000).
    pub temperature_mdeg: i64,
    /// Humidity percentage.
    pub humidity_pct: i32,
    /// Illuminance raw value, or -1 if absent.
    pub illuminance: i32,
    /// Whether any sensor value is out of normal range.
    pub anomaly: bool,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build an IoT telemetry event for ALICE-Analytics.
///
/// `anomaly` is set branchlessly: temperature outside [-10°C, 50°C] or
/// humidity outside [0, 100] triggers the flag.
#[inline]
#[must_use]
pub fn iot_sensor_to_analytics_event(
    device_id: &str,
    device_type: &DeviceType,
    sensor: &SensorData,
    timestamp_ms: u64,
) -> IotAnalyticsTelemetryEvent {
    let content_hash = fnv1a(device_id.as_bytes());
    let dtype = match device_type {
        DeviceType::Hub2 => 0,
        DeviceType::ColorBulb => 1,
        DeviceType::SmartLockPro => 2,
        DeviceType::KeypadTouch => 3,
        DeviceType::MotionSensor => 4,
        DeviceType::ContactSensor => 5,
        DeviceType::OutdoorCamera => 6,
        DeviceType::IrAirConditioner => 7,
        DeviceType::IrTv => 8,
        DeviceType::Unknown => 255,
    };
    let temperature_mdeg = sensor.temperature.map_or(0, |t| (t * 1000.0) as i64);
    let humidity_pct = sensor.humidity.unwrap_or(-1);
    let illuminance = sensor.illuminance.unwrap_or(-1);
    // Branchless anomaly: temp outside [-10000, 50000] mdeg or humidity outside [0, 100].
    let temp_anomaly = !(-10_000..=50_000).contains(&temperature_mdeg) as u8;
    let hum_anomaly = !(0..=100).contains(&humidity_pct) as u8;
    let anomaly = (temp_anomaly | hum_anomaly) != 0;
    IotAnalyticsTelemetryEvent {
        content_hash,
        device_type: dtype,
        temperature_mdeg,
        humidity_pct,
        illuminance,
        anomaly,
        timestamp_ms,
    }
}

// ── Bridge 4: IoT → Edge (compact payload) ──────────────────────────────

/// Compact IoT payload for ALICE-Edge transmission.
///
/// Minimises wire size for edge-to-cloud sensor telemetry by packing
/// only essential fields into a fixed-size struct (no heap allocation).
pub struct IotEdgePayload {
    /// FNV-1a hash of the device ID.
    pub content_hash: u64,
    /// Device type encoded as u8.
    pub device_type: u8,
    /// Temperature in milli-degrees (×1000).
    pub temperature_mdeg: i64,
    /// Humidity percentage.
    pub humidity_pct: i32,
    /// Payload size estimate in bytes.
    pub payload_bytes: usize,
}

/// Build a compact IoT payload for ALICE-Edge.
///
/// `payload_bytes` = 32 (fixed header) + 8 per present sensor field —
/// integer multiply, no division.
#[inline]
#[must_use]
pub fn iot_sensor_to_edge_payload(
    device_id: &str,
    device_type: &DeviceType,
    sensor: &SensorData,
) -> IotEdgePayload {
    let content_hash = fnv1a(device_id.as_bytes());
    let dtype = match device_type {
        DeviceType::Hub2 => 0,
        DeviceType::ColorBulb => 1,
        DeviceType::SmartLockPro => 2,
        DeviceType::KeypadTouch => 3,
        DeviceType::MotionSensor => 4,
        DeviceType::ContactSensor => 5,
        DeviceType::OutdoorCamera => 6,
        DeviceType::IrAirConditioner => 7,
        DeviceType::IrTv => 8,
        DeviceType::Unknown => 255,
    };
    let temperature_mdeg = sensor.temperature.map_or(0, |t| (t * 1000.0) as i64);
    let humidity_pct = sensor.humidity.unwrap_or(-1);
    let field_count = sensor.temperature.is_some() as usize
        + sensor.humidity.is_some() as usize
        + sensor.light_level.is_some() as usize
        + sensor.illuminance.is_some() as usize;
    let payload_bytes = 32 + field_count * 8;
    IotEdgePayload {
        content_hash,
        device_type: dtype,
        temperature_mdeg,
        humidity_pct,
        payload_bytes,
    }
}

// ── Bridge 5: IoT → Monitor (alert) ─────────────────────────────────────

/// IoT alert record for ALICE-Monitor.
///
/// Emitted when sensor readings exceed safe thresholds so that operators
/// can respond to environmental anomalies (e.g. freezing pipes, overheating).
///
/// `severity`: 0 = info, 1 = warning, 2 = critical.
pub struct IotMonitorAlert {
    /// FNV-1a hash of the device ID.
    pub content_hash: u64,
    /// Device type encoded as u8.
    pub device_type: u8,
    /// Assessed severity (0=info, 1=warning, 2=critical).
    pub severity: u8,
    /// Temperature in milli-degrees (×1000).
    pub temperature_mdeg: i64,
    /// Humidity percentage.
    pub humidity_pct: i32,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build an IoT alert for ALICE-Monitor.
///
/// Severity derivation (branchless):
/// - temp > 45°C or temp < -5°C → critical (2)
/// - temp > 35°C or temp < 0°C  → warning (1)
/// - else                        → info (0)
///
/// Returns `None` for severity == 0 (info) to avoid alert noise.
#[inline]
#[must_use]
pub fn iot_sensor_to_monitor_alert(
    device_id: &str,
    device_type: &DeviceType,
    sensor: &SensorData,
    timestamp_ms: u64,
) -> Option<IotMonitorAlert> {
    let temperature_mdeg = sensor.temperature.map_or(0, |t| (t * 1000.0) as i64);
    let humidity_pct = sensor.humidity.unwrap_or(-1);
    let is_critical = !(-5_000..=45_000).contains(&temperature_mdeg) as u8;
    let is_warning = !(0..=35_000).contains(&temperature_mdeg) as u8;
    let severity = (is_critical * 2).max(is_warning);
    if severity == 0 {
        return None;
    }
    let content_hash = fnv1a(device_id.as_bytes());
    let dtype = match device_type {
        DeviceType::Hub2 => 0,
        DeviceType::ColorBulb => 1,
        DeviceType::SmartLockPro => 2,
        DeviceType::KeypadTouch => 3,
        DeviceType::MotionSensor => 4,
        DeviceType::ContactSensor => 5,
        DeviceType::OutdoorCamera => 6,
        DeviceType::IrAirConditioner => 7,
        DeviceType::IrTv => 8,
        DeviceType::Unknown => 255,
    };
    Some(IotMonitorAlert {
        content_hash,
        device_type: dtype,
        severity,
        temperature_mdeg,
        humidity_pct,
        timestamp_ms,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sensor() -> SensorData {
        SensorData {
            temperature: Some(23.5),
            humidity: Some(55),
            light_level: Some(3),
            illuminance: Some(200),
        }
    }

    #[test]
    fn test_iot_db_record_basic() {
        let s = sample_sensor();
        let rec = iot_sensor_to_db_record("C6D649CCC251", &DeviceType::Hub2, &s, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.device_type, 0);
        assert_eq!(rec.temperature_mdeg, 23_500);
        assert_eq!(rec.humidity_pct, 55);
        assert_eq!(rec.light_level, 3);
    }

    #[test]
    fn test_iot_db_record_absent_sensors() {
        let s = SensorData {
            temperature: None,
            humidity: None,
            light_level: None,
            illuminance: None,
        };
        let rec = iot_sensor_to_db_record("DEV001", &DeviceType::MotionSensor, &s, 0);
        assert_eq!(rec.temperature_mdeg, 0);
        assert_eq!(rec.humidity_pct, -1);
        assert_eq!(rec.light_level, -1);
        assert_eq!(rec.device_type, 4);
    }

    #[test]
    fn test_iot_cache_state_fast_sensor_ttl() {
        let s = sample_sensor();
        let state = iot_sensor_to_cache_state("MOTION01", &DeviceType::MotionSensor, &s);
        assert_eq!(state.ttl_seconds, 30, "MotionSensor should have 30s TTL");
        let state2 = iot_sensor_to_cache_state("CONTACT01", &DeviceType::ContactSensor, &s);
        assert_eq!(state2.ttl_seconds, 30, "ContactSensor should have 30s TTL");
    }

    #[test]
    fn test_iot_cache_state_slow_sensor_ttl() {
        let s = sample_sensor();
        let state = iot_sensor_to_cache_state("HUB01", &DeviceType::Hub2, &s);
        assert_eq!(state.ttl_seconds, 300, "Hub2 should have 300s TTL");
    }

    #[test]
    fn test_iot_analytics_anomaly_normal() {
        let s = sample_sensor(); // 23.5°C, 55% — normal
        let ev = iot_sensor_to_analytics_event("DEV01", &DeviceType::Hub2, &s, 0);
        assert!(!ev.anomaly, "normal readings should not flag anomaly");
        assert_eq!(ev.illuminance, 200);
    }

    #[test]
    fn test_iot_analytics_anomaly_extreme_temp() {
        let s = SensorData {
            temperature: Some(55.0), // 55°C — above 50°C threshold
            humidity: Some(50),
            light_level: None,
            illuminance: None,
        };
        let ev = iot_sensor_to_analytics_event("DEV02", &DeviceType::Hub2, &s, 0);
        assert!(ev.anomaly, "55°C should flag anomaly");
    }

    #[test]
    fn test_iot_edge_payload_size() {
        let s = sample_sensor(); // 4 fields present
        let p = iot_sensor_to_edge_payload("DEV01", &DeviceType::Hub2, &s);
        assert_eq!(p.payload_bytes, 32 + 4 * 8); // 64 bytes
        assert_ne!(p.content_hash, 0);

        // 0 fields present
        let empty = SensorData {
            temperature: None,
            humidity: None,
            light_level: None,
            illuminance: None,
        };
        let p2 = iot_sensor_to_edge_payload("DEV01", &DeviceType::Hub2, &empty);
        assert_eq!(p2.payload_bytes, 32); // header only
    }

    #[test]
    fn test_iot_monitor_alert_info_returns_none() {
        let s = sample_sensor(); // 23.5°C — normal
        let r = iot_sensor_to_monitor_alert("DEV01", &DeviceType::Hub2, &s, 0);
        assert!(r.is_none(), "normal temp should not produce an alert");
    }

    #[test]
    fn test_iot_monitor_alert_critical() {
        let s = SensorData {
            temperature: Some(50.0), // 50°C — above 45°C critical threshold
            humidity: Some(30),
            light_level: None,
            illuminance: None,
        };
        let alert = iot_sensor_to_monitor_alert("DEV03", &DeviceType::Hub2, &s, 1_700_000_000_000)
            .expect("50°C should produce a critical alert");
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.temperature_mdeg, 50_000);
    }

    #[test]
    fn test_iot_monitor_alert_warning() {
        let s = SensorData {
            temperature: Some(40.0), // 40°C — above 35°C warning, below 45°C critical
            humidity: Some(20),
            light_level: None,
            illuminance: None,
        };
        let alert = iot_sensor_to_monitor_alert("DEV04", &DeviceType::Hub2, &s, 0)
            .expect("40°C should produce a warning alert");
        assert_eq!(alert.severity, 1);
    }

    #[test]
    fn test_iot_hash_determinism() {
        let s = sample_sensor();
        let r1 = iot_sensor_to_db_record("SAME_ID", &DeviceType::Hub2, &s, 0);
        let r2 = iot_sensor_to_db_record("SAME_ID", &DeviceType::Hub2, &s, 0);
        assert_eq!(
            r1.content_hash, r2.content_hash,
            "same input must produce same hash"
        );
        assert_ne!(r1.content_hash, 0);
    }
}
