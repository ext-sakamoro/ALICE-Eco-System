//! Identity bridges — Identity ↔ DB, Cache, Analytics, Auth, Notify
//!
//! 5 bridges connecting identity management data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Identity → DB (identity record persistence) ────────────────

/// Identity record for ALICE-DB persistence.
pub struct IdentityDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// User identifier hash.
    pub user_hash: u64,
    /// Identity provider hash.
    pub provider_hash: u64,
    /// Number of identity claims.
    pub claim_count: u16,
    /// Whether multi-factor authentication is enabled.
    pub mfa_enabled: bool,
    /// Record timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize identity data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn identity_to_db_record(
    user_hash: u64,
    provider_hash: u64,
    claim_count: u16,
    mfa_enabled: bool,
    timestamp_ms: u64,
) -> IdentityDbRecord {
    // buf: user_hash(8) + provider_hash(8) + claim_count(2) + mfa_enabled(1) + timestamp_ms(8) = 27
    let mut buf = [0u8; 27];
    buf[0..8].copy_from_slice(&user_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&provider_hash.to_le_bytes());
    buf[16..18].copy_from_slice(&claim_count.to_le_bytes());
    buf[18] = mfa_enabled as u8;
    buf[19..27].copy_from_slice(&timestamp_ms.to_le_bytes());
    IdentityDbRecord {
        content_hash: fnv1a(&buf),
        user_hash,
        provider_hash,
        claim_count,
        mfa_enabled,
        timestamp_ms,
    }
}

// ── Bridge 2: Identity → Cache (session cache entry) ─────────────────────

/// Identity session cache entry for ALICE-Cache.
pub struct IdentityCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// User identifier hash.
    pub user_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of active sessions.
    pub session_count: u32,
    /// Last login timestamp in seconds since epoch.
    pub last_login_ts: u64,
}

/// Build identity session cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn identity_to_cache_entry(
    user_hash: u64,
    ttl_secs: u32,
    session_count: u32,
    last_login_ts: u64,
) -> IdentityCacheEntry {
    // buf: user_hash(8) + session_count(4) + last_login_ts(8) = 20
    let mut buf = [0u8; 20];
    buf[0..8].copy_from_slice(&user_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&session_count.to_le_bytes());
    buf[12..20].copy_from_slice(&last_login_ts.to_le_bytes());
    IdentityCacheEntry {
        content_hash: fnv1a(&buf),
        user_hash,
        ttl_secs,
        session_count,
        last_login_ts,
    }
}

// ── Bridge 3: Identity → Analytics (login analytics event) ───────────────

/// Identity analytics event for ALICE-Analytics ingestion.
pub struct IdentityAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Total successful login count.
    pub login_count: u64,
    /// Total failed login count.
    pub failed_count: u64,
    /// MFA usage rate in basis points (0–10000).
    pub mfa_usage_bps: u16,
    /// Number of unique users active in the period.
    pub unique_users: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build identity analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn identity_to_analytics_event(
    login_count: u64,
    failed_count: u64,
    mfa_usage_bps: u16,
    unique_users: u64,
    timestamp_ms: u64,
) -> IdentityAnalyticsEvent {
    // buf: login_count(8) + failed_count(8) + mfa_usage_bps(2) + unique_users(8) + timestamp_ms(8) = 34
    let mut buf = [0u8; 34];
    buf[0..8].copy_from_slice(&login_count.to_le_bytes());
    buf[8..16].copy_from_slice(&failed_count.to_le_bytes());
    buf[16..18].copy_from_slice(&mfa_usage_bps.to_le_bytes());
    buf[18..26].copy_from_slice(&unique_users.to_le_bytes());
    buf[26..34].copy_from_slice(&timestamp_ms.to_le_bytes());
    IdentityAnalyticsEvent {
        content_hash: fnv1a(&buf),
        login_count,
        failed_count,
        mfa_usage_bps,
        unique_users,
        timestamp_ms,
    }
}

// ── Bridge 4: Identity → Auth (token link) ───────────────────────────────

/// Identity auth token link for ALICE-Auth.
pub struct IdentityAuthLink {
    /// Content hash.
    pub content_hash: u64,
    /// User identifier hash.
    pub user_hash: u64,
    /// Identity provider hash.
    pub provider_hash: u64,
    /// OAuth scope hash.
    pub scope_hash: u64,
    /// Token expiry timestamp in seconds since epoch.
    pub token_expiry_ts: u64,
}

/// Build identity auth token link for ALICE-Auth.
#[inline]
#[must_use]
pub fn identity_to_auth_link(
    user_hash: u64,
    provider_hash: u64,
    scope_hash: u64,
    token_expiry_ts: u64,
) -> IdentityAuthLink {
    // buf: user_hash(8) + provider_hash(8) + scope_hash(8) + token_expiry_ts(8) = 32
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&user_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&provider_hash.to_le_bytes());
    buf[16..24].copy_from_slice(&scope_hash.to_le_bytes());
    buf[24..32].copy_from_slice(&token_expiry_ts.to_le_bytes());
    IdentityAuthLink {
        content_hash: fnv1a(&buf),
        user_hash,
        provider_hash,
        scope_hash,
        token_expiry_ts,
    }
}

