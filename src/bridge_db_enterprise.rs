//! DB-Enterprise bridges — ALICE-DB-Enterprise ↔ Auth, Billing, DB
//!
//! 5 bridges connecting the enterprise database licensing layer
//! to authentication, billing, and the core DB engine.

// ── Bridge 1: DB-Enterprise → Auth (license-gated access) ───────────────

/// License verification result for ALICE-Auth integration.
pub struct DbEnterpriseLicenseCheck {
    /// Deterministic hash of the license key.
    pub content_hash: u64,
    /// Whether the license key is valid and active.
    pub is_valid: bool,
    /// Enabled enterprise features as a bitmask.
    pub feature_flags: u32,
}

/// Validate an enterprise license key and return the feature set
/// for ALICE-Auth permission checks.
#[inline]
#[must_use]
pub fn db_enterprise_to_auth_license(key: &str) -> DbEnterpriseLicenseCheck {
    let is_valid = alice_db_enterprise::validate_license(key);
    let content_hash = fnv1a(key.as_bytes());
    let feature_flags = if is_valid {
        let mut flags = 0u32;
        if alice_db_enterprise::has_feature("clustering") {
            flags |= 1;
        }
        if alice_db_enterprise::has_feature("replication") {
            flags |= 2;
        }
        if alice_db_enterprise::has_feature("advanced_index") {
            flags |= 4;
        }
        flags
    } else {
        0
    };
    DbEnterpriseLicenseCheck {
        content_hash,
        is_valid,
        feature_flags,
    }
}

// ── Bridge 2: DB-Enterprise → Billing (trial activation telemetry) ──────

/// Trial activation event for ALICE-Billing integration.
pub struct DbEnterpriseTrialEvent {
    /// Deterministic hash of the trial key.
    pub content_hash: u64,
    /// Trial duration in days.
    pub duration_days: u16,
    /// Generated trial key (hashed for storage).
    pub trial_key_hash: u64,
}

/// Activate a trial license and emit a billing event.
#[inline]
#[must_use]
pub fn db_enterprise_to_billing_trial(days: u16) -> DbEnterpriseTrialEvent {
    let trial_key = alice_db_enterprise::activate_trial(days);
    let trial_key_hash = fnv1a(trial_key.as_bytes());
    let content_hash = fnv1a(&days.to_le_bytes());
    DbEnterpriseTrialEvent {
        content_hash,
        duration_days: days,
        trial_key_hash,
    }
}

// ── Bridge 3: DB-Enterprise → DB (feature availability query) ───────────

/// Enterprise feature availability for ALICE-DB query planner.
pub struct DbEnterpriseFeatureSet {
    /// Deterministic hash over (clustering, replication, advanced_index).
    pub content_hash: u64,
    /// Clustering enabled (multi-node sharding).
    pub clustering: bool,
    /// Replication enabled (read replicas).
    pub replication: bool,
    /// Advanced indexing enabled (bloom, LSM-tree, fractal).
    pub advanced_index: bool,
    /// Feature count (for telemetry).
    pub enabled_count: u8,
}

/// Query the available enterprise features for ALICE-DB's query planner.
#[inline]
#[must_use]
pub fn db_enterprise_to_db_features() -> DbEnterpriseFeatureSet {
    let clustering = alice_db_enterprise::has_feature("clustering");
    let replication = alice_db_enterprise::has_feature("replication");
    let advanced_index = alice_db_enterprise::has_feature("advanced_index");
    let enabled_count = u8::from(clustering) + u8::from(replication) + u8::from(advanced_index);
    let bits = [
        u8::from(clustering),
        u8::from(replication),
        u8::from(advanced_index),
    ];
    let content_hash = fnv1a(&bits);
    DbEnterpriseFeatureSet {
        content_hash,
        clustering,
        replication,
        advanced_index,
        enabled_count,
    }
}

// ── Bridge 4: DB-Enterprise → Analytics (license audit event) ───────────

