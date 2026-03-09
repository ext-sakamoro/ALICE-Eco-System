//! Form bridges — Form ↔ DB, Cache, Analytics, API, Notify
//!
//! 5 bridges connecting form data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Form → DB (form record persistence) ────────────────────────

/// Form record for ALICE-DB persistence.
pub struct FormDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Form identifier hash.
    pub form_hash: u64,
    /// Number of fields in the form.
    pub field_count: u16,
    /// Total submission count.
    pub submission_count: u64,
    /// Form schema version.
    pub schema_version: u32,
    /// Record timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize form data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn form_to_db_record(
    form_hash: u64,
    field_count: u16,
    submission_count: u64,
    schema_version: u32,
    timestamp_ms: u64,
) -> FormDbRecord {
    // buf: form_hash(8) + field_count(2) + submission_count(8) + schema_version(4) + timestamp_ms(8) = 30
    let mut buf = [0u8; 30];
    buf[0..8].copy_from_slice(&form_hash.to_le_bytes());
    buf[8..10].copy_from_slice(&field_count.to_le_bytes());
    buf[10..18].copy_from_slice(&submission_count.to_le_bytes());
    buf[18..22].copy_from_slice(&schema_version.to_le_bytes());
    buf[22..30].copy_from_slice(&timestamp_ms.to_le_bytes());
    FormDbRecord {
        content_hash: fnv1a(&buf),
        form_hash,
        field_count,
        submission_count,
        schema_version,
        timestamp_ms,
    }
}

// ── Bridge 2: Form → Cache (form schema cache entry) ─────────────────────

/// Form schema cache entry for ALICE-Cache.
pub struct FormCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Form identifier hash.
    pub form_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of fields.
    pub field_count: u16,
    /// Serialized schema size in bytes.
    pub schema_bytes: u64,
}

/// Build form schema cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn form_to_cache_entry(
    form_hash: u64,
    ttl_secs: u32,
    field_count: u16,
    schema_bytes: u64,
) -> FormCacheEntry {
    // buf: form_hash(8) + field_count(2) + schema_bytes(8) = 18
    let mut buf = [0u8; 18];
    buf[0..8].copy_from_slice(&form_hash.to_le_bytes());
    buf[8..10].copy_from_slice(&field_count.to_le_bytes());
    buf[10..18].copy_from_slice(&schema_bytes.to_le_bytes());
    FormCacheEntry {
        content_hash: fnv1a(&buf),
        form_hash,
        ttl_secs,
        field_count,
        schema_bytes,
    }
}

// ── Bridge 3: Form → Analytics (submission analytics event) ──────────────

/// Form analytics event for ALICE-Analytics ingestion.
pub struct FormAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Cumulative submission count.
    pub submission_count: u64,
    /// Form completion rate in basis points (0–10000).
    pub completion_rate_bps: u16,
    /// Average time to fill the form in milliseconds.
    pub avg_fill_time_ms: u64,
    /// Number of validation errors encountered.
    pub error_count: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build form analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn form_to_analytics_event(
    submission_count: u64,
    completion_rate_bps: u16,
    avg_fill_time_ms: u64,
    error_count: u32,
    timestamp_ms: u64,
) -> FormAnalyticsEvent {
    // buf: submission_count(8) + completion_rate_bps(2) + avg_fill_time_ms(8) + error_count(4) + timestamp_ms(8) = 30
    let mut buf = [0u8; 30];
    buf[0..8].copy_from_slice(&submission_count.to_le_bytes());
    buf[8..10].copy_from_slice(&completion_rate_bps.to_le_bytes());
    buf[10..18].copy_from_slice(&avg_fill_time_ms.to_le_bytes());
    buf[18..22].copy_from_slice(&error_count.to_le_bytes());
    buf[22..30].copy_from_slice(&timestamp_ms.to_le_bytes());
    FormAnalyticsEvent {
        content_hash: fnv1a(&buf),
        submission_count,
        completion_rate_bps,
        avg_fill_time_ms,
        error_count,
        timestamp_ms,
    }
}

// ── Bridge 4: Form → API (form summary payload) ───────────────────────────

/// Form summary API payload for ALICE-API responses.
pub struct FormApiPayload {
    /// Content hash.
    pub content_hash: u64,
    /// Form identifier hash.
    pub form_hash: u64,
    /// Number of fields.
    pub field_count: u16,
    /// Total submission count.
    pub submission_count: u64,
    /// API schema version.
    pub schema_version: u16,
}

