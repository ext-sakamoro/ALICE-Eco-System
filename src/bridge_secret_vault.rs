//! SecretVault bridges — SecretVault ↔ DB, Cache, Analytics, Auth, Notify
//!
//! 5 bridges connecting secret vault data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: SecretVault → DB (vault snapshot persistence) ──────────────

/// Vault snapshot record for ALICE-DB persistence.
pub struct SecretVaultDbRecord {
    /// Content hash over vault fields.
    pub content_hash: u64,
    /// Number of secrets stored in the vault.
    pub secret_count: u32,
    /// FNV-1a hash of the vault identifier.
    pub vault_hash: u64,
    /// Encryption algorithm code: 0=AES-256-GCM, 1=ChaCha20-Poly1305, 2=AES-128-GCM.
    pub encryption_algo: u8,
    /// Number of encryption keys managed by the vault.
    pub key_count: u32,
    /// Record timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize a vault snapshot for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn secret_vault_to_db_record(
    secret_count: u32,
    vault_hash: u64,
    encryption_algo: u8,
    key_count: u32,
    timestamp_ms: u64,
) -> SecretVaultDbRecord {
    let mut key = [0u8; 29];
    key[0..4].copy_from_slice(&secret_count.to_le_bytes());
    key[4..12].copy_from_slice(&vault_hash.to_le_bytes());
    key[12] = encryption_algo;
    key[13..17].copy_from_slice(&key_count.to_le_bytes());
    key[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    key[25..29].copy_from_slice(&secret_count.to_le_bytes());
    SecretVaultDbRecord {
        content_hash: fnv1a(&key),
        secret_count,
        vault_hash,
        encryption_algo,
        key_count,
        timestamp_ms,
    }
}

// ── Bridge 2: SecretVault → Cache (vault metadata caching) ───────────────

/// Vault metadata cache entry for ALICE-Cache.
pub struct SecretVaultCacheEntry {
    /// Content hash over vault + ttl fields.
    pub content_hash: u64,
    /// FNV-1a hash of the vault identifier.
    pub vault_hash: u64,
    /// Cache TTL in seconds (shorter for high-access vaults).
    pub ttl_secs: u32,
    /// Number of secrets in the vault.
    pub secret_count: u32,
    /// Cumulative access count for this vault.
    pub access_count: u64,
}

/// Build a vault metadata cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 60 s when access_count > 1000 (hot vault).
#[inline]
#[must_use]
pub fn secret_vault_to_cache_entry(
    vault_hash: u64,
    secret_count: u32,
    access_count: u64,
) -> SecretVaultCacheEntry {
    // Branchless hot-vault TTL: 300 s normal, 60 s when access_count > 1000.
    let hot = (access_count > 1_000) as u32;
    let ttl_secs = 300_u32 - hot * 240_u32;
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&vault_hash.to_le_bytes());
    key[8..12].copy_from_slice(&secret_count.to_le_bytes());
    key[12..20].copy_from_slice(&access_count.to_le_bytes());
    SecretVaultCacheEntry {
        content_hash: fnv1a(&key),
        vault_hash,
        ttl_secs,
        secret_count,
        access_count,
    }
}

// ── Bridge 3: SecretVault → Analytics (access metrics) ───────────────────

