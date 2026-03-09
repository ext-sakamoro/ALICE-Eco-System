//! Chat bridges — ALICE-Chat ↔ DB, Cache, Analytics, Notify, Search
//!
//! 5 bridges connecting real-time chat to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Chat → DB (message log persistence) ────────────────────────

/// Message log record for ALICE-DB persistence.
pub struct ChatDbRecord {
    /// Content hash over room + message payload.
    pub content_hash: u64,
    /// FNV-1a hash of the room identifier string.
    pub room_id_hash: u64,
    /// Length of the message body in bytes.
    pub message_len: u32,
    /// Number of active users in the room at send time.
    pub user_count: u32,
    /// Message send timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
    /// Sequence number within the room (monotonic).
    pub sequence: u64,
}

/// Serialize a chat message event for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn chat_to_db_record(
    room_id: &[u8],
    message_body: &[u8],
    user_count: u32,
    timestamp_ns: u64,
    sequence: u64,
) -> ChatDbRecord {
    let room_id_hash = fnv1a(room_id);
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&room_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&timestamp_ns.to_le_bytes());
    key[16..24].copy_from_slice(&sequence.to_le_bytes());
    let content_hash = fnv1a(&[&key[..], message_body].concat());
    ChatDbRecord {
        content_hash,
        room_id_hash,
        message_len: message_body.len() as u32,
        user_count,
        timestamp_ns,
        sequence,
    }
}

// ── Bridge 2: Chat → Cache (session state) ───────────────────────────────

/// Session cache entry for ALICE-Cache.
pub struct ChatCacheEntry {
    /// Content hash over room + user count.
    pub content_hash: u64,
    /// FNV-1a hash of the room identifier.
    pub room_id_hash: u64,
    /// Number of active users in the session.
    pub user_count: u32,
    /// Cache TTL in seconds (reduced when room is empty).
    pub ttl_secs: u32,
    /// Last activity timestamp in nanoseconds.
    pub last_active_ns: u64,
}

/// Build a session cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 60 s when the room has no active users.
#[inline]
#[must_use]
pub fn chat_to_cache_entry(room_id: &[u8], user_count: u32, last_active_ns: u64) -> ChatCacheEntry {
    let room_id_hash = fnv1a(room_id);
    let mut key = [0u8; 12];
    key[0..8].copy_from_slice(&room_id_hash.to_le_bytes());
    key[8..12].copy_from_slice(&user_count.to_le_bytes());
    // Branchless TTL: 3600 s for active rooms, 60 s for empty rooms.
    let is_empty = (user_count == 0) as u32;
    let ttl_secs = 3_600_u32 - is_empty * 3_540_u32;
    ChatCacheEntry {
        content_hash: fnv1a(&key),
        room_id_hash,
        user_count,
        ttl_secs,
        last_active_ns,
    }
}

// ── Bridge 3: Chat → Analytics (message metrics) ─────────────────────────

/// Message metrics for ALICE-Analytics ingestion.
pub struct ChatAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total messages sent in the reporting window.
    pub messages_sent: u64,
    /// Total bytes of message content in the window.
    pub total_bytes: u64,
    /// Average message length in bytes.
    pub avg_message_len: f64,
    /// Peak concurrent user count observed in the window.
    pub peak_user_count: u32,
    /// Window start timestamp in nanoseconds.
    pub window_start_ns: u64,
}

/// Build message metrics for ALICE-Analytics ingestion.
///
/// Average message length uses reciprocal multiply to avoid repeated division.
#[inline]
#[must_use]
pub fn chat_to_analytics_metrics(
    messages_sent: u64,
    total_bytes: u64,
    peak_user_count: u32,
    window_start_ns: u64,
) -> ChatAnalyticsMetrics {
    let rcp = 1.0 / messages_sent.max(1) as f64;
    let avg_message_len = total_bytes as f64 * rcp;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&messages_sent.to_le_bytes());
    key[8..16].copy_from_slice(&total_bytes.to_le_bytes());
    key[16..24].copy_from_slice(&window_start_ns.to_le_bytes());
    ChatAnalyticsMetrics {
        content_hash: fnv1a(&key),
        messages_sent,
        total_bytes,
        avg_message_len,
        peak_user_count,
        window_start_ns,
    }
}

// ── Bridge 4: Chat → Notify (push notification payload) ──────────────────

/// Push notification payload for ALICE-Notify.
pub struct ChatNotifyPayload {
    /// Content hash over room + recipient hash.
    pub content_hash: u64,
    /// FNV-1a hash of the room identifier.
    pub room_id_hash: u64,
    /// FNV-1a hash of the recipient user identifier.
    pub recipient_hash: u64,
    /// Truncated message preview length in bytes (max 128).
    pub preview_len: u8,
    /// Delivery priority: 0 = normal, 1 = high.
    pub priority: u8,
    /// Timestamp the notification was enqueued (nanoseconds).
    pub enqueued_ns: u64,
}

