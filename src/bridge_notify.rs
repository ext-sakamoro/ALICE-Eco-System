//! Notify bridges — ALICE-Notify ↔ DB, Cache, Analytics, Queue, Edge
//!
//! 5 bridges connecting the notification dispatch layer to the ALICE ecosystem.
//! Covers notification record persistence, state caching, metric telemetry,
//! Queue-based dispatch, and Edge push event forwarding.

use alice_notify::{hmac_sign, BloomFilter, Channel, ExponentialBackoff, Notification, Urgency};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Notify → DB (notification records) ─────────────────────────

/// Notification delivery record for ALICE-DB.
///
/// Written when a notification is dispatched so that delivery state is
/// durable and auditable across retries and failures.
pub struct NotifyDbRecord {
    /// FNV-1a hash of the notification ID.
    pub content_hash: u64,
    /// Channel code: 0=Webhook, 1=Email, 2=Sms, 3=Push.
    pub channel: u8,
    /// Urgency level: 0=Low, 1=Normal, 2=High, 3=Critical.
    pub urgency: u8,
    /// Payload size in bytes (subject + body).
    pub payload_bytes: usize,
    /// HMAC signature of the notification body (FNV-based stub).
    pub hmac_signature: u64,
}

/// Build a notification DB record from a `Notification`.
#[inline]
#[must_use]
pub fn notify_to_db_record(notification: &Notification, signing_key: &[u8]) -> NotifyDbRecord {
    let content_hash = fnv1a(notification.id.as_bytes());
    let channel: u8 = match notification.channel {
        Channel::Webhook => 0,
        Channel::Email => 1,
        Channel::Sms => 2,
        Channel::Push => 3,
    };
    let urgency: u8 = match notification.urgency {
        Urgency::Low => 0,
        Urgency::Normal => 1,
        Urgency::High => 2,
        Urgency::Critical => 3,
    };
    let payload_bytes = notification.subject.len() + notification.body.len();
    let hmac_signature = hmac_sign(signing_key, notification.body.as_bytes());
    NotifyDbRecord {
        content_hash,
        channel,
        urgency,
        payload_bytes,
        hmac_signature,
    }
}

// ── Bridge 2: Notify → Cache (notification cache) ────────────────────────

/// Notification deduplication cache entry for ALICE-Cache.
///
/// Caches recently dispatched notification IDs (via a Bloom-filter proxy)
/// so that duplicate dispatch is suppressed without a DB lookup on every
/// request.  TTL is set branchlessly: 60 s for Critical/High urgency,
/// 300 s for Normal/Low (lower-urgency duplicates are tolerable longer).
pub struct NotifyCacheEntry {
    /// FNV-1a hash of the notification ID — primary cache key.
    pub content_hash: u64,
    /// True when the Bloom filter reports the notification may already be known.
    pub may_be_duplicate: bool,
    /// Urgency level (mirrors `NotifyDbRecord::urgency`).
    pub urgency: u8,
    /// Cache TTL in seconds (branchless: 60 high-urgency, 300 low-urgency).
    pub ttl_secs: u32,
    /// Channel code (mirrors `NotifyDbRecord::channel`).
    pub channel: u8,
}

/// Build a notification deduplication cache entry.
///
/// `bloom` is used to check whether the notification ID has already been seen.
/// `ttl_secs` is branchless: 60 when urgency >= High (2), else 300.
#[inline]
#[must_use]
pub fn notify_to_cache_entry(notification: &Notification, bloom: &BloomFilter) -> NotifyCacheEntry {
    let content_hash = fnv1a(notification.id.as_bytes());
    let may_be_duplicate = bloom.might_contain(notification.id.as_bytes());
    let urgency: u8 = match notification.urgency {
        Urgency::Low => 0,
        Urgency::Normal => 1,
        Urgency::High => 2,
        Urgency::Critical => 3,
    };
    let channel: u8 = match notification.channel {
        Channel::Webhook => 0,
        Channel::Email => 1,
        Channel::Sms => 2,
        Channel::Push => 3,
    };
    // ブランチレス TTL: 高優先度(>=2)=60s, 低優先度=300s
    let is_high_urgency = (urgency >= 2) as u32;
    let ttl_secs = 300 - is_high_urgency * 240;
    NotifyCacheEntry {
        content_hash,
        may_be_duplicate,
        urgency,
        ttl_secs,
        channel,
    }
}

