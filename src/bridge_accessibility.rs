//! Accessibility bridges — Accessibility ↔ DB, Cache, Analytics, API, Render
//!
//! 5 bridges connecting accessibility scan data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Accessibility → DB (scan record persistence) ───────────────

/// Accessibility scan record for ALICE-DB persistence.
pub struct AccessibilityDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Number of accessibility violations found.
    pub violation_count: u32,
    /// Total number of DOM elements evaluated.
    pub element_count: u64,
    /// Page identifier hash.
    pub page_hash: u64,
    /// WCAG conformance level (0 = A, 1 = AA, 2 = AAA).
    pub wcag_level: u8,
    /// Scan timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize accessibility scan data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn accessibility_to_db_record(
    violation_count: u32,
    element_count: u64,
    page_hash: u64,
    wcag_level: u8,
    timestamp_ms: u64,
) -> AccessibilityDbRecord {
    // buf: violation_count(4) + element_count(8) + page_hash(8) + wcag_level(1) + timestamp_ms(8) = 29
    let mut buf = [0u8; 29];
    buf[0..4].copy_from_slice(&violation_count.to_le_bytes());
    buf[4..12].copy_from_slice(&element_count.to_le_bytes());
    buf[12..20].copy_from_slice(&page_hash.to_le_bytes());
    buf[20] = wcag_level;
    buf[21..29].copy_from_slice(&timestamp_ms.to_le_bytes());
    AccessibilityDbRecord {
        content_hash: fnv1a(&buf),
        violation_count,
        element_count,
        page_hash,
        wcag_level,
        timestamp_ms,
    }
}

// ── Bridge 2: Accessibility → Cache (page scan cache entry) ──────────────

/// Accessibility page scan cache entry for ALICE-Cache.
pub struct AccessibilityCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Page identifier hash.
    pub page_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of violations found.
    pub violation_count: u32,
    /// Accessibility score in basis points (0–10000, higher is better).
    pub score_bps: u16,
}

/// Build accessibility page scan cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn accessibility_to_cache_entry(
    page_hash: u64,
    ttl_secs: u32,
    violation_count: u32,
    score_bps: u16,
) -> AccessibilityCacheEntry {
    // buf: page_hash(8) + violation_count(4) + score_bps(2) = 14
    let mut buf = [0u8; 14];
    buf[0..8].copy_from_slice(&page_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&violation_count.to_le_bytes());
    buf[12..14].copy_from_slice(&score_bps.to_le_bytes());
    AccessibilityCacheEntry {
        content_hash: fnv1a(&buf),
        page_hash,
        ttl_secs,
        violation_count,
        score_bps,
    }
}

// ── Bridge 3: Accessibility → Analytics (scan analytics event) ───────────

/// Accessibility analytics event for ALICE-Analytics ingestion.
pub struct AccessibilityAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Cumulative scan count.
    pub scan_count: u64,
    /// Cumulative violation count.
    pub violation_total: u64,
    /// Compliance rate in basis points (0–10000).
    pub compliance_bps: u16,
    /// Average accessibility score in basis points (0–10000).
    pub avg_score_bps: u16,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build accessibility analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn accessibility_to_analytics_event(
    scan_count: u64,
    violation_total: u64,
    compliance_bps: u16,
    avg_score_bps: u16,
    timestamp_ms: u64,
) -> AccessibilityAnalyticsEvent {
    // buf: scan_count(8) + violation_total(8) + compliance_bps(2) + avg_score_bps(2) + timestamp_ms(8) = 28
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&scan_count.to_le_bytes());
    buf[8..16].copy_from_slice(&violation_total.to_le_bytes());
    buf[16..18].copy_from_slice(&compliance_bps.to_le_bytes());
    buf[18..20].copy_from_slice(&avg_score_bps.to_le_bytes());
    buf[20..28].copy_from_slice(&timestamp_ms.to_le_bytes());
    AccessibilityAnalyticsEvent {
        content_hash: fnv1a(&buf),
        scan_count,
        violation_total,
        compliance_bps,
        avg_score_bps,
        timestamp_ms,
    }
}

// ── Bridge 4: Accessibility → API (scan result payload) ──────────────────

/// Accessibility scan result API payload for ALICE-API responses.
pub struct AccessibilityApiPayload {
    /// Content hash.
    pub content_hash: u64,
    /// Page identifier hash.
    pub page_hash: u64,
    /// Number of violations found.
    pub violation_count: u32,
    /// Accessibility score in basis points (0–10000).
    pub score_bps: u16,
    /// API schema version.
    pub schema_version: u16,
}

/// Build accessibility scan result API payload for ALICE-API.
#[inline]
#[must_use]
pub fn accessibility_to_api_payload(
    page_hash: u64,
    violation_count: u32,
    score_bps: u16,
    schema_version: u16,
) -> AccessibilityApiPayload {
    // buf: page_hash(8) + violation_count(4) + score_bps(2) + schema_version(2) = 16
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&page_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&violation_count.to_le_bytes());
    buf[12..14].copy_from_slice(&score_bps.to_le_bytes());
    buf[14..16].copy_from_slice(&schema_version.to_le_bytes());
    AccessibilityApiPayload {
        content_hash: fnv1a(&buf),
        page_hash,
        violation_count,
        score_bps,
        schema_version,
    }
}

