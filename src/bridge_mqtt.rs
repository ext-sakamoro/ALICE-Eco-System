//! MQTT bridges — MQTT ↔ DB, Cache, Analytics, Edge, Monitor
//!
//! 5 bridges connecting MQTT broker telemetry to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: MQTT → DB (topic message persistence) ──────────────────────

/// MQTT topic record for ALICE-DB persistence.
pub struct MqttDbRecord {
    /// Content hash over topic + broker + timestamp.
    pub content_hash: u64,
    /// FNV-1a hash of the topic string.
    pub topic_hash: u64,
    /// Total message count published to this topic.
    pub msg_count: u64,
    /// QoS level: 0=at_most_once, 1=at_least_once, 2=exactly_once.
    pub qos: u8,
    /// Number of retained messages on this topic.
    pub retained_count: u32,
    /// FNV-1a hash of the broker identifier.
    pub broker_hash: u64,
}

/// Serialize an MQTT topic record for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn mqtt_to_db_record(
    topic_hash: u64,
    msg_count: u64,
    qos: u8,
    retained_count: u32,
    broker_hash: u64,
) -> MqttDbRecord {
    let mut key = [0u8; 29];
    key[0..8].copy_from_slice(&topic_hash.to_le_bytes());
    key[8..16].copy_from_slice(&msg_count.to_le_bytes());
    key[16] = qos;
    key[17..21].copy_from_slice(&retained_count.to_le_bytes());
    key[21..29].copy_from_slice(&broker_hash.to_le_bytes());
    MqttDbRecord {
        content_hash: fnv1a(&key),
        topic_hash,
        msg_count,
        qos,
        retained_count,
        broker_hash,
    }
}

// ── Bridge 2: MQTT → Cache (topic payload caching) ───────────────────────

/// MQTT topic cache entry for ALICE-Cache.
pub struct MqttCacheEntry {
    /// Content hash over topic + payload + qos.
    pub content_hash: u64,
    /// FNV-1a hash of the topic string.
    pub topic_hash: u64,
    /// Cache TTL in seconds (shorter for QoS-0 topics).
    pub ttl_secs: u32,
    /// Payload size in bytes.
    pub payload_bytes: u64,
    /// QoS level for this cache entry.
    pub qos: u8,
}

/// Build an MQTT topic cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 10 s for QoS-0 (fire-and-forget) topics.
#[inline]
#[must_use]
pub fn mqtt_to_cache_entry(topic_hash: u64, payload_bytes: u64, qos: u8) -> MqttCacheEntry {
    // Branchless QoS-0 TTL: 300 s normal, 10 s for qos == 0.
    let low_qos = (qos == 0) as u32;
    let ttl_secs = 300_u32 - low_qos * 290_u32;
    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&topic_hash.to_le_bytes());
    key[8..16].copy_from_slice(&payload_bytes.to_le_bytes());
    key[16] = qos;
    MqttCacheEntry {
        content_hash: fnv1a(&key),
        topic_hash,
        ttl_secs,
        payload_bytes,
        qos,
    }
}

// ── Bridge 3: MQTT → Analytics (publish/subscribe metrics) ───────────────

/// MQTT publish/subscribe analytics event for ALICE-Analytics.
pub struct MqttAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total messages published in the reporting window.
    pub msg_count: u64,
    /// Publish rate in messages per second (fixed-point × 1000).
    pub publish_rate: u64,
    /// Number of active subscribers.
    pub subscribe_count: u32,
    /// Average end-to-end latency in microseconds.
    pub avg_latency_us: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an MQTT analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn mqtt_to_analytics_event(
    msg_count: u64,
    publish_rate: u64,
    subscribe_count: u32,
    avg_latency_us: u64,
    timestamp_ms: u64,
) -> MqttAnalyticsEvent {
    let mut key = [0u8; 36];
    key[0..8].copy_from_slice(&msg_count.to_le_bytes());
    key[8..16].copy_from_slice(&publish_rate.to_le_bytes());
    key[16..20].copy_from_slice(&subscribe_count.to_le_bytes());
    key[20..28].copy_from_slice(&avg_latency_us.to_le_bytes());
    key[28..36].copy_from_slice(&timestamp_ms.to_le_bytes());
    MqttAnalyticsEvent {
        content_hash: fnv1a(&key),
        msg_count,
        publish_rate,
        subscribe_count,
        avg_latency_us,
        timestamp_ms,
    }
}

// ── Bridge 4: MQTT → Edge (edge device telemetry) ────────────────────────

/// MQTT edge telemetry record for ALICE-Edge.
pub struct MqttEdgeTelemetry {
    /// Content hash over topic + counts.
    pub content_hash: u64,
    /// FNV-1a hash of the topic string.
    pub topic_hash: u64,
    /// Total messages processed at the edge.
    pub msg_count: u64,
    /// Total bytes transferred.
    pub byte_count: u64,
    /// Number of active connections to the edge broker.
    pub connection_count: u32,
}