// ── Bridge 3: Notify → Analytics (notification metrics) ──────────────────

/// Notification delivery metric event for ALICE-Analytics.
///
/// Emitted on each dispatch attempt so the analytics layer can build
/// per-channel delivery rate, retry histogram, and urgency distribution
/// dashboards.
pub struct NotifyAnalyticsEvent {
    /// FNV-1a hash of the recipient identifier.
    pub content_hash: u64,
    /// Channel code.
    pub channel: u8,
    /// Urgency level.
    pub urgency: u8,
    /// Payload size in bytes.
    pub payload_bytes: usize,
    /// Next retry delay in milliseconds (0 when attempt = 0).
    pub next_retry_ms: u64,
    /// True when max retries have been exhausted.
    pub is_exhausted: bool,
}

/// Build a notification analytics event from a `Notification` and retry state.
#[inline]
#[must_use]
pub fn notify_to_analytics_event(
    notification: &Notification,
    backoff: &ExponentialBackoff,
    attempt: u32,
) -> NotifyAnalyticsEvent {
    let content_hash = fnv1a(notification.recipient.as_bytes());
    let channel: u8 = match notification.channel {
        Channel::Webhook => 0,
        Channel::Email => 1,
        Channel::Sms => 2,
        Channel::Push => 3,
    };
    let urgency: u8 = match notification.urgency {
        Urgency::Low => 0,
        Urgency::Normal => 1,
        Urgency::High => 2,
        Urgency::Critical => 3,
    };
    let payload_bytes = notification.subject.len() + notification.body.len();
    let next_retry_ms = if attempt == 0 {
        0
    } else {
        backoff.delay(attempt)
    };
    let is_exhausted = !backoff.should_retry(attempt);
    NotifyAnalyticsEvent {
        content_hash,
        channel,
        urgency,
        payload_bytes,
        next_retry_ms,
        is_exhausted,
    }
}

// ── Bridge 4: Notify → Queue (notification dispatch) ─────────────────────

/// Notification dispatch message for ALICE-Queue.
///
/// Enqueued when a notification is ready for delivery so that worker
/// processes can pick it up asynchronously without coupling to the notify
/// engine's internal scheduler.
pub struct NotifyQueueMessage {
    /// FNV-1a hash of the notification ID.
    pub content_hash: u64,
    /// Channel code.
    pub channel: u8,
    /// Urgency level (used by queue priority scheduler).
    pub urgency: u8,
    /// Retry attempt number.
    pub attempt: u32,
    /// Computed backoff delay for this attempt in milliseconds.
    pub backoff_ms: u64,
    /// Payload size in bytes.
    pub payload_bytes: usize,
}

/// Build a notification queue message for ALICE-Queue.
#[inline]
#[must_use]
pub fn notify_to_queue_message(
    notification: &Notification,
    backoff: &ExponentialBackoff,
    attempt: u32,
) -> NotifyQueueMessage {
    let content_hash = fnv1a(notification.id.as_bytes());
    let channel: u8 = match notification.channel {
        Channel::Webhook => 0,
        Channel::Email => 1,
        Channel::Sms => 2,
        Channel::Push => 3,
    };
    let urgency: u8 = match notification.urgency {
        Urgency::Low => 0,
        Urgency::Normal => 1,
        Urgency::High => 2,
        Urgency::Critical => 3,
    };
    let backoff_ms = backoff.delay(attempt);
    let payload_bytes = notification.subject.len() + notification.body.len();
    NotifyQueueMessage {
        content_hash,
        channel,
        urgency,
        attempt,
        backoff_ms,
        payload_bytes,
    }
}

