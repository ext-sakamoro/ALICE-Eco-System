//! NFC bridges — ALICE-NFC ↔ DB, Cache, Analytics, Monitor, Edge
//!
//! 5 bridges connecting the NFC reader layer to the ALICE ecosystem.
//! Covers tag-read persistence in DB, tag-data caching, tap metrics in
//! Analytics, reader-health monitoring, and payment relay via Edge.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: NFC → DB (tag read log) ────────────────────────────────────

/// Tag read log entry for ALICE-DB persistence.
///
/// Written each time an NFC tag is read so the database layer can store
/// tap history, audit trails, and NDEF payload integrity records.
pub struct NfcDbTagLog {
    /// FNV-1a hash of uid bytes and tag_type — primary deduplication key.
    pub content_hash: u64,
    /// UID hash (FNV-1a of the raw UID bytes).
    pub uid_hash: u64,
    /// Tag type: 0=Type1, 1=Type2, 2=Type3, 3=Type4, 4=ISO15693, 5=Mifare.
    pub tag_type: u8,
    /// NDEF message size in bytes (0 if no NDEF content).
    pub ndef_size: u16,
    /// Number of NDEF records in the message.
    pub record_count: u8,
    /// Reader timestamp in milliseconds since Unix epoch.
    pub timestamp_ms: u64,
    /// True when the tag passed NDEF integrity check.
    pub ndef_valid: bool,
}

/// Build a tag read log entry for ALICE-DB from raw NFC tag fields.
///
/// `uid` is the raw UID byte slice (4, 7, or 10 bytes).
#[inline]
#[must_use]
pub fn nfc_to_db_tag_log(
    uid: &[u8],
    tag_type: u8,
    ndef_size: u16,
    record_count: u8,
    timestamp_ms: u64,
    ndef_valid: bool,
) -> NfcDbTagLog {
    let uid_hash = fnv1a(uid);
    let tag_byte = tag_type.min(5);
    let mut key = [0u8; 10];
    key[0..8].copy_from_slice(&uid_hash.to_le_bytes());
    key[8] = tag_byte;
    key[9] = record_count;
    NfcDbTagLog {
        content_hash: fnv1a(&key),
        uid_hash,
        tag_type: tag_byte,
        ndef_size,
        record_count,
        timestamp_ms,
        ndef_valid,
    }
}

// ── Bridge 2: NFC → Cache (tag data cache) ───────────────────────────────

/// Tag data cache entry for ALICE-Cache.
///
/// Caches the parsed NDEF payload keyed by UID hash so repeated reads of
/// the same tag skip re-parsing.  High-capacity tags (ndef_size >= 512)
/// receive a shorter TTL to bound stale-data risk.
pub struct NfcCacheTagEntry {
    /// FNV-1a hash of uid_hash bytes — cache key.
    pub content_hash: u64,
    /// UID hash used as the primary lookup key.
    pub uid_hash: u64,
    /// NDEF payload size in bytes.
    pub ndef_size: u16,
    /// Number of NDEF records.
    pub record_count: u8,
    /// Cache TTL in seconds: 60 for large tags (>= 512 bytes), else 300.
    pub ttl_secs: u32,
    /// True when the cached entry represents a writable tag.
    pub is_writable: bool,
}

/// Build a tag data cache entry for ALICE-Cache from NFC tag fields.
///
/// TTL is computed branchlessly: large tags (ndef_size >= 512) → 60 s;
/// small tags → 300 s.
#[inline]
#[must_use]
pub fn nfc_to_cache_tag_entry(
    uid: &[u8],
    ndef_size: u16,
    record_count: u8,
    is_writable: bool,
) -> NfcCacheTagEntry {
    let uid_hash = fnv1a(uid);
    let content_hash = fnv1a(&uid_hash.to_le_bytes());
    // ブランチレス TTL: large=1 → 60s, small=0 → 300s
    let large = (ndef_size >= 512) as u32;
    let ttl_secs = 300 - large * 240;
    NfcCacheTagEntry {
        content_hash,
        uid_hash,
        ndef_size,
        record_count,
        ttl_secs,
        is_writable,
    }
}

// ── Bridge 3: NFC → Analytics (tap metrics) ──────────────────────────────

