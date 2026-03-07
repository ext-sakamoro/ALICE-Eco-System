//! DataShield bridges — ALICE-DataShield ↔ DB, Analytics, Cache, Crypto, Edge
//!
//! 5 bridges connecting ALICE-DataShield (data masking, k-anonymity,
//! differential privacy) to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: DataShield → DB (anonymized record persistence) ────────────

/// Anonymized record for ALICE-DB persistence.
///
/// Stores a masked and generalized representation of a sensitive record,
/// ready for safe storage in a multi-tenant database.
pub struct DataShieldDbRecord {
    /// FNV-1a hash of the original field name — DB row key.
    pub content_hash: u64,
    /// FNV-1a hash of the masked value — deduplication fingerprint.
    pub masked_value_hash: u64,
    /// Byte length of the masked value string.
    pub masked_byte_len: usize,
    /// Byte length of the original value before masking.
    pub original_byte_len: usize,
    /// Generalization bucket lower bound (×1000, stored as integer to avoid f64 in DB).
    pub bucket_lo_milli: i64,
    /// Generalization bucket upper bound (×1000).
    pub bucket_hi_milli: i64,
}

/// Build an anonymized DB record from masking and generalization outputs.
///
/// # Optimization notes
/// - Bucket bounds are stored as integer milliunits (`f64 * 1000.0` cast to `i64`)
///   to avoid floating-point serialization overhead on the DB bridge path.
/// - Both hashes are computed in separate FNV passes; no heap allocation.
#[inline]
#[must_use]
pub fn datashield_to_db_record(
    field_name: &str,
    masked_value: &str,
    original_byte_len: usize,
    bucket_lo: f64,
    bucket_hi: f64,
) -> DataShieldDbRecord {
    let content_hash = fnv1a(field_name.as_bytes());
    let masked_value_hash = fnv1a(masked_value.as_bytes());

    // Store bucket bounds as integer milliunits to avoid f64 in the record.
    // Multiply by 1000 then cast; saturating cast avoids UB on out-of-range f64.
    let bucket_lo_milli = (bucket_lo * 1_000.0) as i64;
    let bucket_hi_milli = (bucket_hi * 1_000.0) as i64;

    DataShieldDbRecord {
        content_hash,
        masked_value_hash,
        masked_byte_len: masked_value.len(),
        original_byte_len,
        bucket_lo_milli,
        bucket_hi_milli,
    }
}

// ── Bridge 2: DataShield → Analytics (privacy metrics) ───────────────────

/// Privacy metrics event for ALICE-Analytics.
///
/// Tracks differential privacy noise injection and k-anonymity group sizes
/// so the analytics layer can monitor privacy budget consumption.
pub struct DataShieldAnalyticsEvent {
    /// FNV-1a hash of the dataset identifier — analytics stream key.
    pub content_hash: u64,
    /// True count before differential privacy noise was added.
    pub true_count: u64,
    /// Noisy count after Laplace noise injection (×1000 as integer).
    pub noisy_count_milli: i64,
    /// Epsilon privacy parameter (×10000 as integer for lossless storage).
    pub epsilon_micro: u64,
    /// Minimum equivalence class size (k) in the anonymized dataset.
    pub k_min: u32,
    /// Number of equivalence classes in the dataset.
    pub class_count: u32,
    /// True when k-anonymity requirement is satisfied (k_min >= k_threshold).
    pub k_satisfied: u8,
}

/// Build a privacy metrics analytics event from DP and k-anonymity outputs.
///
/// # Optimization notes
/// - `noisy_count_milli` stores `noisy_count * 1000` as i64 for precision
///   without floating-point serialization cost.
/// - `k_satisfied` is a branchless u8 cast from bool.
/// - `epsilon_micro` stores `epsilon * 10000` as u64 for lossless integer storage.
#[inline]
#[must_use]
pub fn datashield_to_analytics_event(
    dataset_id: &str,
    true_count: u64,
    noisy_count: f64,
    epsilon: f64,
    k_min: u32,
    class_count: u32,
    k_threshold: u32,
) -> DataShieldAnalyticsEvent {
    let content_hash = fnv1a(dataset_id.as_bytes());
    let noisy_count_milli = (noisy_count * 1_000.0) as i64;
    let epsilon_micro = (epsilon * 10_000.0) as u64;
    // Branchless satisfaction check: cast bool to u8.
    let k_satisfied = (k_min >= k_threshold) as u8;

    DataShieldAnalyticsEvent {
        content_hash,
        true_count,
        noisy_count_milli,
        epsilon_micro,
        k_min,
        class_count,
        k_satisfied,
    }
}

// ── Bridge 3: DataShield → Cache (mask rule cache) ────────────────────────

