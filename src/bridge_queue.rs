//! Queue bridges — ALICE-Queue ↔ DB, Edge, Crypto, Analytics, Sync, Cache
//!
//! 6 bridges connecting message queue to the ALICE ecosystem.

use alice_queue::{GapResult, IdempotencyBarrier, Message};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Queue → DB (message persistence) ──────────────────────────

/// Message persistence record for ALICE-DB.
pub struct QueueDbRecord {
    /// Message ID (BLAKE3 hash).
    pub message_id: [u8; 32],
    /// Content hash (FNV-1a of payload).
    pub content_hash: u64,
    /// Sender key.
    pub sender: [u8; 32],
    /// Sequence number.
    pub sequence: u64,
    /// Payload size in bytes.
    pub payload_bytes: usize,
}

/// Serialize message for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn queue_to_db_record(msg: &Message) -> QueueDbRecord {
    QueueDbRecord {
        message_id: msg.header.id,
        content_hash: fnv1a(&msg.payload),
        sender: msg.header.sender,
        sequence: msg.header.seq,
        payload_bytes: msg.payload.len(),
    }
}

// ── Bridge 2: Queue → Edge (lightweight message delivery) ────────────────

/// Lightweight message delivery for ALICE-Edge devices.
pub struct QueueEdgePayload {
    /// Sender hash (FNV-1a of sender key).
    pub sender_hash: u64,
    /// Sequence.
    pub sequence: u64,
    /// Payload size in bytes.
    pub payload_bytes: usize,
    /// Is compact (payload < 256 bytes).
    pub is_compact: bool,
}

/// Prepare message for ALICE-Edge lightweight delivery.
#[inline]
#[must_use]
pub fn queue_to_edge_payload(msg: &Message) -> QueueEdgePayload {
    QueueEdgePayload {
        sender_hash: fnv1a(&msg.header.sender),
        sequence: msg.header.seq,
        payload_bytes: msg.payload.len(),
        is_compact: msg.payload.len() < 256,
    }
}

// ── Bridge 3: Queue → Crypto (message authentication) ───────────────────

/// Message authentication metadata for ALICE-Crypto.
pub struct QueueCryptoPayload {
    /// Content hash (FNV-1a of payload).
    pub content_hash: u64,
    /// Message ID (BLAKE3 hash).
    pub message_id: [u8; 32],
    /// Payload size.
    pub payload_bytes: usize,
    /// Serialized message size.
    pub serialized_bytes: usize,
}

/// Prepare message for ALICE-Crypto authentication.
#[inline]
#[must_use]
pub fn queue_to_crypto_payload(msg: &Message) -> QueueCryptoPayload {
    QueueCryptoPayload {
        content_hash: fnv1a(&msg.payload),
        message_id: msg.header.id,
        payload_bytes: msg.payload.len(),
        serialized_bytes: msg.serialized_size(),
    }
}

// ── Bridge 4: Queue → Analytics (message throughput metrics) ─────────────

/// Message throughput metrics for ALICE-Analytics.
pub struct QueueAnalyticsMetrics {
    /// Content hash.
    pub content_hash: u64,
    /// Sender hash.
    pub sender_hash: u64,
    /// Sequence number.
    pub sequence: u64,
    /// Payload size.
    pub payload_bytes: usize,
    /// Serialized size.
    pub serialized_bytes: usize,
}

/// Extract throughput metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn queue_to_analytics_metrics(msg: &Message) -> QueueAnalyticsMetrics {
    let data = [msg.header.sender.as_slice(), &msg.header.seq.to_le_bytes()].concat();
    QueueAnalyticsMetrics {
        content_hash: fnv1a(&data),
        sender_hash: fnv1a(&msg.header.sender),
        sequence: msg.header.seq,
        payload_bytes: msg.payload.len(),
        serialized_bytes: msg.serialized_size(),
    }
}