/// Tap metrics event for ALICE-Analytics.
///
/// Emitted on each successful tag read so the analytics layer can track
/// tap rates, tag-type distributions, and read-time latency histograms.
pub struct NfcAnalyticsTapEvent {
    /// FNV-1a hash of uid_hash and reader_id bytes.
    pub content_hash: u64,
    /// UID hash of the tapped tag.
    pub uid_hash: u64,
    /// Tag type (0–5, mirrors NfcDbTagLog::tag_type).
    pub tag_type: u8,
    /// Number of taps recorded for this UID in the current session.
    pub tap_count: u32,
    /// Read latency in microseconds.
    pub read_time_us: u32,
    /// Reader identifier (hardware slot index).
    pub reader_id: u8,
    /// True when the read succeeded on the first attempt.
    pub first_attempt_success: bool,
}

/// Build a tap metrics event for ALICE-Analytics from NFC read result.
#[inline]
#[must_use]
pub fn nfc_to_analytics_tap_event(
    uid: &[u8],
    tag_type: u8,
    tap_count: u32,
    read_time_us: u32,
    reader_id: u8,
    first_attempt_success: bool,
) -> NfcAnalyticsTapEvent {
    let uid_hash = fnv1a(uid);
    let mut key = [0u8; 9];
    key[0..8].copy_from_slice(&uid_hash.to_le_bytes());
    key[8] = reader_id;
    NfcAnalyticsTapEvent {
        content_hash: fnv1a(&key),
        uid_hash,
        tag_type: tag_type.min(5),
        tap_count,
        read_time_us,
        reader_id,
        first_attempt_success,
    }
}

// ── Bridge 4: NFC → Monitor (reader health) ──────────────────────────────

/// Reader health record for ALICE-Monitor.
///
/// Tracks per-reader error rates and antenna signal strength so the
/// monitoring layer can raise alerts when readers degrade.
pub struct NfcMonitorReaderHealth {
    /// FNV-1a hash of reader_id and firmware_version bytes.
    pub content_hash: u64,
    /// Reader hardware slot index.
    pub reader_id: u8,
    /// Firmware version as a packed u32 (major<<16 | minor<<8 | patch).
    pub firmware_version: u32,
    /// Total reads attempted since last reset.
    pub total_reads: u64,
    /// Total read errors since last reset.
    pub error_count: u32,
    /// Antenna signal strength in dBm (negative, e.g. -42).
    pub signal_dbm: i8,
    /// True when error_count / total_reads exceeds 5 % threshold.
    pub degraded: bool,
}

/// Build a reader health record for ALICE-Monitor.
///
/// `degraded` is set when `error_count * 20 > total_reads` (branchless).
#[inline]
#[must_use]
pub fn nfc_to_monitor_reader_health(
    reader_id: u8,
    firmware_version: u32,
    total_reads: u64,
    error_count: u32,
    signal_dbm: i8,
) -> NfcMonitorReaderHealth {
    let mut key = [0u8; 5];
    key[0] = reader_id;
    key[1..5].copy_from_slice(&firmware_version.to_le_bytes());
    let content_hash = fnv1a(&key);
    // 5% 閾値チェック: error_count * 20 > total_reads
    let degraded = (error_count as u64).saturating_mul(20) > total_reads;
    NfcMonitorReaderHealth {
        content_hash,
        reader_id,
        firmware_version,
        total_reads,
        error_count,
        signal_dbm,
        degraded,
    }
}

// ── Bridge 5: NFC → Edge (payment relay) ─────────────────────────────────

/// Payment relay payload for ALICE-Edge.
///
/// Packages an NFC tap event as an edge payment relay frame so the edge
/// layer can forward contactless payment authorisation requests to the
/// acquiring network with sub-50 ms latency.
pub struct NfcEdgePaymentRelay {
    /// FNV-1a hash of uid_hash, amount_cents, and currency bytes.
    pub content_hash: u64,
    /// UID hash of the payment-enabled tag or card.
    pub uid_hash: u64,
    /// Transaction amount in the smallest currency unit (e.g. cents).
    pub amount_cents: u64,
    /// ISO 4217 currency code as a 3-byte ASCII array (e.g. b"JPY").
    pub currency: [u8; 3],
    /// Payment protocol: 0=EMV, 1=FeliCa, 2=ISO14443A, 3=ISO14443B.
    pub protocol: u8,
    /// True when the tag supports offline authorisation.
    pub offline_capable: bool,
}

