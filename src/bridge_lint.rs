//! Lint bridges — Lint ↔ DB, Cache, Analytics, CI, API
//!
//! 5 bridges connecting lint scan data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Lint → DB (scan record persistence) ────────────────────────

/// Lint scan record for ALICE-DB persistence.
pub struct LintDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Number of files scanned.
    pub file_count: u32,
    /// Total warnings emitted.
    pub warning_count: u32,
    /// Total errors emitted.
    pub error_count: u32,
    /// Number of lint rules applied.
    pub rule_count: u32,
    /// Language identifier hash.
    pub language_hash: u64,
}

/// Serialize lint scan data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn lint_to_db_record(
    file_count: u32,
    warning_count: u32,
    error_count: u32,
    rule_count: u32,
    language_hash: u64,
) -> LintDbRecord {
    // buf: file_count(4) + warning_count(4) + error_count(4) + rule_count(4) + language_hash(8) = 24
    let mut buf = [0u8; 24];
    buf[0..4].copy_from_slice(&file_count.to_le_bytes());
    buf[4..8].copy_from_slice(&warning_count.to_le_bytes());
    buf[8..12].copy_from_slice(&error_count.to_le_bytes());
    buf[12..16].copy_from_slice(&rule_count.to_le_bytes());
    buf[16..24].copy_from_slice(&language_hash.to_le_bytes());
    LintDbRecord {
        content_hash: fnv1a(&buf),
        file_count,
        warning_count,
        error_count,
        rule_count,
        language_hash,
    }
}

// ── Bridge 2: Lint → Cache (file scan cache entry) ───────────────────────

/// Lint file scan cache entry for ALICE-Cache.
pub struct LintCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Hash of the scanned file contents.
    pub file_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Warning count for this file.
    pub warning_count: u32,
    /// Error count for this file.
    pub error_count: u32,
}

/// Build lint file scan cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn lint_to_cache_entry(
    file_hash: u64,
    ttl_secs: u32,
    warning_count: u32,
    error_count: u32,
) -> LintCacheEntry {
    // buf: file_hash(8) + warning_count(4) + error_count(4) = 16
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&file_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&warning_count.to_le_bytes());
    buf[12..16].copy_from_slice(&error_count.to_le_bytes());
    LintCacheEntry {
        content_hash: fnv1a(&buf),
        file_hash,
        ttl_secs,
        warning_count,
        error_count,
    }
}

// ── Bridge 3: Lint → Analytics (scan analytics event) ────────────────────

/// Lint analytics event for ALICE-Analytics ingestion.
pub struct LintAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Cumulative scan count.
    pub scan_count: u64,
    /// Cumulative warning total.
    pub warning_total: u64,
    /// Cumulative error total.
    pub error_total: u64,
    /// Auto-fix rate in basis points (0–10000).
    pub fix_rate_bps: u16,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build lint analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn lint_to_analytics_event(
    scan_count: u64,
    warning_total: u64,
    error_total: u64,
    fix_rate_bps: u16,
    timestamp_ms: u64,
) -> LintAnalyticsEvent {
    // buf: scan_count(8) + warning_total(8) + error_total(8) + fix_rate_bps(2) + timestamp_ms(8) = 34
    let mut buf = [0u8; 34];
    buf[0..8].copy_from_slice(&scan_count.to_le_bytes());
    buf[8..16].copy_from_slice(&warning_total.to_le_bytes());
    buf[16..24].copy_from_slice(&error_total.to_le_bytes());
    buf[24..26].copy_from_slice(&fix_rate_bps.to_le_bytes());
    buf[26..34].copy_from_slice(&timestamp_ms.to_le_bytes());
    LintAnalyticsEvent {
        content_hash: fnv1a(&buf),
        scan_count,
        warning_total,
        error_total,
        fix_rate_bps,
        timestamp_ms,
    }
}

// ── Bridge 4: Lint → CI (pipeline lint gate) ─────────────────────────────

/// Lint CI gate link for ALICE CI pipeline integration.
pub struct LintCiLink {
    /// Content hash.
    pub content_hash: u64,
    /// CI pipeline identifier hash.
    pub pipeline_hash: u64,
    /// Warning count for this pipeline run.
    pub warning_count: u32,
    /// Error count for this pipeline run.
    pub error_count: u32,
    /// Whether the lint gate passed.
    pub pass: bool,
}