// ── Bridge 5: Notify → Edge (push events) ────────────────────────────────

/// Push notification event for ALICE-Edge.
///
/// Edge nodes forward Push-channel notifications to mobile devices via APNs
/// or FCM.  This descriptor carries the urgency and payload size so Edge can
/// apply platform-specific payload constraints.
pub struct NotifyEdgePushEvent {
    /// FNV-1a hash of the notification ID.
    pub content_hash: u64,
    /// Recipient identifier hash (FNV-1a of recipient string).
    pub recipient_hash: u64,
    /// Urgency level.
    pub urgency: u8,
    /// Push platform: derived from channel (3=Push → 0; others → 255=invalid).
    pub platform: u8,
    /// Payload size in bytes.
    pub payload_bytes: usize,
    /// True when payload fits within APNs/FCM 4 KB limit.
    pub within_platform_limit: bool,
}

/// Build a push event for ALICE-Edge from a `Notification`.
///
/// Only Push-channel notifications produce a valid `platform` (0).
/// Other channels set `platform = 255` to signal an invalid routing.
#[inline]
#[must_use]
pub fn notify_to_edge_push_event(notification: &Notification) -> NotifyEdgePushEvent {
    let content_hash = fnv1a(notification.id.as_bytes());
    let recipient_hash = fnv1a(notification.recipient.as_bytes());
    let urgency: u8 = match notification.urgency {
        Urgency::Low => 0,
        Urgency::Normal => 1,
        Urgency::High => 2,
        Urgency::Critical => 3,
    };
    let platform: u8 = match notification.channel {
        Channel::Push => 0,
        _ => 255,
    };
    let payload_bytes = notification.subject.len() + notification.body.len();
    // APNs/FCM の 4 KB 制限（4096 バイト）以内かどうか
    let within_platform_limit = payload_bytes <= 4_096;
    NotifyEdgePushEvent {
        content_hash,
        recipient_hash,
        urgency,
        platform,
        payload_bytes,
        within_platform_limit,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_notify::{BloomFilter, Channel, ExponentialBackoff, Notification, Urgency};

    fn make_notification(channel: Channel, urgency: Urgency) -> Notification {
        Notification {
            id: std::string::String::from("notif-001"),
            channel,
            urgency,
            recipient: std::string::String::from("user@example.com"),
            subject: std::string::String::from("Alert"),
            body: std::string::String::from("Your order has shipped."),
        }
    }

    fn default_backoff() -> ExponentialBackoff {
        ExponentialBackoff::new(1_000, 60_000, 5)
    }

    // Bridge 1 ───────────────────────────────────────────────────────────

    #[test]
    fn test_notify_to_db_record_basic() {
        let n = make_notification(Channel::Email, Urgency::Normal);
        let rec = notify_to_db_record(&n, b"secret-key");
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.channel, 1); // Email
        assert_eq!(rec.urgency, 1); // Normal
        assert!(rec.payload_bytes > 0);
        assert_ne!(rec.hmac_signature, 0);
    }

    #[test]
    fn test_notify_to_db_record_channel_mapping() {
        let channels = [
            (Channel::Webhook, 0u8),
            (Channel::Email, 1),
            (Channel::Sms, 2),
            (Channel::Push, 3),
        ];
        for (ch, expected) in channels {
            let n = make_notification(ch, Urgency::Low);
            let rec = notify_to_db_record(&n, b"k");
            assert_eq!(rec.channel, expected);
        }
    }

    // Bridge 2 ───────────────────────────────────────────────────────────

    #[test]
    fn test_notify_to_cache_entry_high_urgency_ttl() {
        let n = make_notification(Channel::Push, Urgency::Critical);
        let bloom = BloomFilter::new(1_000, 0.01);
        let entry = notify_to_cache_entry(&n, &bloom);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.urgency, 3); // Critical
        assert_eq!(entry.ttl_secs, 60); // 高優先度 → 60s
        assert!(!entry.may_be_duplicate); // 空のBloomFilter
    }

    #[test]
    fn test_notify_to_cache_entry_low_urgency_ttl() {
        let n = make_notification(Channel::Email, Urgency::Low);
        let bloom = BloomFilter::new(1_000, 0.01);
        let entry = notify_to_cache_entry(&n, &bloom);
        assert_eq!(entry.ttl_secs, 300); // 低優先度 → 300s
    }

    #[test]
    fn test_notify_to_cache_entry_bloom_hit() {
        let n = make_notification(Channel::Webhook, Urgency::Normal);
        let mut bloom = BloomFilter::new(1_000, 0.01);
        bloom.insert(n.id.as_bytes()); // 登録済み
        let entry = notify_to_cache_entry(&n, &bloom);
        assert!(entry.may_be_duplicate);
    }

    #[test]
    fn test_notify_to_cache_entry_determinism() {
        let n = make_notification(Channel::Sms, Urgency::High);
        let bloom = BloomFilter::new(1_000, 0.01);
        let e1 = notify_to_cache_entry(&n, &bloom);
        let e2 = notify_to_cache_entry(&n, &bloom);
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.ttl_secs, e2.ttl_secs);
    }

    // Bridge 3 ───────────────────────────────────────────────────────────

    #[test]
    fn test_notify_to_analytics_event_first_attempt() {
        let n = make_notification(Channel::Webhook, Urgency::High);
        let bo = default_backoff();
        let ev = notify_to_analytics_event(&n, &bo, 0);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.next_retry_ms, 0); // attempt=0 → no delay
        assert!(!ev.is_exhausted);
        assert_eq!(ev.urgency, 2); // High
    }

    #[test]
    fn test_notify_to_analytics_event_exhausted() {
        let n = make_notification(Channel::Email, Urgency::Low);
        let bo = default_backoff();
        let ev = notify_to_analytics_event(&n, &bo, 5); // max_retries=5
        assert!(ev.is_exhausted);
        assert!(ev.next_retry_ms > 0);
    }

    // Bridge 4 ───────────────────────────────────────────────────────────

    #[test]
    fn test_notify_to_queue_message_basic() {
        let n = make_notification(Channel::Push, Urgency::Critical);
        let bo = default_backoff();
        let msg = notify_to_queue_message(&n, &bo, 2);
        assert_ne!(msg.content_hash, 0);
        assert_eq!(msg.urgency, 3); // Critical
        assert_eq!(msg.channel, 3); // Push
        assert_eq!(msg.attempt, 2);
        // 2回目: 1000 × 2^2 = 4000ms
        assert_eq!(msg.backoff_ms, 4_000);
    }

    #[test]
    fn test_notify_to_queue_message_urgency_mapping() {
        let urgencies = [
            (Urgency::Low, 0u8),
            (Urgency::Normal, 1),
            (Urgency::High, 2),
            (Urgency::Critical, 3),
        ];
        for (urg, expected) in urgencies {
            let n = make_notification(Channel::Email, urg);
            let bo = default_backoff();
            let msg = notify_to_queue_message(&n, &bo, 0);
            assert_eq!(msg.urgency, expected);
        }
    }

    // Bridge 5 ───────────────────────────────────────────────────────────

    #[test]
    fn test_notify_to_edge_push_event_push_channel() {
        let n = make_notification(Channel::Push, Urgency::High);
        let ev = notify_to_edge_push_event(&n);
        assert_ne!(ev.content_hash, 0);
        assert_ne!(ev.recipient_hash, 0);
        assert_eq!(ev.platform, 0); // Push → 有効プラットフォーム
        assert_eq!(ev.urgency, 2);
        assert!(ev.within_platform_limit);
    }

    #[test]
    fn test_notify_to_edge_push_event_invalid_channel() {
        let n = make_notification(Channel::Email, Urgency::Normal);
        let ev = notify_to_edge_push_event(&n);
        assert_eq!(ev.platform, 255); // Push 以外 → 無効
    }
}