/// Build a payment relay payload for ALICE-Edge from NFC tap data.
#[inline]
#[must_use]
pub fn nfc_to_edge_payment_relay(
    uid: &[u8],
    amount_cents: u64,
    currency: [u8; 3],
    protocol: u8,
    offline_capable: bool,
) -> NfcEdgePaymentRelay {
    let uid_hash = fnv1a(uid);
    let proto = protocol.min(3);
    // 3バイト通貨コードをキーに含める
    let mut full_key = [0u8; 19];
    full_key[0..8].copy_from_slice(&uid_hash.to_le_bytes());
    full_key[8..16].copy_from_slice(&amount_cents.to_le_bytes());
    full_key[16..19].copy_from_slice(&currency);
    NfcEdgePaymentRelay {
        content_hash: fnv1a(&full_key),
        uid_hash,
        amount_cents,
        currency,
        protocol: proto,
        offline_capable,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const UID_A: &[u8] = &[0x04, 0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6];
    const UID_B: &[u8] = &[0x01, 0x02, 0x03, 0x04];

    // Bridge 1 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_nfc_to_db_tag_log_basic() {
        let log = nfc_to_db_tag_log(UID_A, 2, 128, 3, 1_700_000_000_000, true);
        assert_ne!(log.content_hash, 0);
        assert_ne!(log.uid_hash, 0);
        assert_eq!(log.tag_type, 2);
        assert_eq!(log.ndef_size, 128);
        assert_eq!(log.record_count, 3);
        assert!(log.ndef_valid);
    }

    #[test]
    fn test_nfc_to_db_tag_log_type_clamped() {
        let log = nfc_to_db_tag_log(UID_B, 99, 0, 0, 0, false);
        // tag_type が 5 に丸められること
        assert_eq!(log.tag_type, 5);
        assert!(!log.ndef_valid);
    }

    // Bridge 2 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_nfc_to_cache_tag_entry_small_ttl() {
        // ndef_size < 512 → ttl = 300
        let entry = nfc_to_cache_tag_entry(UID_A, 256, 2, false);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
        assert!(!entry.is_writable);
    }

    #[test]
    fn test_nfc_to_cache_tag_entry_large_ttl() {
        // ndef_size >= 512 → ttl = 60
        let entry = nfc_to_cache_tag_entry(UID_B, 512, 1, true);
        assert_eq!(entry.ttl_secs, 60);
        assert!(entry.is_writable);
    }

    // Bridge 3 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_nfc_to_analytics_tap_event() {
        let ev = nfc_to_analytics_tap_event(UID_A, 1, 42, 350, 0, true);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.tag_type, 1);
        assert_eq!(ev.tap_count, 42);
        assert_eq!(ev.read_time_us, 350);
        assert!(ev.first_attempt_success);
    }

    #[test]
    fn test_nfc_to_analytics_tap_event_hash_determinism() {
        let ev1 = nfc_to_analytics_tap_event(UID_B, 0, 1, 100, 2, false);
        let ev2 = nfc_to_analytics_tap_event(UID_B, 0, 1, 100, 2, false);
        assert_eq!(ev1.content_hash, ev2.content_hash);
        assert_eq!(ev1.uid_hash, ev2.uid_hash);
    }

    // Bridge 4 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_nfc_to_monitor_reader_health_ok() {
        // error_count * 20 <= total_reads → not degraded
        let h = nfc_to_monitor_reader_health(0, 0x0001_0200, 1000, 10, -42);
        assert_ne!(h.content_hash, 0);
        assert!(!h.degraded); // 10*20=200 <= 1000
        assert_eq!(h.signal_dbm, -42);
    }

    #[test]
    fn test_nfc_to_monitor_reader_health_degraded() {
        // error_count * 20 > total_reads → degraded
        let h = nfc_to_monitor_reader_health(1, 0x0001_0200, 100, 10, -80);
        assert!(h.degraded); // 10*20=200 > 100
    }

    // Bridge 5 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_nfc_to_edge_payment_relay_basic() {
        let relay = nfc_to_edge_payment_relay(UID_A, 1500, *b"JPY", 1, false);
        assert_ne!(relay.content_hash, 0);
        assert_eq!(relay.amount_cents, 1500);
        assert_eq!(&relay.currency, b"JPY");
        assert_eq!(relay.protocol, 1);
        assert!(!relay.offline_capable);
    }

    #[test]
    fn test_nfc_to_edge_payment_relay_protocol_clamped() {
        let relay = nfc_to_edge_payment_relay(UID_B, 0, *b"USD", 99, true);
        assert_eq!(relay.protocol, 3);
        assert!(relay.offline_capable);
    }
}