// ── Bridge 5: Accessibility → Render (overlay descriptor) ────────────────

/// Accessibility render overlay descriptor for ALICE-Render.
pub struct AccessibilityRenderOverlay {
    /// Content hash.
    pub content_hash: u64,
    /// Number of DOM elements with accessibility annotations.
    pub element_count: u64,
    /// Number of visual annotations in the overlay.
    pub annotation_count: u32,
    /// Overlay pixel data size in bytes.
    pub overlay_bytes: u64,
    /// Overlay render time in microseconds.
    pub render_time_us: u64,
}

/// Build accessibility render overlay descriptor for ALICE-Render.
#[inline]
#[must_use]
pub fn accessibility_to_render_overlay(
    element_count: u64,
    annotation_count: u32,
    overlay_bytes: u64,
    render_time_us: u64,
) -> AccessibilityRenderOverlay {
    // buf: element_count(8) + annotation_count(4) + overlay_bytes(8) + render_time_us(8) = 28
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&element_count.to_le_bytes());
    buf[8..12].copy_from_slice(&annotation_count.to_le_bytes());
    buf[12..20].copy_from_slice(&overlay_bytes.to_le_bytes());
    buf[20..28].copy_from_slice(&render_time_us.to_le_bytes());
    AccessibilityRenderOverlay {
        content_hash: fnv1a(&buf),
        element_count,
        annotation_count,
        overlay_bytes,
        render_time_us,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessibility_to_db_record_hash_nonzero() {
        let rec = accessibility_to_db_record(12, 350, 0xdead_beef_0001, 1, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_accessibility_to_db_record_fields() {
        let rec = accessibility_to_db_record(5, 200, 0x1234, 0, 99_999);
        assert_eq!(rec.violation_count, 5);
        assert_eq!(rec.element_count, 200);
        assert_eq!(rec.page_hash, 0x1234);
        assert_eq!(rec.wcag_level, 0);
        assert_eq!(rec.timestamp_ms, 99_999);
    }

    #[test]
    fn test_accessibility_to_db_record_determinism() {
        let a = accessibility_to_db_record(0, 100, 0xab, 2, 500);
        let b = accessibility_to_db_record(0, 100, 0xab, 2, 500);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_accessibility_to_cache_entry_hash_nonzero() {
        let entry = accessibility_to_cache_entry(0xbeef_cafe, 3_600, 3, 9_200);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_accessibility_to_cache_entry_fields() {
        let entry = accessibility_to_cache_entry(0x5555, 1_800, 1, 9_800);
        assert_eq!(entry.page_hash, 0x5555);
        assert_eq!(entry.ttl_secs, 1_800);
        assert_eq!(entry.violation_count, 1);
        assert_eq!(entry.score_bps, 9_800);
    }

    #[test]
    fn test_accessibility_to_analytics_event_hash_nonzero() {
        let ev = accessibility_to_analytics_event(1_000, 8_500, 9_150, 8_700, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_accessibility_to_analytics_event_fields() {
        let ev = accessibility_to_analytics_event(200, 400, 8_000, 8_500, 55_555);
        assert_eq!(ev.scan_count, 200);
        assert_eq!(ev.violation_total, 400);
        assert_eq!(ev.compliance_bps, 8_000);
        assert_eq!(ev.avg_score_bps, 8_500);
        assert_eq!(ev.timestamp_ms, 55_555);
    }

    #[test]
    fn test_accessibility_to_api_payload_hash_nonzero() {
        let payload = accessibility_to_api_payload(0xcafe_face, 7, 9_100, 1);
        assert_ne!(payload.content_hash, 0);
    }

    #[test]
    fn test_accessibility_to_api_payload_fields() {
        let payload = accessibility_to_api_payload(0x9999, 2, 9_500, 2);
        assert_eq!(payload.page_hash, 0x9999);
        assert_eq!(payload.violation_count, 2);
        assert_eq!(payload.score_bps, 9_500);
        assert_eq!(payload.schema_version, 2);
    }

    #[test]
    fn test_accessibility_to_render_overlay_hash_nonzero() {
        let overlay = accessibility_to_render_overlay(350, 42, 131_072, 2_500);
        assert_ne!(overlay.content_hash, 0);
    }

    #[test]
    fn test_accessibility_to_render_overlay_fields() {
        let overlay = accessibility_to_render_overlay(100, 15, 65_536, 1_000);
        assert_eq!(overlay.element_count, 100);
        assert_eq!(overlay.annotation_count, 15);
        assert_eq!(overlay.overlay_bytes, 65_536);
        assert_eq!(overlay.render_time_us, 1_000);
    }

    #[test]
    fn test_accessibility_to_render_overlay_determinism() {
        let a = accessibility_to_render_overlay(10, 5, 4_096, 500);
        let b = accessibility_to_render_overlay(10, 5, 4_096, 500);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
