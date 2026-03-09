//! OTA bridges — ALICE-OTA ↔ DB, Cache, Analytics, Edge, Notify
//!
//! 5 bridges connecting over-the-air firmware update data (extracted as
//! primitives) to the ALICE ecosystem. No external crate types are imported;
//! all fields use primitive types derived from serialised OTA state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: OTA → DB (firmware campaign persistence) ────────────────────

/// OTA firmware campaign record for ALICE-DB persistence.
pub struct OtaDbRecord {
    /// Content hash over firmware_hash, device_count, and timestamp_ms.
    pub content_hash: u64,
    /// Hash of the firmware image being distributed.
    pub firmware_hash: u64,
    /// Number of target devices in this campaign.
    pub device_count: u32,
    /// Firmware version number.
    pub version: u32,
    /// Size of the firmware image in bytes.
    pub image_bytes: u64,
    /// Unix timestamp in milliseconds when the campaign was created.
    pub timestamp_ms: u64,
}

/// Build a DB persistence record from extracted OTA campaign data.
#[inline]
#[must_use]
pub fn ota_to_db_record(
    firmware_id: &[u8],
    device_count: u32,
    version: u32,
    image_bytes: u64,
    timestamp_ms: u64,
) -> OtaDbRecord {
    let firmware_hash = fnv1a(firmware_id);
    let mut buf = [0u8; 20];
    buf[0..8].copy_from_slice(&firmware_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&device_count.to_le_bytes());
    buf[12..20].copy_from_slice(&timestamp_ms.to_le_bytes());
    OtaDbRecord {
        content_hash: fnv1a(&buf),
        firmware_hash,
        device_count,
        version,
        image_bytes,
        timestamp_ms,
    }
}

// ── Bridge 2: OTA → Cache (firmware image caching) ────────────────────────

/// Cached OTA firmware image entry for ALICE-Cache.
pub struct OtaCacheEntry {
    /// Content hash over firmware_hash and version.
    pub content_hash: u64,
    /// Hash of the cached firmware image used as cache key.
    pub firmware_hash: u64,
    /// TTL in seconds for this cache entry.
    pub ttl_secs: u32,
    /// Firmware image size in bytes.
    pub image_bytes: u64,
    /// Firmware version number.
    pub version: u32,
}

/// Build a cache entry for an OTA firmware image.
///
/// TTL is 3600 s by default; reduced to 300 s when `image_bytes` exceeds
/// 32 MB to limit memory pressure from large firmware images.
#[inline]
#[must_use]
pub fn ota_to_cache_entry(firmware_id: &[u8], image_bytes: u64, version: u32) -> OtaCacheEntry {
    let firmware_hash = fnv1a(firmware_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&firmware_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&version.to_le_bytes());
    let large = (image_bytes > 32_000_000) as u32;
    let ttl_secs = 3_600 - large * 3_300;
    OtaCacheEntry {
        content_hash: fnv1a(&buf),
        firmware_hash,
        ttl_secs,
        image_bytes,
        version,
    }
}

// ── Bridge 3: OTA → Analytics (campaign metrics ingestion) ────────────────

/// OTA campaign metrics event for ALICE-Analytics ingestion.
pub struct OtaAnalyticsEvent {
    /// Content hash over firmware_hash and timestamp_ms.
    pub content_hash: u64,
    /// Total update attempts in the campaign so far.
    pub update_count: u64,
    /// Number of devices that successfully applied the update.
    pub success_count: u64,
    /// Number of devices that failed to apply the update.
    pub failure_count: u32,
    /// Average update duration in milliseconds across all devices.
    pub avg_duration_ms: u64,
    /// Unix timestamp in milliseconds when the event was recorded.
    pub timestamp_ms: u64,
}