/// License audit event for ALICE-Analytics.
pub struct DbEnterpriseAuditEvent {
    /// Deterministic hash of the license key.
    pub content_hash: u64,
    /// License validity status.
    pub is_valid: bool,
    /// Feature flags bitmask.
    pub feature_flags: u32,
    /// Feature count.
    pub feature_count: u8,
}

/// Generate a license audit event for ALICE-Analytics telemetry.
#[inline]
#[must_use]
pub fn db_enterprise_to_analytics_audit(key: &str) -> DbEnterpriseAuditEvent {
    let check = db_enterprise_to_auth_license(key);
    let feature_count = check.feature_flags.count_ones() as u8;
    DbEnterpriseAuditEvent {
        content_hash: check.content_hash,
        is_valid: check.is_valid,
        feature_flags: check.feature_flags,
        feature_count,
    }
}

// ── Bridge 5: DB-Enterprise → Cache (license TTL) ───────────────────────

/// License cache entry for ALICE-Cache.
pub struct DbEnterpriseCacheEntry {
    /// Deterministic hash of the license key.
    pub content_hash: u64,
    /// TTL in seconds (valid = 3600, invalid = 60).
    pub ttl_secs: u32,
    /// Whether to cache the result.
    pub cacheable: bool,
}

/// Create a cache entry for a license check result.
#[inline]
#[must_use]
pub fn db_enterprise_to_cache_license(key: &str) -> DbEnterpriseCacheEntry {
    let is_valid = alice_db_enterprise::validate_license(key);
    let content_hash = fnv1a(key.as_bytes());
    // Branchless TTL: valid=3600s, invalid=60s
    let condition = u32::from(is_valid);
    let ttl_secs = 60 + condition * 3540;
    DbEnterpriseCacheEntry {
        content_hash,
        ttl_secs,
        cacheable: true,
    }
}

// ── Shared ──────────────────────────────────────────────────────────────

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_check_invalid_hash_nonzero() {
        let result = db_enterprise_to_auth_license("invalid-key-xyz");
        assert_ne!(result.content_hash, 0);
    }

    #[test]
    fn license_check_invalid_not_valid() {
        let result = db_enterprise_to_auth_license("invalid-key-xyz");
        assert!(!result.is_valid);
        assert_eq!(result.feature_flags, 0);
    }

    #[test]
    fn trial_event_hash_nonzero() {
        let event = db_enterprise_to_billing_trial(30);
        assert_ne!(event.content_hash, 0);
        assert_ne!(event.trial_key_hash, 0);
    }

    #[test]
    fn trial_event_days() {
        let event = db_enterprise_to_billing_trial(14);
        assert_eq!(event.duration_days, 14);
    }

    #[test]
    fn feature_set_hash_nonzero() {
        let features = db_enterprise_to_db_features();
        assert_ne!(features.content_hash, 0);
    }

    #[test]
    fn feature_set_no_license() {
        let features = db_enterprise_to_db_features();
        assert_eq!(features.enabled_count, 0);
    }

    #[test]
    fn audit_event_matches_license() {
        let audit = db_enterprise_to_analytics_audit("test-key");
        let check = db_enterprise_to_auth_license("test-key");
        assert_eq!(audit.content_hash, check.content_hash);
        assert_eq!(audit.is_valid, check.is_valid);
    }

    #[test]
    fn cache_ttl_invalid_license() {
        let entry = db_enterprise_to_cache_license("bad-key");
        assert_eq!(entry.ttl_secs, 60);
        assert!(entry.cacheable);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn fnv1a_deterministic() {
        assert_eq!(fnv1a(b"test"), fnv1a(b"test"));
        assert_ne!(fnv1a(b"a"), fnv1a(b"b"));
    }

    #[test]
    fn hash_different_keys() {
        let h1 = db_enterprise_to_auth_license("key-a").content_hash;
        let h2 = db_enterprise_to_auth_license("key-b").content_hash;
        assert_ne!(h1, h2);
    }
}
