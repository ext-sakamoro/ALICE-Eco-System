//! AuthZ bridges — ALICE-AuthZ ↔ DB, Cache, Analytics, Auth, Notify
//!
//! 5 bridges connecting authorization policy evaluation to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: AuthZ → DB (policy storage) ────────────────────────────────

/// Authorization policy storage record for ALICE-DB persistence.
pub struct AuthzDbRecord {
    /// Content hash over the policy metadata.
    pub content_hash: u64,
    /// Number of authorization policies.
    pub policy_count: u32,
    /// Number of roles defined.
    pub role_count: u32,
    /// Hash of the principal (user/service) identifier.
    pub principal_hash: u64,
    /// Policy rule set version.
    pub rule_version: u32,
    /// Record timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize authorization policy metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn authz_to_db_record(
    policy_count: u32,
    role_count: u32,
    principal_hash: u64,
    rule_version: u32,
    timestamp_ms: u64,
) -> AuthzDbRecord {
    let mut buf = [0u8; 28];
    buf[0..4].copy_from_slice(&policy_count.to_le_bytes());
    buf[4..8].copy_from_slice(&role_count.to_le_bytes());
    buf[8..16].copy_from_slice(&principal_hash.to_le_bytes());
    buf[16..20].copy_from_slice(&rule_version.to_le_bytes());
    buf[20..28].copy_from_slice(&timestamp_ms.to_le_bytes());
    AuthzDbRecord {
        content_hash: fnv1a(&buf),
        policy_count,
        role_count,
        principal_hash,
        rule_version,
        timestamp_ms,
    }
}

// ── Bridge 2: AuthZ → Cache (decision cache) ─────────────────────────────

/// Authorization decision cache entry for ALICE-Cache.
pub struct AuthzCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of policies evaluated.
    pub policy_count: u32,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Hash of the cached allow/deny decision set.
    pub decision_hash: u64,
    /// Number of roles included in this decision.
    pub role_count: u32,
}

/// Build an authorization decision cache entry for ALICE-Cache.
///
/// Single-role decisions (role_count == 1) get a longer TTL (300 s) since
/// they are less likely to change; multi-role decisions get 60 s.
#[inline]
#[must_use]
pub fn authz_to_cache_entry(
    policy_count: u32,
    decision_hash: u64,
    role_count: u32,
) -> AuthzCacheEntry {
    let mut buf = [0u8; 16];
    buf[0..4].copy_from_slice(&policy_count.to_le_bytes());
    buf[4..12].copy_from_slice(&decision_hash.to_le_bytes());
    buf[12..16].copy_from_slice(&role_count.to_le_bytes());
    let single_role = (role_count == 1) as u32;
    let ttl_secs = 60 + single_role * 240;
    AuthzCacheEntry {
        content_hash: fnv1a(&buf),
        policy_count,
        ttl_secs,
        decision_hash,
        role_count,
    }
}

// ── Bridge 3: AuthZ → Analytics (decision event) ─────────────────────────

/// Authorization decision analytics event for ALICE-Analytics.
pub struct AuthzAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Total number of authorization decisions made.
    pub decision_count: u64,
    /// Number of deny decisions.
    pub deny_count: u64,
    /// Average evaluation latency in microseconds.
    pub latency_us: u64,
    /// Policy version in effect during this window.
    pub policy_version: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an authorization decision analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn authz_to_analytics_event(
    decision_count: u64,
    deny_count: u64,
    latency_us: u64,
    policy_version: u32,
    timestamp_ms: u64,
) -> AuthzAnalyticsEvent {
    let mut buf = [0u8; 36];
    buf[0..8].copy_from_slice(&decision_count.to_le_bytes());
    buf[8..16].copy_from_slice(&deny_count.to_le_bytes());
    buf[16..24].copy_from_slice(&latency_us.to_le_bytes());
    buf[24..28].copy_from_slice(&policy_version.to_le_bytes());
    buf[28..36].copy_from_slice(&timestamp_ms.to_le_bytes());
    AuthzAnalyticsEvent {
        content_hash: fnv1a(&buf),
        decision_count,
        deny_count,
        latency_us,
        policy_version,
        timestamp_ms,
    }
}