/// Mask rule cache entry for ALICE-Cache.
///
/// Caches pre-computed masking parameters keyed by field name so that
/// repeated mask rule lookups are avoided on the hot path.
pub struct DataShieldCacheEntry {
    /// FNV-1a hash of the field name — cache lookup key.
    pub content_hash: u64,
    /// Keep-prefix length used for masking this field type.
    pub keep_prefix: u8,
    /// Mask character code (ASCII); typically b'*' = 42.
    pub mask_char: u8,
    /// Field type hint: 0=generic, 1=email, 2=card, 3=phone.
    pub field_type: u8,
    /// Cache TTL in seconds.
    ///
    /// More sensitive field types get shorter TTL via branchless formula:
    /// `base - field_type_clamped * step`.
    pub ttl_seconds: u32,
}

/// Build a mask rule cache entry for a named field.
///
/// # Optimization notes
/// - TTL uses branchless arithmetic: `base - min(field_type, 3) * step`.
///   More sensitive types (higher field_type) expire sooner.
/// - All fields are integer/byte — no heap allocation.
#[inline]
#[must_use]
pub fn datashield_to_cache_entry(
    field_name: &str,
    keep_prefix: u8,
    mask_char: u8,
    field_type: u8,
) -> DataShieldCacheEntry {
    // Branchless TTL: base=3600, step=600 per sensitivity level, max 3 levels.
    // field_type 0 (generic) → 3600; field_type 3 (phone) → 1800.
    const BASE: u32 = 3_600;
    const STEP: u32 = 600;
    const MAX_TYPE: u8 = 3;

    let content_hash = fnv1a(field_name.as_bytes());
    let clamped = field_type.min(MAX_TYPE) as u32;
    let ttl_seconds = BASE - clamped * STEP;

    DataShieldCacheEntry {
        content_hash,
        keep_prefix,
        mask_char,
        field_type,
        ttl_seconds,
    }
}

// ── Bridge 4: DataShield → Crypto (encryption integration) ───────────────

/// Encryption integration record for ALICE-Crypto.
///
/// After masking, remaining sensitive fragments that cannot be generalized
/// are handed to the Crypto layer for authenticated encryption before storage.
pub struct DataShieldCryptoRecord {
    /// FNV-1a hash of the plaintext field value — integrity fingerprint.
    pub content_hash: u64,
    /// FNV-1a hash of the field name — identifies the encryption context.
    pub field_hash: u64,
    /// Byte length of the value to encrypt.
    pub plaintext_byte_len: usize,
    /// Recommended cipher: 0=AES-256-GCM, 1=ChaCha20-Poly1305.
    pub cipher: u8,
    /// Data classification level: 0=public, 1=internal, 2=confidential, 3=restricted.
    pub classification: u8,
}

/// Build a crypto integration record from a sensitive field value.
///
/// # Optimization notes
/// - cipher selection: `classification >= 3` selects ChaCha20 (cipher=1)
///   branchlessly via `(classification >= 3) as u8`.
/// - Two FNV passes (field name + value) — no heap allocation.
#[inline]
#[must_use]
pub fn datashield_to_crypto_record(
    field_name: &str,
    field_value: &str,
    classification: u8,
) -> DataShieldCryptoRecord {
    let content_hash = fnv1a(field_value.as_bytes());
    let field_hash = fnv1a(field_name.as_bytes());
    // Branchless cipher selection: restricted fields use ChaCha20 (cipher=1).
    let cipher = (classification >= 3) as u8;

    DataShieldCryptoRecord {
        content_hash,
        field_hash,
        plaintext_byte_len: field_value.len(),
        cipher,
        classification,
    }
}

// ── Bridge 5: DataShield → Edge (privacy violation events) ───────────────

/// Privacy violation event for ALICE-Edge forwarding.
///
/// Emitted when a k-anonymity or differential privacy violation is detected
/// so the edge layer can rate-limit or block the offending request.
pub struct DataShieldEdgeEvent {
    /// FNV-1a hash of the dataset or request identifier — edge routing key.
    pub content_hash: u64,
    /// Violation type: 0=k_anonymity_fail, 1=dp_budget_exceeded, 2=unmasked_pii.
    pub violation_type: u8,
    /// Severity: 0=low, 1=medium, 2=high, 3=critical.
    pub severity: u8,
    /// k value at violation time (0 when not applicable).
    pub k_value: u32,
    /// Event timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a privacy violation edge event.
///
/// # Optimization notes
/// - severity is derived branchlessly: `min(violation_type, 3)` as a
///   conservative default; callers may override by passing the mapped value.
/// - content_hash is computed over `dataset_id` bytes in one FNV pass.
#[inline]
#[must_use]
pub fn datashield_to_edge_event(
    dataset_id: &str,
    violation_type: u8,
    k_value: u32,
    timestamp_ms: u64,
) -> DataShieldEdgeEvent {
    let content_hash = fnv1a(dataset_id.as_bytes());
    // Branchless severity: violation_type maps 0→0, 1→1, 2→2, clamp at 3.
    let severity = violation_type.min(3);

    DataShieldEdgeEvent {
        content_hash,
        violation_type,
        severity,
        k_value,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const FIELD_NAME: &str = "email";
    const MASKED_VALUE: &str = "a****@example.com";
    const DATASET_ID: &str = "user-dataset-2026";

    #[test]
    fn test_db_record_basic() {
        let rec = datashield_to_db_record(FIELD_NAME, MASKED_VALUE, 20, 20.0, 30.0);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.masked_value_hash, 0);
        assert_ne!(rec.content_hash, rec.masked_value_hash);
        assert_eq!(rec.masked_byte_len, MASKED_VALUE.len());
        assert_eq!(rec.original_byte_len, 20);
        assert_eq!(rec.bucket_lo_milli, 20_000);
        assert_eq!(rec.bucket_hi_milli, 30_000);
    }

    #[test]
    fn test_db_record_hash_determinism() {
        let a = datashield_to_db_record(FIELD_NAME, MASKED_VALUE, 20, 0.0, 10.0);
        let b = datashield_to_db_record(FIELD_NAME, MASKED_VALUE, 20, 0.0, 10.0);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.masked_value_hash, b.masked_value_hash);
    }

