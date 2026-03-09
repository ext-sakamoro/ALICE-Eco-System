//! Payment bridges — ALICE-Payment ↔ DB, Cache, Analytics, Ledger, Notify
//!
//! 5 bridges connecting payment transaction processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Payment → DB (transaction storage) ─────────────────────────

/// Transaction storage record for ALICE-DB persistence.
pub struct PaymentDbRecord {
    /// Content hash over the transaction metadata.
    pub content_hash: u64,
    /// Transaction amount in minor currency units (e.g. cents).
    pub amount_minor: u64,
    /// Hash of the ISO 4217 currency code.
    pub currency_hash: u64,
    /// Hash of the merchant identifier.
    pub merchant_hash: u64,
    /// Transaction status byte (0=pending, 1=success, 2=failed, 3=refunded).
    pub status: u8,
    /// Transaction timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize payment transaction metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn payment_to_db_record(
    amount_minor: u64,
    currency_hash: u64,
    merchant_hash: u64,
    status: u8,
    timestamp_ms: u64,
) -> PaymentDbRecord {
    let mut buf = [0u8; 33];
    buf[0..8].copy_from_slice(&amount_minor.to_le_bytes());
    buf[8..16].copy_from_slice(&currency_hash.to_le_bytes());
    buf[16..24].copy_from_slice(&merchant_hash.to_le_bytes());
    buf[24] = status;
    buf[25..33].copy_from_slice(&timestamp_ms.to_le_bytes());
    PaymentDbRecord {
        content_hash: fnv1a(&buf),
        amount_minor,
        currency_hash,
        merchant_hash,
        status,
        timestamp_ms,
    }
}

// ── Bridge 2: Payment → Cache (session cache) ────────────────────────────

/// Payment session cache entry for ALICE-Cache.
pub struct PaymentCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Hash of the payment session identifier.
    pub session_hash: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Transaction amount in minor currency units.
    pub amount_minor: u64,
    /// Transaction status byte.
    pub status: u8,
}

/// Build a payment session cache entry for ALICE-Cache.
///
/// Pending sessions (status == 0) receive a short TTL (30 s) to avoid stale
/// locks; completed sessions (status != 0) get 300 s for audit access.
#[inline]
#[must_use]
pub fn payment_to_cache_entry(
    session_hash: u64,
    amount_minor: u64,
    status: u8,
) -> PaymentCacheEntry {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&session_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&amount_minor.to_le_bytes());
    buf[16] = status;
    let is_pending = (status == 0) as u32;
    let ttl_secs = 300 - is_pending * 270;
    PaymentCacheEntry {
        content_hash: fnv1a(&buf),
        session_hash,
        ttl_secs,
        amount_minor,
        status,
    }
}

// ── Bridge 3: Payment → Analytics (transaction event) ────────────────────

/// Payment analytics event for ALICE-Analytics ingestion.
pub struct PaymentAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Total transaction count in this window.
    pub tx_count: u64,
    /// Total transaction value in minor currency units.
    pub total_minor: u64,
    /// Success rate in basis points.
    pub success_rate_bps: u16,
    /// Average processing latency in milliseconds.
    pub avg_latency_ms: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a payment analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn payment_to_analytics_event(
    tx_count: u64,
    total_minor: u64,
    success_rate_bps: u16,
    avg_latency_ms: u32,
    timestamp_ms: u64,
) -> PaymentAnalyticsEvent {
    let mut buf = [0u8; 30];
    buf[0..8].copy_from_slice(&tx_count.to_le_bytes());
    buf[8..16].copy_from_slice(&total_minor.to_le_bytes());
    buf[16..18].copy_from_slice(&success_rate_bps.to_le_bytes());
    buf[18..22].copy_from_slice(&avg_latency_ms.to_le_bytes());
    buf[22..30].copy_from_slice(&timestamp_ms.to_le_bytes());
    PaymentAnalyticsEvent {
        content_hash: fnv1a(&buf),
        tx_count,
        total_minor,
        success_rate_bps,
        avg_latency_ms,
        timestamp_ms,
    }
}

// ── Bridge 4: Payment → Ledger (double-entry) ────────────────────────────

/// Double-entry ledger record for ALICE-Ledger integration.
pub struct PaymentLedgerEntry {
    /// Content hash over the ledger entry.
    pub content_hash: u64,
    /// Transaction amount in minor currency units.
    pub amount_minor: u64,
    /// Hash of the ISO 4217 currency code.
    pub currency_hash: u64,
    /// Hash of the debit account identifier.
    pub debit_account_hash: u64,
    /// Hash of the credit account identifier.
    pub credit_account_hash: u64,
}

