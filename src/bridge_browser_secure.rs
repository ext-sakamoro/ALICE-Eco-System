//! BrowserSecure bridges — ALICE-Browser-Secure ↔ Analytics, DB, Cache, Auth, Edge
//!
//! 5 bridges connecting the ALICE-Browser-Secure security engine (CSP, XSS detection,
//! CSRF tokens, HTML sanitization) to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: BrowserSecure → Analytics (security event metrics) ─────────

/// Security event metrics for ALICE-Analytics ingestion.
///
/// Captures per-request XSS detection and sanitization outcomes so the
/// analytics pipeline can track attack frequency and trend over time.
pub struct BrowserSecureAnalyticsEvent {
    /// FNV-1a hash of the input bytes — analytics stream key.
    pub content_hash: u64,
    /// Number of XSS threats detected in the input.
    pub xss_threat_count: u32,
    /// Byte length of the original unsanitized input.
    pub raw_byte_len: usize,
    /// Byte length of the sanitized output.
    pub sanitized_byte_len: usize,
    /// Bytes removed by sanitization (`raw_byte_len - sanitized_byte_len`),
    /// computed branchlessly via saturating subtraction.
    pub bytes_removed: usize,
    /// True when at least one XSS threat was detected.
    pub has_threat: bool,
}

/// Convert XSS detection and sanitization results into an analytics event.
///
/// # Optimization notes
/// - `bytes_removed` uses saturating subtraction — branchless on all targets.
/// - `has_threat` is derived from `xss_threat_count != 0` without a branch;
///   the compiler emits a `setne` / `cmp` on x86-64.
/// - content_hash is computed over the raw input bytes in a single FNV pass.
#[inline]
#[must_use]
pub fn browser_secure_to_analytics_event(
    raw_input: &str,
    xss_threat_count: u32,
    sanitized_byte_len: usize,
) -> BrowserSecureAnalyticsEvent {
    let content_hash = fnv1a(raw_input.as_bytes());
    let raw_byte_len = raw_input.len();
    let bytes_removed = raw_byte_len.saturating_sub(sanitized_byte_len);
    // Branchless boolean: converts nonzero count to true via != 0.
    let has_threat = xss_threat_count != 0;

    BrowserSecureAnalyticsEvent {
        content_hash,
        xss_threat_count,
        raw_byte_len,
        sanitized_byte_len,
        bytes_removed,
        has_threat,
    }
}

// ── Bridge 2: BrowserSecure → DB (security event log) ────────────────────

/// Security event log record for ALICE-DB persistence.
///
/// Written on every request that triggered at least one security check so
/// that the operations team can audit attack attempts and blocked content.
pub struct BrowserSecureDbRecord {
    /// FNV-1a hash of the input bytes — primary DB key.
    pub content_hash: u64,
    /// FNV-1a hash of the session identifier for join queries.
    pub session_hash: u64,
    /// Number of XSS threats detected.
    pub xss_threat_count: u32,
    /// CSRF token verification result: 1=passed, 0=failed.
    pub csrf_verified: u8,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
    /// Input byte length.
    pub input_byte_len: usize,
}

/// Serialize a security check outcome for ALICE-DB persistence.
///
/// # Optimization notes
/// - `csrf_verified` is stored as `u8` (0/1) via branchless cast from bool.
/// - Both hashes are computed independently in two FNV passes; no allocation.
#[inline]
#[must_use]
pub fn browser_secure_to_db_record(
    raw_input: &str,
    session_id: &str,
    xss_threat_count: u32,
    csrf_verified: bool,
    timestamp_ms: u64,
) -> BrowserSecureDbRecord {
    let content_hash = fnv1a(raw_input.as_bytes());
    let session_hash = fnv1a(session_id.as_bytes());
    // Branchless: bool as u8 yields 0 or 1 without a conditional branch.
    let csrf_verified = csrf_verified as u8;

    BrowserSecureDbRecord {
        content_hash,
        session_hash,
        xss_threat_count,
        csrf_verified,
        timestamp_ms,
        input_byte_len: raw_input.len(),
    }
}

// ── Bridge 3: BrowserSecure → Cache (CSP policy cache) ───────────────────

/// CSP policy cache entry for ALICE-Cache.
///
/// Caches serialized Content-Security-Policy headers keyed by policy identity
/// so that repeated header generation is avoided on the hot path.
pub struct BrowserSecureCacheEntry {
    /// FNV-1a hash of the CSP header string — cache lookup key.
    pub content_hash: u64,
    /// Byte length of the serialized CSP header.
    pub header_byte_len: usize,
    /// Number of directives in the policy.
    pub directive_count: u32,
    /// Cache TTL in seconds.
    ///
    /// Policies with more directives are considered more complex and receive
    /// a shorter TTL computed branchlessly: `base - clamp(directives, 0, 8) * step`.
    pub ttl_seconds: u32,
}