// ── Bridge 5: Identity → Notify (security alert) ─────────────────────────

/// Identity security alert for ALICE-Notify.
pub struct IdentityNotifyAlert {
    /// Content hash.
    pub content_hash: u64,
    /// Alert severity level (0 = info, 1 = warn, 2 = critical).
    pub severity: u8,
    /// User identifier hash.
    pub user_hash: u64,
    /// Consecutive failed login count.
    pub failed_count: u64,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build identity security alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn identity_to_notify_alert(
    severity: u8,
    user_hash: u64,
    failed_count: u64,
    timestamp_ms: u64,
) -> IdentityNotifyAlert {
    // buf: severity(1) + user_hash(8) + failed_count(8) + timestamp_ms(8) = 25
    let mut buf = [0u8; 25];
    buf[0] = severity;
    buf[1..9].copy_from_slice(&user_hash.to_le_bytes());
    buf[9..17].copy_from_slice(&failed_count.to_le_bytes());
    buf[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    IdentityNotifyAlert {
        content_hash: fnv1a(&buf),
        severity,
        user_hash,
        failed_count,
        timestamp_ms,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_to_db_record_hash_nonzero() {
        let rec = identity_to_db_record(0xdead_beef_0001, 0xcafe_1234, 5, true, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_identity_to_db_record_fields() {
        let rec = identity_to_db_record(0x1111, 0x2222, 3, false, 99_999);
        assert_eq!(rec.user_hash, 0x1111);
        assert_eq!(rec.provider_hash, 0x2222);
        assert_eq!(rec.claim_count, 3);
        assert!(!rec.mfa_enabled);
        assert_eq!(rec.timestamp_ms, 99_999);
    }

    #[test]
    fn test_identity_to_db_record_mfa_enabled() {
        let rec = identity_to_db_record(0xffff, 0x0001, 2, true, 1_000);
        assert!(rec.mfa_enabled);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_identity_to_cache_entry_hash_nonzero() {
        let entry = identity_to_cache_entry(0xabcd_1234, 3_600, 2, 1_700_000_000);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_identity_to_cache_entry_fields() {
        let entry = identity_to_cache_entry(0x5555, 7_200, 1, 1_699_999_999);
        assert_eq!(entry.user_hash, 0x5555);
        assert_eq!(entry.ttl_secs, 7_200);
        assert_eq!(entry.session_count, 1);
        assert_eq!(entry.last_login_ts, 1_699_999_999);
    }

    #[test]
    fn test_identity_to_analytics_event_hash_nonzero() {
        let ev = identity_to_analytics_event(5_000, 100, 6_500, 3_200, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_identity_to_analytics_event_fields() {
        let ev = identity_to_analytics_event(200, 10, 7_000, 150, 55_555);
        assert_eq!(ev.login_count, 200);
        assert_eq!(ev.failed_count, 10);
        assert_eq!(ev.mfa_usage_bps, 7_000);
        assert_eq!(ev.unique_users, 150);
        assert_eq!(ev.timestamp_ms, 55_555);
    }

    #[test]
    fn test_identity_to_auth_link_hash_nonzero() {
        let link = identity_to_auth_link(0xbeef_cafe, 0x1234_5678, 0xaaaa_bbbb, 1_700_100_000);
        assert_ne!(link.content_hash, 0);
    }

    #[test]
    fn test_identity_to_auth_link_fields() {
        let link = identity_to_auth_link(0x1111, 0x2222, 0x3333, 9_999_999);
        assert_eq!(link.user_hash, 0x1111);
        assert_eq!(link.provider_hash, 0x2222);
        assert_eq!(link.scope_hash, 0x3333);
        assert_eq!(link.token_expiry_ts, 9_999_999);
    }

    #[test]
    fn test_identity_to_notify_alert_hash_nonzero() {
        let alert = identity_to_notify_alert(2, 0xcafe_0001, 10, 1_700_000_000_000);
        assert_ne!(alert.content_hash, 0);
    }

    #[test]
    fn test_identity_to_notify_alert_fields() {
        let alert = identity_to_notify_alert(1, 0x9999, 5, 12_345);
        assert_eq!(alert.severity, 1);
        assert_eq!(alert.user_hash, 0x9999);
        assert_eq!(alert.failed_count, 5);
        assert_eq!(alert.timestamp_ms, 12_345);
    }

    #[test]
    fn test_identity_to_notify_alert_determinism() {
        let a = identity_to_notify_alert(2, 0xff, 3, 999);
        let b = identity_to_notify_alert(2, 0xff, 3, 999);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