/// Secret vault access metrics for ALICE-Analytics ingestion.
pub struct SecretVaultAnalyticsEvent {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total access count in the reporting window.
    pub access_count: u64,
    /// Number of key rotations performed.
    pub rotation_count: u32,
    /// Number of leak detection events (0 = no leak detected).
    pub leak_detected: u32,
    /// Average age of secrets in days.
    pub avg_age_days: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build vault access metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn secret_vault_to_analytics_event(
    access_count: u64,
    rotation_count: u32,
    leak_detected: u32,
    avg_age_days: u32,
    timestamp_ms: u64,
) -> SecretVaultAnalyticsEvent {
    let mut key = [0u8; 28];
    key[0..8].copy_from_slice(&access_count.to_le_bytes());
    key[8..12].copy_from_slice(&rotation_count.to_le_bytes());
    key[12..16].copy_from_slice(&leak_detected.to_le_bytes());
    key[16..20].copy_from_slice(&avg_age_days.to_le_bytes());
    key[20..28].copy_from_slice(&timestamp_ms.to_le_bytes());
    SecretVaultAnalyticsEvent {
        content_hash: fnv1a(&key),
        access_count,
        rotation_count,
        leak_detected,
        avg_age_days,
        timestamp_ms,
    }
}

// ── Bridge 4: SecretVault → Auth (principal access link) ─────────────────

/// Auth link record associating a vault with a principal.
pub struct SecretVaultAuthLink {
    /// Content hash over vault + principal + permission.
    pub content_hash: u64,
    /// FNV-1a hash of the vault identifier.
    pub vault_hash: u64,
    /// FNV-1a hash of the principal identifier (user/service account).
    pub principal_hash: u64,
    /// Permission bitmask: 0x01=read, 0x02=write, 0x04=rotate, 0x08=admin.
    pub permission: u8,
    /// Last access timestamp in milliseconds since epoch.
    pub last_access_ts: u64,
}

/// Build an auth link for ALICE-Auth associating a principal with a vault.
#[inline]
#[must_use]
pub fn secret_vault_to_auth_link(
    vault_hash: u64,
    principal_hash: u64,
    permission: u8,
    last_access_ts: u64,
) -> SecretVaultAuthLink {
    let mut key = [0u8; 25];
    key[0..8].copy_from_slice(&vault_hash.to_le_bytes());
    key[8..16].copy_from_slice(&principal_hash.to_le_bytes());
    key[16] = permission;
    key[17..25].copy_from_slice(&last_access_ts.to_le_bytes());
    SecretVaultAuthLink {
        content_hash: fnv1a(&key),
        vault_hash,
        principal_hash,
        permission,
        last_access_ts,
    }
}

// ── Bridge 5: SecretVault → Notify (security alert) ──────────────────────

/// Security alert payload for ALICE-Notify.
pub struct SecretVaultNotifyAlert {
    /// Content hash over severity + vault + reason + timestamp.
    pub content_hash: u64,
    /// Severity level: 0=info, 1=warning, 2=critical.
    pub severity: u8,
    /// FNV-1a hash of the vault identifier.
    pub vault_hash: u64,
    /// FNV-1a hash of the alert reason string.
    pub reason_hash: u64,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a security alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn secret_vault_to_notify_alert(
    severity: u8,
    vault_hash: u64,
    reason: &[u8],
    timestamp_ms: u64,
) -> SecretVaultNotifyAlert {
    let reason_hash = fnv1a(reason);
    let mut key = [0u8; 25];
    key[0] = severity;
    key[1..9].copy_from_slice(&vault_hash.to_le_bytes());
    key[9..17].copy_from_slice(&reason_hash.to_le_bytes());
    key[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    SecretVaultNotifyAlert {
        content_hash: fnv1a(&key),
        severity,
        vault_hash,
        reason_hash,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const VAULT_HASH: u64 = 0xDEAD_BEEF_CAFE_1234;
    const PRINCIPAL_HASH: u64 = 0x1234_5678_9ABC_DEF0;
    const REASON: &[u8] = b"unauthorized_access";

    #[test]
    fn test_secret_vault_to_db_record_hash_nonzero() {
        let rec = secret_vault_to_db_record(42, VAULT_HASH, 0, 5, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_secret_vault_to_db_record_fields() {
        let rec = secret_vault_to_db_record(10, VAULT_HASH, 1, 3, 1_700_000_000_000);
        assert_eq!(rec.secret_count, 10);
        assert_eq!(rec.vault_hash, VAULT_HASH);
        assert_eq!(rec.encryption_algo, 1);
        assert_eq!(rec.key_count, 3);
        assert_eq!(rec.timestamp_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_secret_vault_to_cache_entry_normal_ttl() {
        let entry = secret_vault_to_cache_entry(VAULT_HASH, 20, 500);
        assert_eq!(entry.ttl_secs, 300);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_secret_vault_to_cache_entry_hot_ttl() {
        // access_count > 1000 → hot vault → TTL = 60.
        let entry = secret_vault_to_cache_entry(VAULT_HASH, 20, 1_001);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_secret_vault_to_analytics_event_fields() {
        let ev = secret_vault_to_analytics_event(9_999, 12, 0, 30, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.access_count, 9_999);
        assert_eq!(ev.rotation_count, 12);
        assert_eq!(ev.leak_detected, 0);
        assert_eq!(ev.avg_age_days, 30);
    }

    #[test]
    fn test_secret_vault_to_analytics_event_determinism() {
        let a = secret_vault_to_analytics_event(100, 1, 0, 7, 0);
        let b = secret_vault_to_analytics_event(100, 1, 0, 7, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_secret_vault_to_auth_link_fields() {
        let link = secret_vault_to_auth_link(VAULT_HASH, PRINCIPAL_HASH, 0x03, 1_700_000_000_000);
        assert_ne!(link.content_hash, 0);
        assert_eq!(link.vault_hash, VAULT_HASH);
        assert_eq!(link.principal_hash, PRINCIPAL_HASH);
        assert_eq!(link.permission, 0x03);
        assert_eq!(link.last_access_ts, 1_700_000_000_000);
    }

    #[test]
    fn test_secret_vault_to_notify_alert_fields() {
        let alert = secret_vault_to_notify_alert(2, VAULT_HASH, REASON, 1_700_000_000_000);
        assert_ne!(alert.content_hash, 0);
        assert_ne!(alert.reason_hash, 0);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.vault_hash, VAULT_HASH);
        assert_eq!(alert.timestamp_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_secret_vault_to_notify_alert_determinism() {
        let a = secret_vault_to_notify_alert(1, VAULT_HASH, REASON, 42);
        let b = secret_vault_to_notify_alert(1, VAULT_HASH, REASON, 42);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.reason_hash, b.reason_hash);
    }
}