/// Build a push notification payload for ALICE-Notify.
///
/// `preview_len` is clamped to 128 bytes branchlessly.
#[inline]
#[must_use]
pub fn chat_to_notify_payload(
    room_id: &[u8],
    recipient_id: &[u8],
    message_body: &[u8],
    high_priority: bool,
    enqueued_ns: u64,
) -> ChatNotifyPayload {
    let room_id_hash = fnv1a(room_id);
    let recipient_hash = fnv1a(recipient_id);
    // Branchless clamp to 128.
    let raw_len = message_body.len();
    let over = (raw_len > 128) as usize;
    let preview_len = (raw_len * (1 - over) + 128 * over) as u8;
    let priority = high_priority as u8;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&room_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&recipient_hash.to_le_bytes());
    ChatNotifyPayload {
        content_hash: fnv1a(&key),
        room_id_hash,
        recipient_hash,
        preview_len,
        priority,
        enqueued_ns,
    }
}

// ── Bridge 5: Chat → Search (message index) ──────────────────────────────

/// Message search index record for ALICE-Search.
pub struct ChatSearchIndex {
    /// Content hash over room + sequence.
    pub content_hash: u64,
    /// FNV-1a hash of the room identifier.
    pub room_id_hash: u64,
    /// FNV-1a hash of the message body (for deduplication).
    pub body_hash: u64,
    /// Message sequence number within the room.
    pub sequence: u64,
    /// Message send timestamp in nanoseconds (for range queries).
    pub timestamp_ns: u64,
    /// Message length in bytes (for relevance scoring).
    pub message_len: u32,
}

/// Build a search index record for ALICE-Search.
#[inline]
#[must_use]
pub fn chat_to_search_index(
    room_id: &[u8],
    message_body: &[u8],
    sequence: u64,
    timestamp_ns: u64,
) -> ChatSearchIndex {
    let room_id_hash = fnv1a(room_id);
    let body_hash = fnv1a(message_body);
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&room_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&sequence.to_le_bytes());
    ChatSearchIndex {
        content_hash: fnv1a(&key),
        room_id_hash,
        body_hash,
        sequence,
        timestamp_ns,
        message_len: message_body.len() as u32,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const ROOM: &[u8] = b"room:lobby";
    const MSG: &[u8] = b"hello world";
    const USER: &[u8] = b"user:42";

    #[test]
    fn test_chat_to_db_record_hash_nonzero() {
        let rec = chat_to_db_record(ROOM, MSG, 5, 1_000_000_000, 1);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.room_id_hash, 0);
    }

    #[test]
    fn test_chat_to_db_record_fields() {
        let rec = chat_to_db_record(ROOM, MSG, 7, 2_000_000_000, 42);
        assert_eq!(rec.message_len, MSG.len() as u32);
        assert_eq!(rec.user_count, 7);
        assert_eq!(rec.timestamp_ns, 2_000_000_000);
        assert_eq!(rec.sequence, 42);
    }

    #[test]
    fn test_chat_to_cache_entry_active_room() {
        let entry = chat_to_cache_entry(ROOM, 10, 999);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 3_600);
    }

    #[test]
    fn test_chat_to_cache_entry_empty_room() {
        let entry = chat_to_cache_entry(ROOM, 0, 0);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_chat_to_analytics_metrics_avg() {
        // 100 messages, 1000 bytes total → avg 10.0 bytes/message.
        let m = chat_to_analytics_metrics(100, 1_000, 50, 0);
        assert_ne!(m.content_hash, 0);
        assert!((m.avg_message_len - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_chat_to_analytics_metrics_zero_messages() {
        let m = chat_to_analytics_metrics(0, 0, 0, 0);
        assert_eq!(m.messages_sent, 0);
        assert_eq!(m.avg_message_len, 0.0);
    }

    #[test]
    fn test_chat_to_notify_payload_priority() {
        let p = chat_to_notify_payload(ROOM, USER, MSG, true, 1_234);
        assert_eq!(p.priority, 1);
        assert_ne!(p.content_hash, 0);
    }

    #[test]
    fn test_chat_to_search_index_deterministic() {
        let a = chat_to_search_index(ROOM, MSG, 1, 100);
        let b = chat_to_search_index(ROOM, MSG, 1, 100);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.body_hash, b.body_hash);
        assert_eq!(a.message_len, MSG.len() as u32);
    }
}