/// Build a CSP policy cache entry from the serialized header string.
///
/// # Optimization notes
/// - TTL uses branchless arithmetic: `base - min(directives, 8) * step`.
///   No conditional branches; `min` compiles to a `cmov` on x86-64.
/// - content_hash is computed over `header_bytes` in one FNV pass.
#[inline]
#[must_use]
pub fn browser_secure_to_cache_entry(
    csp_header: &str,
    directive_count: u32,
) -> BrowserSecureCacheEntry {
    // Branchless TTL: base=3600, step=300, max reduction at 8 directives.
    // Each extra directive reduces TTL by 300 s; floor at 3600 - 8*300 = 1200.
    const BASE: u32 = 3_600;
    const STEP: u32 = 300;
    const MAX_REDUCTION_DIRECTIVES: u32 = 8;

    let content_hash = fnv1a(csp_header.as_bytes());
    let header_byte_len = csp_header.len();
    let clamped = directive_count.min(MAX_REDUCTION_DIRECTIVES);
    let ttl_seconds = BASE - clamped * STEP;

    BrowserSecureCacheEntry {
        content_hash,
        header_byte_len,
        directive_count,
        ttl_seconds,
    }
}

// ── Bridge 4: BrowserSecure → Auth (CSRF + auth integration) ─────────────

/// CSRF token validation result for ALICE-Auth integration.
///
/// Passes the CSRF verification outcome to the auth layer so that session
/// state can be updated or revoked based on security policy.
pub struct BrowserSecureAuthRecord {
    /// FNV-1a hash of the session identifier — auth session key.
    pub content_hash: u64,
    /// FNV-1a hash of the CSRF token bytes — correlation key.
    pub token_hash: u64,
    /// CSRF verification result: 1=valid, 0=invalid.
    pub csrf_valid: u8,
    /// Token age in milliseconds at verification time.
    pub token_age_ms: u64,
    /// Token TTL in milliseconds as supplied at generation time.
    pub token_ttl_ms: u64,
    /// Remaining validity in milliseconds; 0 when expired.
    /// Computed branchlessly via saturating subtraction.
    pub remaining_ms: u64,
}

/// Build an auth integration record from CSRF token verification inputs.
///
/// # Optimization notes
/// - `remaining_ms` uses saturating subtraction — no branch, no underflow.
/// - `csrf_valid` cast from bool is branchless on all targets.
#[inline]
#[must_use]
pub fn browser_secure_to_auth_record(
    session_id: &str,
    csrf_token: u64,
    csrf_valid: bool,
    token_age_ms: u64,
    token_ttl_ms: u64,
) -> BrowserSecureAuthRecord {
    let content_hash = fnv1a(session_id.as_bytes());
    let token_hash = fnv1a(&csrf_token.to_le_bytes());
    let remaining_ms = token_ttl_ms.saturating_sub(token_age_ms);

    BrowserSecureAuthRecord {
        content_hash,
        token_hash,
        csrf_valid: csrf_valid as u8,
        token_age_ms,
        token_ttl_ms,
        remaining_ms,
    }
}

// ── Bridge 5: BrowserSecure → Edge (security alert forwarding) ───────────

