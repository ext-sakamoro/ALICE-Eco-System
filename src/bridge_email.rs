//! Email bridges — Email ↔ DB, Cache, Analytics, Queue, Notify
//!
//! 5 bridges connecting email data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Email → DB (message record persistence) ────────────────────

/// Email message record for ALICE-DB persistence.
pub struct EmailDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Sender address hash.
    pub sender_hash: u64,
    /// Number of recipients.
    pub recipient_count: u32,
    /// Message body size in bytes.
    pub body_bytes: u64,
    /// Whether the message has attachments.
    pub has_attachment: bool,
    /// Message timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize email message data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn email_to_db_record(
    sender_hash: u64,
    recipient_count: u32,
    body_bytes: u64,
    has_attachment: bool,
    timestamp_ms: u64,
) -> EmailDbRecord {
    // buf: sender_hash(8) + recipient_count(4) + body_bytes(8) + has_attachment(1) + timestamp_ms(8) = 29
    let mut buf = [0u8; 29];
    buf[0..8].copy_from_slice(&sender_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&recipient_count.to_le_bytes());
    buf[12..20].copy_from_slice(&body_bytes.to_le_bytes());
    buf[20] = has_attachment as u8;
    buf[21..29].copy_from_slice(&timestamp_ms.to_le_bytes());
    EmailDbRecord {
        content_hash: fnv1a(&buf),
        sender_hash,
        recipient_count,
        body_bytes,
        has_attachment,
        timestamp_ms,
    }
}

// ── Bridge 2: Email → Cache (thread cache entry) ──────────────────────────

/// Email thread cache entry for ALICE-Cache.
pub struct EmailCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Thread identifier hash.
    pub thread_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of messages in the thread.
    pub msg_count: u32,
    /// Number of unread messages.
    pub unread_count: u32,
}

/// Build email thread cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn email_to_cache_entry(
    thread_hash: u64,
    ttl_secs: u32,
    msg_count: u32,
    unread_count: u32,
) -> EmailCacheEntry {
    // buf: thread_hash(8) + msg_count(4) + unread_count(4) = 16
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&thread_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&msg_count.to_le_bytes());
    buf[12..16].copy_from_slice(&unread_count.to_le_bytes());
    EmailCacheEntry {
        content_hash: fnv1a(&buf),
        thread_hash,
        ttl_secs,
        msg_count,
        unread_count,
    }
}

// ── Bridge 3: Email → Analytics (delivery analytics event) ───────────────

/// Email delivery analytics event for ALICE-Analytics ingestion.
pub struct EmailAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Total sent message count.
    pub sent_count: u64,
    /// Total received message count.
    pub received_count: u64,
    /// Bounce rate in basis points (0–10000).
    pub bounce_rate_bps: u16,
    /// Open rate in basis points (0–10000).
    pub open_rate_bps: u16,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build email delivery analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn email_to_analytics_event(
    sent_count: u64,
    received_count: u64,
    bounce_rate_bps: u16,
    open_rate_bps: u16,
    timestamp_ms: u64,
) -> EmailAnalyticsEvent {
    // buf: sent_count(8) + received_count(8) + bounce_rate_bps(2) + open_rate_bps(2) + timestamp_ms(8) = 28
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&sent_count.to_le_bytes());
    buf[8..16].copy_from_slice(&received_count.to_le_bytes());
    buf[16..18].copy_from_slice(&bounce_rate_bps.to_le_bytes());
    buf[18..20].copy_from_slice(&open_rate_bps.to_le_bytes());
    buf[20..28].copy_from_slice(&timestamp_ms.to_le_bytes());
    EmailAnalyticsEvent {
        content_hash: fnv1a(&buf),
        sent_count,
        received_count,
        bounce_rate_bps,
        open_rate_bps,
        timestamp_ms,
    }
}

// ── Bridge 4: Email → Queue (outbound message queue entry) ────────────────

/// Email outbound message queue entry for ALICE-Queue.
pub struct EmailQueueEntry {
    /// Content hash.
    pub content_hash: u64,
    /// Sender address hash.
    pub sender_hash: u64,
    /// Recipient address hash.
    pub recipient_hash: u64,
    /// Delivery priority (0 = low, 1 = normal, 2 = high).
    pub priority: u8,
    /// Message body size in bytes.
    pub body_bytes: u64,
}

/// Build email outbound queue entry for ALICE-Queue.
#[inline]
#[must_use]
pub fn email_to_queue_entry(
    sender_hash: u64,
    recipient_hash: u64,
    priority: u8,
    body_bytes: u64,
) -> EmailQueueEntry {
    // buf: sender_hash(8) + recipient_hash(8) + priority(1) + body_bytes(8) = 25
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&sender_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&recipient_hash.to_le_bytes());
    buf[16] = priority;
    buf[17..25].copy_from_slice(&body_bytes.to_le_bytes());
    EmailQueueEntry {
        content_hash: fnv1a(&buf),
        sender_hash,
        recipient_hash,
        priority,
        body_bytes,
    }
}