/// Build an MQTT edge telemetry record for ALICE-Edge.
#[inline]
#[must_use]
pub fn mqtt_to_edge_telemetry(
    topic_hash: u64,
    msg_count: u64,
    byte_count: u64,
    connection_count: u32,
) -> MqttEdgeTelemetry {
    let mut key = [0u8; 28];
    key[0..8].copy_from_slice(&topic_hash.to_le_bytes());
    key[8..16].copy_from_slice(&msg_count.to_le_bytes());
    key[16..24].copy_from_slice(&byte_count.to_le_bytes());
    key[24..28].copy_from_slice(&connection_count.to_le_bytes());
    MqttEdgeTelemetry {
        content_hash: fnv1a(&key),
        topic_hash,
        msg_count,
        byte_count,
        connection_count,
    }
}

// ── Bridge 5: MQTT → Monitor (broker health status) ──────────────────────

/// MQTT broker health status for ALICE-Monitor.
pub struct MqttMonitorStatus {
    /// Content hash over broker + metrics.
    pub content_hash: u64,
    /// FNV-1a hash of the broker identifier.
    pub broker_hash: u64,
    /// Number of currently connected clients.
    pub connected_clients: u32,
    /// Current message rate in messages per second (fixed-point × 1000).
    pub msg_rate: u64,
    /// Whether the broker is considered healthy.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an MQTT broker health status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn mqtt_to_monitor_status(
    broker_hash: u64,
    connected_clients: u32,
    msg_rate: u64,
    is_healthy: bool,
    timestamp_ms: u64,
) -> MqttMonitorStatus {
    let mut key = [0u8; 21];
    key[0..8].copy_from_slice(&broker_hash.to_le_bytes());
    key[8..12].copy_from_slice(&connected_clients.to_le_bytes());
    key[12..20].copy_from_slice(&msg_rate.to_le_bytes());
    key[20] = is_healthy as u8;
    MqttMonitorStatus {
        content_hash: fnv1a(&key),
        broker_hash,
        connected_clients,
        msg_rate,
        is_healthy,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const TOPIC_HASH: u64 = 0xABCD_1234_5678_EF90;
    const BROKER_HASH: u64 = 0xFEDC_BA98_7654_3210;

    #[test]
    fn test_mqtt_to_db_record_hash_nonzero() {
        let rec = mqtt_to_db_record(TOPIC_HASH, 1_000, 1, 5, BROKER_HASH);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_mqtt_to_db_record_fields() {
        let rec = mqtt_to_db_record(TOPIC_HASH, 500, 2, 3, BROKER_HASH);
        assert_eq!(rec.topic_hash, TOPIC_HASH);
        assert_eq!(rec.msg_count, 500);
        assert_eq!(rec.qos, 2);
        assert_eq!(rec.retained_count, 3);
        assert_eq!(rec.broker_hash, BROKER_HASH);
    }

    #[test]
    fn test_mqtt_to_cache_entry_normal_ttl() {
        let entry = mqtt_to_cache_entry(TOPIC_HASH, 256, 1);
        assert_eq!(entry.ttl_secs, 300);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_mqtt_to_cache_entry_qos0_ttl() {
        // QoS-0 → reduced TTL = 10 s.
        let entry = mqtt_to_cache_entry(TOPIC_HASH, 256, 0);
        assert_eq!(entry.ttl_secs, 10);
    }

    #[test]
    fn test_mqtt_to_analytics_event_fields() {
        let ev = mqtt_to_analytics_event(10_000, 5_000_000, 200, 1_500, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.msg_count, 10_000);
        assert_eq!(ev.subscribe_count, 200);
        assert_eq!(ev.avg_latency_us, 1_500);
    }

    #[test]
    fn test_mqtt_to_analytics_event_determinism() {
        let a = mqtt_to_analytics_event(1, 2, 3, 4, 5);
        let b = mqtt_to_analytics_event(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_mqtt_to_edge_telemetry_fields() {
        let tel = mqtt_to_edge_telemetry(TOPIC_HASH, 800, 204_800, 12);
        assert_ne!(tel.content_hash, 0);
        assert_eq!(tel.topic_hash, TOPIC_HASH);
        assert_eq!(tel.msg_count, 800);
        assert_eq!(tel.byte_count, 204_800);
        assert_eq!(tel.connection_count, 12);
    }

    #[test]
    fn test_mqtt_to_monitor_status_healthy() {
        let status = mqtt_to_monitor_status(BROKER_HASH, 150, 3_000_000, true, 1_700_000_000_000);
        assert_ne!(status.content_hash, 0);
        assert_eq!(status.broker_hash, BROKER_HASH);
        assert_eq!(status.connected_clients, 150);
        assert!(status.is_healthy);
    }

    #[test]
    fn test_mqtt_to_monitor_status_unhealthy() {
        let status = mqtt_to_monitor_status(BROKER_HASH, 0, 0, false, 1_700_000_000_000);
        assert!(!status.is_healthy);
        assert_ne!(status.content_hash, 0);
    }
}