/// Security alert for ALICE-Edge forwarding.
///
/// Emitted when a high-severity security event (XSS attempt, CSRF failure,
/// unsafe URL) should be propagated to the edge layer for rate limiting or
/// IP blocking decisions.
pub struct BrowserSecureEdgeAlert {
    /// FNV-1a hash of the source identifier (IP or session) — edge routing key.
    pub content_hash: u64,
    /// Alert severity: 0=info, 1=low, 2=medium, 3=high, 4=critical.
    pub severity: u8,
    /// Number of threats in this event.
    pub threat_count: u32,
    /// Alert type: 0=xss, 1=csrf_fail, 2=unsafe_url, 3=csp_violation.
    pub alert_type: u8,
    /// Alert generation timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build an edge security alert from detection results.
///
/// # Optimization notes
/// - `severity` and `alert_type` are passed directly as pre-mapped `u8`
///   values; callers use `match` to map enums (no `as u8` cast at call site).
/// - content_hash covers `source_id` bytes in one FNV pass.
#[inline]
#[must_use]
pub fn browser_secure_to_edge_alert(
    source_id: &str,
    alert_type: u8,
    threat_count: u32,
    severity: u8,
    timestamp_ms: u64,
) -> BrowserSecureEdgeAlert {
    let content_hash = fnv1a(source_id.as_bytes());

    BrowserSecureEdgeAlert {
        content_hash,
        severity,
        threat_count,
        alert_type,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const RAW_INPUT: &str = "<script>alert(1)</script><p>Hello</p>";
    const SESSION_ID: &str = "sess-abc-123";

    #[test]
    fn test_analytics_event_basic() {
        let ev = browser_secure_to_analytics_event(RAW_INPUT, 1, 12);
        assert_ne!(ev.content_hash, 0, "content_hash must be non-zero");
        assert_eq!(ev.xss_threat_count, 1);
        assert_eq!(ev.raw_byte_len, RAW_INPUT.len());
        assert_eq!(ev.sanitized_byte_len, 12);
        assert_eq!(ev.bytes_removed, RAW_INPUT.len().saturating_sub(12));
        assert!(ev.has_threat);
    }

    #[test]
    fn test_analytics_event_no_threat() {
        let clean = "Hello, world!";
        let ev = browser_secure_to_analytics_event(clean, 0, clean.len());
        assert!(!ev.has_threat);
        assert_eq!(ev.bytes_removed, 0);
    }

    #[test]
    fn test_analytics_event_hash_determinism() {
        let a = browser_secure_to_analytics_event(RAW_INPUT, 1, 12);
        let b = browser_secure_to_analytics_event(RAW_INPUT, 1, 12);
        assert_eq!(a.content_hash, b.content_hash, "hash must be deterministic");
    }

    #[test]
    fn test_db_record_basic() {
        let rec = browser_secure_to_db_record(RAW_INPUT, SESSION_ID, 2, true, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.session_hash, 0);
        assert_ne!(
            rec.content_hash, rec.session_hash,
            "hashes of different inputs differ"
        );
        assert_eq!(rec.xss_threat_count, 2);
        assert_eq!(rec.csrf_verified, 1);
        assert_eq!(rec.timestamp_ms, 1_700_000_000_000);
        assert_eq!(rec.input_byte_len, RAW_INPUT.len());
    }

    #[test]
    fn test_db_record_csrf_failed() {
        let rec = browser_secure_to_db_record("input", "sess", 0, false, 0);
        assert_eq!(rec.csrf_verified, 0);
    }

    #[test]
    fn test_cache_entry_ttl_branchless() {
        // 0 directives → TTL = 3600 - 0*300 = 3600.
        let e0 = browser_secure_to_cache_entry("default-src 'self'", 0);
        assert_ne!(e0.content_hash, 0);
        assert_eq!(e0.ttl_seconds, 3_600);

        // 4 directives → TTL = 3600 - 4*300 = 2400.
        let e4 = browser_secure_to_cache_entry(
            "default-src 'self'; script-src 'self'; img-src *; style-src 'self'",
            4,
        );
        assert_eq!(e4.ttl_seconds, 2_400);

        // 8 directives → TTL = 3600 - 8*300 = 1200.
        let e8 = browser_secure_to_cache_entry("many-directives", 8);
        assert_eq!(e8.ttl_seconds, 1_200);

        // 100 directives → clamped to 8 → TTL = 1200 (branchless min).
        let e100 = browser_secure_to_cache_entry("overflow", 100);
        assert_eq!(e100.ttl_seconds, 1_200);
    }

    #[test]
    fn test_auth_record_valid_token() {
        let csrf_token = 0xdeadbeef_cafebabe_u64;
        let rec = browser_secure_to_auth_record(SESSION_ID, csrf_token, true, 30_000, 300_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.token_hash, 0);
        assert_eq!(rec.csrf_valid, 1);
        assert_eq!(rec.token_age_ms, 30_000);
        assert_eq!(rec.token_ttl_ms, 300_000);
        assert_eq!(rec.remaining_ms, 270_000);
    }

    #[test]
    fn test_auth_record_expired_token() {
        // age > ttl → remaining_ms = 0 (saturating_sub, no underflow).
        let rec = browser_secure_to_auth_record("sess", 42, false, 400_000, 300_000);
        assert_eq!(rec.csrf_valid, 0);
        assert_eq!(rec.remaining_ms, 0, "saturating_sub must clamp to 0");
    }

    #[test]
    fn test_edge_alert_basic() {
        let alert = browser_secure_to_edge_alert("192.168.1.1", 0, 3, 3, 1_700_000_001_000);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.alert_type, 0); // xss
        assert_eq!(alert.threat_count, 3);
        assert_eq!(alert.severity, 3); // high
        assert_eq!(alert.timestamp_ms, 1_700_000_001_000);
    }

    #[test]
    fn test_edge_alert_hash_determinism() {
        let a = browser_secure_to_edge_alert("10.0.0.1", 1, 1, 2, 0);
        let b = browser_secure_to_edge_alert("10.0.0.1", 1, 1, 2, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
