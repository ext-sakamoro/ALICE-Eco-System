//! PKI bridges — ALICE-PKI ↔ DB, Cache, Analytics, Auth, Notify
//!
//! 5 bridges connecting public key infrastructure management to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: PKI → DB (certificate storage) ─────────────────────────────

/// Certificate storage record for ALICE-DB persistence.
pub struct PkiDbRecord {
    /// Content hash over the certificate metadata.
    pub content_hash: u64,
    /// Total number of certificates managed.
    pub cert_count: u32,
    /// Number of Certificate Authorities.
    pub ca_count: u16,
    /// Certificate Revocation List size in bytes.
    pub crl_size: u64,
    /// Key size in bits (e.g. 2048, 4096).
    pub key_bits: u16,
    /// Hash of the signature algorithm OID.
    pub algorithm_hash: u64,
}

/// Serialize PKI certificate metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn pki_to_db_record(
    cert_count: u32,
    ca_count: u16,
    crl_size: u64,
    key_bits: u16,
    algorithm_hash: u64,
) -> PkiDbRecord {
    let mut buf = [0u8; 24];
    buf[0..4].copy_from_slice(&cert_count.to_le_bytes());
    buf[4..6].copy_from_slice(&ca_count.to_le_bytes());
    buf[6..14].copy_from_slice(&crl_size.to_le_bytes());
    buf[14..16].copy_from_slice(&key_bits.to_le_bytes());
    buf[16..24].copy_from_slice(&algorithm_hash.to_le_bytes());
    PkiDbRecord {
        content_hash: fnv1a(&buf),
        cert_count,
        ca_count,
        crl_size,
        key_bits,
        algorithm_hash,
    }
}

// ── Bridge 2: PKI → Cache (certificate cache) ────────────────────────────

/// Certificate validation cache entry for ALICE-Cache.
pub struct PkiCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Hash of the certificate DER bytes.
    pub cert_hash: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Certificate expiry as Unix timestamp.
    pub not_after_ts: u64,
    /// Whether the certificate is revoked.
    pub is_revoked: bool,
}

/// Build a certificate validation cache entry for ALICE-Cache.
///
/// Revoked certificates get a short TTL (30 s) to force re-validation;
/// valid certificates get 3600 s.
#[inline]
#[must_use]
pub fn pki_to_cache_entry(cert_hash: u64, not_after_ts: u64, is_revoked: bool) -> PkiCacheEntry {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&cert_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&not_after_ts.to_le_bytes());
    buf[16] = is_revoked as u8;
    let revoked_flag = is_revoked as u32;
    let ttl_secs = 3_600 - revoked_flag * 3_570;
    PkiCacheEntry {
        content_hash: fnv1a(&buf),
        cert_hash,
        ttl_secs,
        not_after_ts,
        is_revoked,
    }
}

// ── Bridge 3: PKI → Analytics (certificate event) ────────────────────────

/// Certificate lifecycle analytics event for ALICE-Analytics.
pub struct PkiAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of certificates issued in this window.
    pub cert_issued: u32,
    /// Number of certificates revoked in this window.
    pub cert_revoked: u32,
    /// Total validation requests processed.
    pub validation_count: u64,
    /// Average certificate chain length.
    pub avg_chain_len: u8,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a PKI certificate lifecycle analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn pki_to_analytics_event(
    cert_issued: u32,
    cert_revoked: u32,
    validation_count: u64,
    avg_chain_len: u8,
    timestamp_ms: u64,
) -> PkiAnalyticsEvent {
    let mut buf = [0u8; 25];
    buf[0..4].copy_from_slice(&cert_issued.to_le_bytes());
    buf[4..8].copy_from_slice(&cert_revoked.to_le_bytes());
    buf[8..16].copy_from_slice(&validation_count.to_le_bytes());
    buf[16] = avg_chain_len;
    buf[17..25].copy_from_slice(&timestamp_ms.to_le_bytes());
    PkiAnalyticsEvent {
        content_hash: fnv1a(&buf),
        cert_issued,
        cert_revoked,
        validation_count,
        avg_chain_len,
        timestamp_ms,
    }
}

// ── Bridge 4: PKI → Auth (certificate link) ──────────────────────────────