/// Build an analytics ingestion event from OTA campaign metrics.
#[inline]
#[must_use]
pub fn ota_to_analytics_event(
    firmware_id: &[u8],
    update_count: u64,
    success_count: u64,
    failure_count: u32,
    avg_duration_ms: u64,
    timestamp_ms: u64,
) -> OtaAnalyticsEvent {
    let firmware_hash = fnv1a(firmware_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&firmware_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    OtaAnalyticsEvent {
        content_hash: fnv1a(&buf),
        update_count,
        success_count,
        failure_count,
        avg_duration_ms,
        timestamp_ms,
    }
}

// ── Bridge 4: OTA → Edge (device update progress telemetry) ───────────────

/// OTA device update progress telemetry for ALICE-Edge.
pub struct OtaEdgeTelemetry {
    /// Content hash over device_hash and target_version.
    pub content_hash: u64,
    /// Hashed device identifier.
    pub device_hash: u64,
    /// Currently installed firmware version on the device.
    pub current_version: u32,
    /// Target firmware version being applied.
    pub target_version: u32,
    /// Download and flash progress as a percentage (0–100).
    pub progress_pct: u8,
}

/// Build an edge telemetry packet for a device's OTA update progress.
#[inline]
#[must_use]
pub fn ota_to_edge_telemetry(
    device_id: &[u8],
    current_version: u32,
    target_version: u32,
    progress_pct: u8,
) -> OtaEdgeTelemetry {
    let device_hash = fnv1a(device_id);
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&device_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&target_version.to_le_bytes());
    OtaEdgeTelemetry {
        content_hash: fnv1a(&buf),
        device_hash,
        current_version,
        target_version,
        progress_pct,
    }
}

// ── Bridge 5: OTA → Notify (failure alert) ────────────────────────────────

/// OTA failure alert for ALICE-Notify dispatch.
pub struct OtaNotifyAlert {
    /// Content hash over firmware_hash and timestamp_ms.
    pub content_hash: u64,
    /// Alert severity level (0 = info, 1 = warn, 2 = error, 3 = critical).
    pub severity: u8,
    /// Hash of the firmware image that triggered failures.
    pub firmware_hash: u64,
    /// Number of device failures that triggered this alert.
    pub failure_count: u32,
    /// Unix timestamp in milliseconds when the alert was raised.
    pub timestamp_ms: u64,
}

/// Build a notify alert from OTA campaign failure data.
#[inline]
#[must_use]
pub fn ota_to_notify_alert(
    firmware_id: &[u8],
    severity: u8,
    failure_count: u32,
    timestamp_ms: u64,
) -> OtaNotifyAlert {
    let firmware_hash = fnv1a(firmware_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&firmware_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    OtaNotifyAlert {
        content_hash: fnv1a(&buf),
        severity,
        firmware_hash,
        failure_count,
        timestamp_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = ota_to_db_record(b"fw-v2.0.0", 500, 200, 16_777_216, 1_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.firmware_hash, 0);
    }

    #[test]
    fn db_record_fields_preserved() {
        let rec = ota_to_db_record(b"fw", 100, 3, 4_096, 42_000);
        assert_eq!(rec.device_count, 100);
        assert_eq!(rec.version, 3);
        assert_eq!(rec.image_bytes, 4_096);
        assert_eq!(rec.timestamp_ms, 42_000);
    }

    #[test]
    fn db_record_hash_deterministic() {
        let a = ota_to_db_record(b"fwx", 50, 1, 2_048, 0);
        let b = ota_to_db_record(b"fwx", 50, 1, 2_048, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_small_image_long_ttl() {
        let entry = ota_to_cache_entry(b"fw1", 1_000_000, 1);
        assert_eq!(entry.ttl_secs, 3_600);
    }

    #[test]
    fn cache_entry_large_image_short_ttl() {
        let entry = ota_to_cache_entry(b"fw2", 64_000_000, 2);
        assert_eq!(entry.ttl_secs, 300);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_fields_and_hash() {
        let ev = ota_to_analytics_event(b"fw-99", 1_000, 950, 50, 45_000, 7_000_000);
        assert_eq!(ev.update_count, 1_000);
        assert_eq!(ev.success_count, 950);
        assert_eq!(ev.failure_count, 50);
        assert_eq!(ev.avg_duration_ms, 45_000);
        assert_ne!(ev.content_hash, 0);
    }

    // ── Edge telemetry tests ──────────────────────────────────────────────

    #[test]
    fn edge_telemetry_progress_preserved() {
        let tel = ota_to_edge_telemetry(b"dev-01", 1, 2, 75);
        assert_eq!(tel.current_version, 1);
        assert_eq!(tel.target_version, 2);
        assert_eq!(tel.progress_pct, 75);
        assert_ne!(tel.content_hash, 0);
    }

    // ── Notify alert tests ────────────────────────────────────────────────

    #[test]
    fn notify_alert_severity_preserved() {
        let alert = ota_to_notify_alert(b"fw-bad", 3, 42, 8_000_000);
        assert_eq!(alert.severity, 3);
        assert_eq!(alert.failure_count, 42);
        assert_ne!(alert.content_hash, 0);
    }

    #[test]
    fn different_firmware_produce_different_hashes() {
        let a = ota_to_db_record(b"fw-A", 10, 1, 1024, 0);
        let b = ota_to_db_record(b"fw-B", 10, 1, 1024, 0);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