    #[test]
    fn test_analytics_event_k_satisfied() {
        let ev = datashield_to_analytics_event(DATASET_ID, 1000, 1001.5, 1.0, 5, 10, 3);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.true_count, 1000);
        assert_eq!(ev.noisy_count_milli, 1_001_500);
        assert_eq!(ev.epsilon_micro, 10_000); // 1.0 * 10000
        assert_eq!(ev.k_min, 5);
        assert_eq!(ev.class_count, 10);
        assert_eq!(ev.k_satisfied, 1); // 5 >= 3
    }

    #[test]
    fn test_analytics_event_k_not_satisfied() {
        let ev = datashield_to_analytics_event(DATASET_ID, 500, 500.0, 0.5, 2, 5, 5);
        assert_eq!(ev.k_satisfied, 0); // 2 < 5
    }

    #[test]
    fn test_cache_entry_ttl_branchless() {
        // field_type=0 (generic) → TTL = 3600 - 0*600 = 3600.
        let e0 = datashield_to_cache_entry("product_id", 4, b'*', 0);
        assert_ne!(e0.content_hash, 0);
        assert_eq!(e0.ttl_seconds, 3_600);

        // field_type=2 (card) → TTL = 3600 - 2*600 = 2400.
        let e2 = datashield_to_cache_entry("card_number", 0, b'*', 2);
        assert_eq!(e2.ttl_seconds, 2_400);

        // field_type=3 (phone) → TTL = 3600 - 3*600 = 1800.
        let e3 = datashield_to_cache_entry("phone", 1, b'*', 3);
        assert_eq!(e3.ttl_seconds, 1_800);

        // field_type=99 → clamped to 3 → TTL = 1800.
        let e99 = datashield_to_cache_entry("secret", 0, b'#', 99);
        assert_eq!(e99.ttl_seconds, 1_800);
    }

    #[test]
    fn test_crypto_record_cipher_selection() {
        // classification < 3 → cipher = 0 (AES-GCM).
        let rec_low = datashield_to_crypto_record(FIELD_NAME, "alice@example.com", 2);
        assert_ne!(rec_low.content_hash, 0);
        assert_ne!(rec_low.field_hash, 0);
        assert_eq!(rec_low.cipher, 0);
        assert_eq!(rec_low.classification, 2);

        // classification = 3 → cipher = 1 (ChaCha20).
        let rec_high = datashield_to_crypto_record("ssn", "123-45-6789", 3);
        assert_eq!(rec_high.cipher, 1);
    }

    #[test]
    fn test_crypto_record_byte_len() {
        let value = "4111111111111111";
        let rec = datashield_to_crypto_record("card", value, 3);
        assert_eq!(rec.plaintext_byte_len, value.len());
    }

    #[test]
    fn test_edge_event_severity_branchless() {
        // violation_type=0 → severity=0.
        let e0 = datashield_to_edge_event(DATASET_ID, 0, 3, 1_700_000_000_000);
        assert_ne!(e0.content_hash, 0);
        assert_eq!(e0.severity, 0);
        assert_eq!(e0.k_value, 3);

        // violation_type=2 → severity=2.
        let e2 = datashield_to_edge_event(DATASET_ID, 2, 0, 0);
        assert_eq!(e2.severity, 2);

        // violation_type=10 → clamped to 3.
        let e10 = datashield_to_edge_event(DATASET_ID, 10, 0, 0);
        assert_eq!(e10.severity, 3);
    }

    #[test]
    fn test_edge_event_hash_determinism() {
        let a = datashield_to_edge_event(DATASET_ID, 1, 5, 42);
        let b = datashield_to_edge_event(DATASET_ID, 1, 5, 42);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