/// Build a double-entry payment ledger record for ALICE-Ledger.
#[inline]
#[must_use]
pub fn payment_to_ledger_entry(
    amount_minor: u64,
    currency_hash: u64,
    debit_account_hash: u64,
    credit_account_hash: u64,
) -> PaymentLedgerEntry {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&amount_minor.to_le_bytes());
    buf[8..16].copy_from_slice(&currency_hash.to_le_bytes());
    buf[16..24].copy_from_slice(&debit_account_hash.to_le_bytes());
    buf[24..32].copy_from_slice(&credit_account_hash.to_le_bytes());
    PaymentLedgerEntry {
        content_hash: fnv1a(&buf),
        amount_minor,
        currency_hash,
        debit_account_hash,
        credit_account_hash,
    }
}

// ── Bridge 5: Payment → Notify (alert) ───────────────────────────────────

/// Payment alert notification for ALICE-Notify.
pub struct PaymentNotifyAlert {
    /// Content hash over the alert tuple.
    pub content_hash: u64,
    /// Severity level (0=info, 1=warn, 2=critical).
    pub severity: u8,
    /// Transaction amount in minor currency units.
    pub amount_minor: u64,
    /// Hash of the merchant identifier.
    pub merchant_hash: u64,
    /// Hash of the human-readable alert reason.
    pub reason_hash: u64,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a payment alert notification for ALICE-Notify.
#[inline]
#[must_use]
pub fn payment_to_notify_alert(
    severity: u8,
    amount_minor: u64,
    merchant_hash: u64,
    reason_hash: u64,
    timestamp_ms: u64,
) -> PaymentNotifyAlert {
    let mut buf = [0u8; 33];
    buf[0] = severity;
    buf[1..9].copy_from_slice(&amount_minor.to_le_bytes());
    buf[9..17].copy_from_slice(&merchant_hash.to_le_bytes());
    buf[17..25].copy_from_slice(&reason_hash.to_le_bytes());
    buf[25..33].copy_from_slice(&timestamp_ms.to_le_bytes());
    PaymentNotifyAlert {
        content_hash: fnv1a(&buf),
        severity,
        amount_minor,
        merchant_hash,
        reason_hash,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_to_db_record_hash_nonzero() {
        let rec = payment_to_db_record(10_000, 0x1111, 0x2222, 1, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_payment_to_db_record_fields() {
        let rec = payment_to_db_record(500_00, 0xaaaa, 0xbbbb, 2, 999_999);
        assert_eq!(rec.amount_minor, 500_00);
        assert_eq!(rec.currency_hash, 0xaaaa);
        assert_eq!(rec.merchant_hash, 0xbbbb);
        assert_eq!(rec.status, 2);
        assert_eq!(rec.timestamp_ms, 999_999);
    }

    #[test]
    fn test_payment_to_db_record_deterministic() {
        let a = payment_to_db_record(1, 2, 3, 0, 5);
        let b = payment_to_db_record(1, 2, 3, 0, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_payment_to_cache_entry_pending_ttl() {
        let entry = payment_to_cache_entry(0x1234, 100, 0);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 30);
        assert_eq!(entry.status, 0);
    }

    #[test]
    fn test_payment_to_cache_entry_completed_ttl() {
        let entry = payment_to_cache_entry(0x5678, 200, 1);
        assert_eq!(entry.ttl_secs, 300);
        assert_eq!(entry.status, 1);
    }

    #[test]
    fn test_payment_to_analytics_event() {
        let ev = payment_to_analytics_event(1_000, 500_000, 9_800, 150, 1_700_000_000_001);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.tx_count, 1_000);
        assert_eq!(ev.success_rate_bps, 9_800);
        assert_eq!(ev.avg_latency_ms, 150);
    }

    #[test]
    fn test_payment_to_ledger_entry() {
        let entry = payment_to_ledger_entry(1_000_00, 0xc1c1, 0xd1d1, 0xe1e1);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.amount_minor, 1_000_00);
        assert_eq!(entry.debit_account_hash, 0xd1d1);
        assert_eq!(entry.credit_account_hash, 0xe1e1);
    }

    #[test]
    fn test_payment_to_notify_alert() {
        let alert = payment_to_notify_alert(2, 99_999, 0xf1f1, 0xa1a1, 1_700_000_000_002);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.amount_minor, 99_999);
        assert_eq!(alert.merchant_hash, 0xf1f1);
    }
}