// ── Bridge 4: AuthZ → Auth (permission link) ─────────────────────────────

/// Auth permission link for ALICE-Auth integration.
pub struct AuthzAuthLink {
    /// Content hash over the permission tuple.
    pub content_hash: u64,
    /// Hash of the principal identifier.
    pub principal_hash: u64,
    /// Hash of the resource identifier.
    pub resource_hash: u64,
    /// Hash of the action identifier.
    pub action_hash: u64,
    /// Decision byte (0=deny, 1=allow).
    pub decision: u8,
}

/// Build an authorization permission link for ALICE-Auth.
#[inline]
#[must_use]
pub fn authz_to_auth_link(
    principal_hash: u64,
    resource_hash: u64,
    action_hash: u64,
    decision: u8,
) -> AuthzAuthLink {
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&principal_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&resource_hash.to_le_bytes());
    buf[16..24].copy_from_slice(&action_hash.to_le_bytes());
    buf[24] = decision;
    AuthzAuthLink {
        content_hash: fnv1a(&buf),
        principal_hash,
        resource_hash,
        action_hash,
        decision,
    }
}

// ── Bridge 5: AuthZ → Notify (alert) ─────────────────────────────────────

/// Authorization alert notification for ALICE-Notify.
pub struct AuthzNotifyAlert {
    /// Content hash over the alert tuple.
    pub content_hash: u64,
    /// Severity level (0=info, 1=warn, 2=critical).
    pub severity: u8,
    /// Number of deny decisions that triggered this alert.
    pub deny_count: u64,
    /// Hash of the principal that triggered the alert.
    pub principal_hash: u64,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an authorization alert notification for ALICE-Notify.
#[inline]
#[must_use]
pub fn authz_to_notify_alert(
    severity: u8,
    deny_count: u64,
    principal_hash: u64,
    timestamp_ms: u64,
) -> AuthzNotifyAlert {
    let mut buf = [0u8; 25];
    buf[0] = severity;
    buf[1..9].copy_from_slice(&deny_count.to_le_bytes());
    buf[9..17].copy_from_slice(&principal_hash.to_le_bytes());
    buf[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    AuthzNotifyAlert {
        content_hash: fnv1a(&buf),
        severity,
        deny_count,
        principal_hash,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authz_to_db_record_hash_nonzero() {
        let rec = authz_to_db_record(50, 5, 0x1234, 3, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_authz_to_db_record_fields() {
        let rec = authz_to_db_record(20, 3, 0xabcd, 7, 999_999);
        assert_eq!(rec.policy_count, 20);
        assert_eq!(rec.role_count, 3);
        assert_eq!(rec.principal_hash, 0xabcd);
        assert_eq!(rec.rule_version, 7);
        assert_eq!(rec.timestamp_ms, 999_999);
    }

    #[test]
    fn test_authz_to_db_record_deterministic() {
        let a = authz_to_db_record(1, 2, 3, 4, 5);
        let b = authz_to_db_record(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_authz_to_cache_entry_single_role_ttl() {
        let entry = authz_to_cache_entry(10, 0xdead, 1);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_authz_to_cache_entry_multi_role_ttl() {
        let entry = authz_to_cache_entry(10, 0xbeef, 4);
        assert_eq!(entry.ttl_secs, 60);
        assert_eq!(entry.role_count, 4);
    }

    #[test]
    fn test_authz_to_analytics_event() {
        let ev = authz_to_analytics_event(10_000, 150, 80, 5, 1_700_000_000_001);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.decision_count, 10_000);
        assert_eq!(ev.deny_count, 150);
        assert_eq!(ev.latency_us, 80);
    }

    #[test]
    fn test_authz_to_auth_link_allow() {
        let link = authz_to_auth_link(0x1111, 0x2222, 0x3333, 1);
        assert_ne!(link.content_hash, 0);
        assert_eq!(link.decision, 1);
        assert_eq!(link.principal_hash, 0x1111);
    }

    #[test]
    fn test_authz_to_notify_alert() {
        let alert = authz_to_notify_alert(2, 500, 0x4444, 1_700_000_000_002);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.deny_count, 500);
    }
}