// ── Bridge 5: Queue → Sync (ordered message delivery) ───────────────────

/// Ordered message delivery for ALICE-Sync.
pub struct QueueSyncDelivery {
    /// Message ID hash (FNV-1a).
    pub message_id_hash: u64,
    /// Sender hash.
    pub sender_hash: u64,
    /// Sequence number.
    pub sequence: u64,
    /// Gap check result.
    pub has_gap: bool,
    /// Payload size.
    pub payload_bytes: usize,
}

/// Check message ordering for ALICE-Sync delivery.
///
/// Note: `sender_id` is the u64 sender ID used in the barrier (not the
/// raw 32-byte sender key). Callers should map sender keys to sender IDs.
#[inline]
#[must_use]
pub fn queue_to_sync_delivery(
    msg: &Message,
    barrier: &IdempotencyBarrier,
    sender_id: u64,
) -> QueueSyncDelivery {
    let gap_result = barrier.check(sender_id, msg.header.seq);
    let has_gap = matches!(gap_result, GapResult::Gap { .. });
    QueueSyncDelivery {
        message_id_hash: fnv1a(&msg.header.id),
        sender_hash: fnv1a(&msg.header.sender),
        sequence: msg.header.seq,
        has_gap,
        payload_bytes: msg.payload.len(),
    }
}

// ── Bridge 6: Queue → Cache (message deduplication) ─────────────────────

/// Message deduplication entry for ALICE-Cache.
pub struct QueueCacheEntry {
    /// Message ID hash (FNV-1a) for dedup key.
    pub message_id_hash: u64,
    /// Content hash.
    pub content_hash: u64,
    /// Sender hash.
    pub sender_hash: u64,
    /// Payload size.
    pub payload_bytes: usize,
}

/// Prepare message deduplication entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn queue_to_cache_entry(msg: &Message) -> QueueCacheEntry {
    QueueCacheEntry {
        message_id_hash: fnv1a(&msg.header.id),
        content_hash: fnv1a(&msg.payload),
        sender_hash: fnv1a(&msg.header.sender),
        payload_bytes: msg.payload.len(),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_message() -> Message {
        Message::new([42u8; 32], 1, vec![0xDE, 0xAD, 0xBE, 0xEF])
    }

    #[test]
    fn test_queue_to_db_record() {
        let msg = test_message();
        let rec = queue_to_db_record(&msg);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.sender, [42u8; 32]);
        assert_eq!(rec.sequence, 1);
        assert_eq!(rec.payload_bytes, 4);
    }

    #[test]
    fn test_queue_to_edge_payload() {
        let msg = test_message();
        let payload = queue_to_edge_payload(&msg);
        assert!(payload.is_compact);
        assert_ne!(payload.sender_hash, 0);
    }

    #[test]
    fn test_queue_to_crypto_payload() {
        let msg = test_message();
        let cp = queue_to_crypto_payload(&msg);
        assert_ne!(cp.content_hash, 0);
        assert_eq!(cp.payload_bytes, 4);
        assert!(cp.serialized_bytes > 4);
    }

    #[test]
    fn test_queue_to_analytics_metrics() {
        let msg = test_message();
        let m = queue_to_analytics_metrics(&msg);
        assert_ne!(m.content_hash, 0);
        assert_ne!(m.sender_hash, 0);
        assert_eq!(m.sequence, 1);
    }

    #[test]
    fn test_queue_to_sync_delivery() {
        let msg = test_message();
        let barrier = IdempotencyBarrier::new();
        let del = queue_to_sync_delivery(&msg, &barrier, 42);
        assert_ne!(del.sender_hash, 0);
        assert_eq!(del.sequence, 1);
    }

    #[test]
    fn test_queue_to_cache_entry() {
        let msg = test_message();
        let entry = queue_to_cache_entry(&msg);
        assert_ne!(entry.content_hash, 0);
        assert_ne!(entry.sender_hash, 0);
    }
}