// ── Bridge 5: Email → Notify (delivery alert) ────────────────────────────

/// Email delivery alert for ALICE-Notify.
pub struct EmailNotifyAlert {
    /// Content hash.
    pub content_hash: u64,
    /// Alert severity level (0 = info, 1 = warn, 2 = critical).
    pub severity: u8,
    /// Number of bounced messages.
    pub bounce_count: u32,
    /// Spam score multiplied by 100 (e.g. 750 = 7.50).
    pub spam_score_x100: u32,
    /// Alert timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build email delivery alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn email_to_notify_alert(
    severity: u8,
    bounce_count: u32,
    spam_score_x100: u32,
    timestamp_ms: u64,
) -> EmailNotifyAlert {
    // buf: severity(1) + bounce_count(4) + spam_score_x100(4) + timestamp_ms(8) = 17
    let mut buf = [0u8; 17];
    buf[0] = severity;
    buf[1..5].copy_from_slice(&bounce_count.to_le_bytes());
    buf[5..9].copy_from_slice(&spam_score_x100.to_le_bytes());
    buf[9..17].copy_from_slice(&timestamp_ms.to_le_bytes());
    EmailNotifyAlert {
        content_hash: fnv1a(&buf),
        severity,
        bounce_count,
        spam_score_x100,
        timestamp_ms,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_to_db_record_hash_nonzero() {
        let rec = email_to_db_record(0xdead_beef_0000_0001, 5, 4_096, true, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_email_to_db_record_fields() {
        let rec = email_to_db_record(0x1234, 3, 2_048, false, 99_999);
        assert_eq!(rec.sender_hash, 0x1234);
        assert_eq!(rec.recipient_count, 3);
        assert_eq!(rec.body_bytes, 2_048);
        assert!(!rec.has_attachment);
        assert_eq!(rec.timestamp_ms, 99_999);
    }

    #[test]
    fn test_email_to_db_record_with_attachment() {
        let rec = email_to_db_record(0xffff, 1, 102_400, true, 1_000);
        assert!(rec.has_attachment);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_email_to_cache_entry_hash_nonzero() {
        let entry = email_to_cache_entry(0xabcd_ef01, 1_800, 15, 3);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_email_to_cache_entry_fields() {
        let entry = email_to_cache_entry(0x5555, 900, 10, 2);
        assert_eq!(entry.thread_hash, 0x5555);
        assert_eq!(entry.ttl_secs, 900);
        assert_eq!(entry.msg_count, 10);
        assert_eq!(entry.unread_count, 2);
    }

    #[test]
    fn test_email_to_analytics_event_hash_nonzero() {
        let ev = email_to_analytics_event(10_000, 9_500, 250, 4_200, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_email_to_analytics_event_fields() {
        let ev = email_to_analytics_event(500, 480, 100, 3_500, 55_555);
        assert_eq!(ev.sent_count, 500);
        assert_eq!(ev.received_count, 480);
        assert_eq!(ev.bounce_rate_bps, 100);
        assert_eq!(ev.open_rate_bps, 3_500);
        assert_eq!(ev.timestamp_ms, 55_555);
    }

    #[test]
    fn test_email_to_queue_entry_hash_nonzero() {
        let entry = email_to_queue_entry(0xbeef_cafe, 0x1234_5678, 1, 8_192);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_email_to_queue_entry_fields() {
        let entry = email_to_queue_entry(0x1111, 0x2222, 2, 1_024);
        assert_eq!(entry.sender_hash, 0x1111);
        assert_eq!(entry.recipient_hash, 0x2222);
        assert_eq!(entry.priority, 2);
        assert_eq!(entry.body_bytes, 1_024);
    }

    #[test]
    fn test_email_to_notify_alert_hash_nonzero() {
        let alert = email_to_notify_alert(2, 50, 850, 1_700_000_000_000);
        assert_ne!(alert.content_hash, 0);
    }

    #[test]
    fn test_email_to_notify_alert_fields() {
        let alert = email_to_notify_alert(1, 10, 320, 12_345);
        assert_eq!(alert.severity, 1);
        assert_eq!(alert.bounce_count, 10);
        assert_eq!(alert.spam_score_x100, 320);
        assert_eq!(alert.timestamp_ms, 12_345);
    }

    #[test]
    fn test_email_to_notify_alert_determinism() {
        let a = email_to_notify_alert(0, 0, 0, 0);
        let b = email_to_notify_alert(0, 0, 0, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