/// Build lint CI gate link for ALICE CI pipeline.
#[inline]
#[must_use]
pub fn lint_to_ci_link(
    pipeline_hash: u64,
    warning_count: u32,
    error_count: u32,
    pass: bool,
) -> LintCiLink {
    // buf: pipeline_hash(8) + warning_count(4) + error_count(4) + pass(1) = 17
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&pipeline_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&warning_count.to_le_bytes());
    buf[12..16].copy_from_slice(&error_count.to_le_bytes());
    buf[16] = pass as u8;
    LintCiLink {
        content_hash: fnv1a(&buf),
        pipeline_hash,
        warning_count,
        error_count,
        pass,
    }
}

// ── Bridge 5: Lint → API (scan result payload) ───────────────────────────

/// Lint scan result API payload for ALICE-API responses.
pub struct LintApiPayload {
    /// Content hash.
    pub content_hash: u64,
    /// Number of files scanned.
    pub file_count: u32,
    /// Warning count.
    pub warning_count: u32,
    /// Error count.
    pub error_count: u32,
    /// API schema version.
    pub schema_version: u16,
}

/// Build lint scan result API payload for ALICE-API.
#[inline]
#[must_use]
pub fn lint_to_api_payload(
    file_count: u32,
    warning_count: u32,
    error_count: u32,
    schema_version: u16,
) -> LintApiPayload {
    // buf: file_count(4) + warning_count(4) + error_count(4) + schema_version(2) = 14
    let mut buf = [0u8; 14];
    buf[0..4].copy_from_slice(&file_count.to_le_bytes());
    buf[4..8].copy_from_slice(&warning_count.to_le_bytes());
    buf[8..12].copy_from_slice(&error_count.to_le_bytes());
    buf[12..14].copy_from_slice(&schema_version.to_le_bytes());
    LintApiPayload {
        content_hash: fnv1a(&buf),
        file_count,
        warning_count,
        error_count,
        schema_version,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_to_db_record_hash_nonzero() {
        let rec = lint_to_db_record(100, 25, 3, 50, 0xdead_beef_1234_5678);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_lint_to_db_record_fields() {
        let rec = lint_to_db_record(10, 5, 1, 20, 0xabcd);
        assert_eq!(rec.file_count, 10);
        assert_eq!(rec.warning_count, 5);
        assert_eq!(rec.error_count, 1);
        assert_eq!(rec.rule_count, 20);
        assert_eq!(rec.language_hash, 0xabcd);
    }

    #[test]
    fn test_lint_to_db_record_determinism() {
        let a = lint_to_db_record(5, 2, 0, 10, 0xff);
        let b = lint_to_db_record(5, 2, 0, 10, 0xff);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_lint_to_cache_entry_hash_nonzero() {
        let entry = lint_to_cache_entry(0x1234_5678_abcd_ef01, 600, 3, 0);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_lint_to_cache_entry_fields() {
        let entry = lint_to_cache_entry(0x9999, 300, 2, 1);
        assert_eq!(entry.file_hash, 0x9999);
        assert_eq!(entry.ttl_secs, 300);
        assert_eq!(entry.warning_count, 2);
        assert_eq!(entry.error_count, 1);
    }

    #[test]
    fn test_lint_to_analytics_event_hash_nonzero() {
        let ev = lint_to_analytics_event(500, 1_200, 80, 7_500, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_lint_to_analytics_event_fields() {
        let ev = lint_to_analytics_event(10, 50, 5, 6_000, 12_345);
        assert_eq!(ev.scan_count, 10);
        assert_eq!(ev.warning_total, 50);
        assert_eq!(ev.error_total, 5);
        assert_eq!(ev.fix_rate_bps, 6_000);
        assert_eq!(ev.timestamp_ms, 12_345);
    }

    #[test]
    fn test_lint_to_ci_link_pass() {
        let link = lint_to_ci_link(0xface_cafe, 5, 0, true);
        assert_ne!(link.content_hash, 0);
        assert!(link.pass);
        assert_eq!(link.error_count, 0);
    }

    #[test]
    fn test_lint_to_ci_link_fail() {
        let link = lint_to_ci_link(0xface_cafe, 10, 2, false);
        assert!(!link.pass);
        assert_eq!(link.error_count, 2);
    }

    #[test]
    fn test_lint_to_api_payload_hash_nonzero() {
        let payload = lint_to_api_payload(50, 10, 2, 1);
        assert_ne!(payload.content_hash, 0);
    }

    #[test]
    fn test_lint_to_api_payload_fields() {
        let payload = lint_to_api_payload(20, 4, 1, 2);
        assert_eq!(payload.file_count, 20);
        assert_eq!(payload.warning_count, 4);
        assert_eq!(payload.error_count, 1);
        assert_eq!(payload.schema_version, 2);
    }

    #[test]
    fn test_lint_to_api_payload_determinism() {
        let a = lint_to_api_payload(1, 0, 0, 1);
        let b = lint_to_api_payload(1, 0, 0, 1);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