/// Build form summary API payload for ALICE-API.
#[inline]
#[must_use]
pub fn form_to_api_payload(
    form_hash: u64,
    field_count: u16,
    submission_count: u64,
    schema_version: u16,
) -> FormApiPayload {
    // buf: form_hash(8) + field_count(2) + submission_count(8) + schema_version(2) = 20
    let mut buf = [0u8; 20];
    buf[0..8].copy_from_slice(&form_hash.to_le_bytes());
    buf[8..10].copy_from_slice(&field_count.to_le_bytes());
    buf[10..18].copy_from_slice(&submission_count.to_le_bytes());
    buf[18..20].copy_from_slice(&schema_version.to_le_bytes());
    FormApiPayload {
        content_hash: fnv1a(&buf),
        form_hash,
        field_count,
        submission_count,
        schema_version,
    }
}

// ── Bridge 5: Form → Notify (validation error alert) ─────────────────────

/// Form validation error alert for ALICE-Notify.
pub struct FormNotifyAlert {
    /// Content hash.
    pub content_hash: u64,
    /// Alert severity level (0 = info, 1 = warn, 2 = critical).
    pub severity: u8,
    /// Form identifier hash.
    pub form_hash: u64,
    /// Number of validation errors.
    pub error_count: u32,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build form validation error alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn form_to_notify_alert(
    severity: u8,
    form_hash: u64,
    error_count: u32,
    timestamp_ms: u64,
) -> FormNotifyAlert {
    // buf: severity(1) + form_hash(8) + error_count(4) + timestamp_ms(8) = 21
    let mut buf = [0u8; 21];
    buf[0] = severity;
    buf[1..9].copy_from_slice(&form_hash.to_le_bytes());
    buf[9..13].copy_from_slice(&error_count.to_le_bytes());
    buf[13..21].copy_from_slice(&timestamp_ms.to_le_bytes());
    FormNotifyAlert {
        content_hash: fnv1a(&buf),
        severity,
        form_hash,
        error_count,
        timestamp_ms,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_to_db_record_hash_nonzero() {
        let rec = form_to_db_record(0xdead_cafe_1234_5678, 10, 500, 2, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_form_to_db_record_fields() {
        let rec = form_to_db_record(0x1234, 5, 100, 1, 99_999);
        assert_eq!(rec.form_hash, 0x1234);
        assert_eq!(rec.field_count, 5);
        assert_eq!(rec.submission_count, 100);
        assert_eq!(rec.schema_version, 1);
        assert_eq!(rec.timestamp_ms, 99_999);
    }

    #[test]
    fn test_form_to_db_record_determinism() {
        let a = form_to_db_record(0xab, 3, 10, 1, 500);
        let b = form_to_db_record(0xab, 3, 10, 1, 500);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_form_to_cache_entry_hash_nonzero() {
        let entry = form_to_cache_entry(0xbeef_0001, 3_600, 8, 2_048);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_form_to_cache_entry_fields() {
        let entry = form_to_cache_entry(0x7777, 1_800, 4, 512);
        assert_eq!(entry.form_hash, 0x7777);
        assert_eq!(entry.ttl_secs, 1_800);
        assert_eq!(entry.field_count, 4);
        assert_eq!(entry.schema_bytes, 512);
    }

    #[test]
    fn test_form_to_analytics_event_hash_nonzero() {
        let ev = form_to_analytics_event(1_000, 7_800, 45_000, 30, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_form_to_analytics_event_fields() {
        let ev = form_to_analytics_event(200, 8_500, 30_000, 10, 77_777);
        assert_eq!(ev.submission_count, 200);
        assert_eq!(ev.completion_rate_bps, 8_500);
        assert_eq!(ev.avg_fill_time_ms, 30_000);
        assert_eq!(ev.error_count, 10);
        assert_eq!(ev.timestamp_ms, 77_777);
    }

    #[test]
    fn test_form_to_api_payload_hash_nonzero() {
        let payload = form_to_api_payload(0xface_cafe, 12, 250, 1);
        assert_ne!(payload.content_hash, 0);
    }

    #[test]
    fn test_form_to_api_payload_fields() {
        let payload = form_to_api_payload(0x9999, 6, 50, 2);
        assert_eq!(payload.form_hash, 0x9999);
        assert_eq!(payload.field_count, 6);
        assert_eq!(payload.submission_count, 50);
        assert_eq!(payload.schema_version, 2);
    }

    #[test]
    fn test_form_to_notify_alert_hash_nonzero() {
        let alert = form_to_notify_alert(1, 0xcafe_babe, 25, 1_700_000_000_000);
        assert_ne!(alert.content_hash, 0);
    }

    #[test]
    fn test_form_to_notify_alert_fields() {
        let alert = form_to_notify_alert(2, 0x1234, 5, 12_345);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.form_hash, 0x1234);
        assert_eq!(alert.error_count, 5);
        assert_eq!(alert.timestamp_ms, 12_345);
    }

    #[test]
    fn test_form_to_notify_alert_determinism() {
        let a = form_to_notify_alert(1, 0xff, 3, 999);
        let b = form_to_notify_alert(1, 0xff, 3, 999);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