/// Certificate auth link for ALICE-Auth TLS integration.
pub struct PkiAuthLink {
    /// Content hash over the certificate binding.
    pub content_hash: u64,
    /// Hash of the certificate DER bytes.
    pub cert_hash: u64,
    /// Hash of the subject distinguished name.
    pub subject_hash: u64,
    /// Hash of the issuer distinguished name.
    pub issuer_hash: u64,
    /// Key usage bit field.
    pub key_usage: u16,
}

/// Build a PKI certificate auth link for ALICE-Auth.
#[inline]
#[must_use]
pub fn pki_to_auth_link(
    cert_hash: u64,
    subject_hash: u64,
    issuer_hash: u64,
    key_usage: u16,
) -> PkiAuthLink {
    let mut buf = [0u8; 26];
    buf[0..8].copy_from_slice(&cert_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&subject_hash.to_le_bytes());
    buf[16..24].copy_from_slice(&issuer_hash.to_le_bytes());
    buf[24..26].copy_from_slice(&key_usage.to_le_bytes());
    PkiAuthLink {
        content_hash: fnv1a(&buf),
        cert_hash,
        subject_hash,
        issuer_hash,
        key_usage,
    }
}

// ── Bridge 5: PKI → Notify (expiry alert) ────────────────────────────────

/// Certificate expiry alert notification for ALICE-Notify.
pub struct PkiNotifyAlert {
    /// Content hash over the alert tuple.
    pub content_hash: u64,
    /// Severity level (0=info, 1=warn, 2=critical).
    pub severity: u8,
    /// Hash of the expiring certificate.
    pub cert_hash: u64,
    /// Days remaining until certificate expiry.
    pub days_to_expiry: u32,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a PKI certificate expiry alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn pki_to_notify_alert(
    severity: u8,
    cert_hash: u64,
    days_to_expiry: u32,
    timestamp_ms: u64,
) -> PkiNotifyAlert {
    let mut buf = [0u8; 21];
    buf[0] = severity;
    buf[1..9].copy_from_slice(&cert_hash.to_le_bytes());
    buf[9..13].copy_from_slice(&days_to_expiry.to_le_bytes());
    buf[13..21].copy_from_slice(&timestamp_ms.to_le_bytes());
    PkiNotifyAlert {
        content_hash: fnv1a(&buf),
        severity,
        cert_hash,
        days_to_expiry,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pki_to_db_record_hash_nonzero() {
        let rec = pki_to_db_record(500, 3, 65_536, 4_096, 0x1234);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_pki_to_db_record_fields() {
        let rec = pki_to_db_record(200, 2, 32_768, 2_048, 0xabcd);
        assert_eq!(rec.cert_count, 200);
        assert_eq!(rec.ca_count, 2);
        assert_eq!(rec.crl_size, 32_768);
        assert_eq!(rec.key_bits, 2_048);
        assert_eq!(rec.algorithm_hash, 0xabcd);
    }

    #[test]
    fn test_pki_to_db_record_deterministic() {
        let a = pki_to_db_record(1, 2, 3, 4, 5);
        let b = pki_to_db_record(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_pki_to_cache_entry_valid_ttl() {
        let entry = pki_to_cache_entry(0xdead, 1_800_000_000, false);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 3_600);
        assert!(!entry.is_revoked);
    }

    #[test]
    fn test_pki_to_cache_entry_revoked_ttl() {
        let entry = pki_to_cache_entry(0xbeef, 1_700_000_000, true);
        assert_eq!(entry.ttl_secs, 30);
        assert!(entry.is_revoked);
    }

    #[test]
    fn test_pki_to_analytics_event() {
        let ev = pki_to_analytics_event(10, 1, 5_000, 3, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.cert_issued, 10);
        assert_eq!(ev.cert_revoked, 1);
        assert_eq!(ev.avg_chain_len, 3);
    }

    #[test]
    fn test_pki_to_auth_link() {
        let link = pki_to_auth_link(0x1111, 0x2222, 0x3333, 0b0000_0011);
        assert_ne!(link.content_hash, 0);
        assert_eq!(link.cert_hash, 0x1111);
        assert_eq!(link.key_usage, 0b0000_0011);
    }

    #[test]
    fn test_pki_to_notify_alert() {
        let alert = pki_to_notify_alert(1, 0x4444, 14, 1_700_000_000_001);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.severity, 1);
        assert_eq!(alert.days_to_expiry, 14);
    }
}
