//! Queue bridges — ALICE-Queue ↔ DB, Edge, Crypto, Analytics, Sync, Cache, Risk, Container
//!
//! 8 bridges connecting message queue to the ALICE ecosystem.

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

// ── Bridge 7: Queue → Risk (優先度超過メッセージのエスカレーションアラート) ─

/// Priority escalation alert for ALICE-Risk when queue depth or priority spikes.
pub struct QueueRiskEscalation {
    /// Content hash (FNV-1a of queue_depth + max_priority + oldest_message_age_ms).
    pub content_hash: u64,
    /// Current depth of the message queue.
    pub queue_depth: u32,
    /// Highest priority value currently in the queue (0 = lowest, 255 = highest).
    pub max_priority: u8,
    /// Age of the oldest unprocessed message in milliseconds.
    pub oldest_message_age_ms: u64,
    /// Escalation level (0 = none … 3 = critical).
    pub escalation_level: u8,
}

/// Build a risk escalation alert from current queue state metadata.
#[inline]
#[must_use]
pub fn queue_to_risk_escalation(
    msg: &Message,
    queue_depth: u32,
    max_priority: u8,
    oldest_message_age_ms: u64,
    escalation_level: u8,
) -> QueueRiskEscalation {
    // キューの深さ・最高優先度・最古メッセージ年齢をハッシュ入力に含める
    let mut hash_data = [0u8; 4 + 1 + 8 + 8];
    hash_data[..4].copy_from_slice(&queue_depth.to_le_bytes());
    hash_data[4] = max_priority;
    hash_data[5..13].copy_from_slice(&oldest_message_age_ms.to_le_bytes());
    hash_data[13..21].copy_from_slice(&msg.header.seq.to_le_bytes());
    QueueRiskEscalation {
        content_hash: fnv1a(&hash_data),
        queue_depth,
        max_priority,
        oldest_message_age_ms,
        escalation_level,
    }
}

// ── Bridge 8: Queue → Container (キューリソース使用率メトリクス) ──────────

/// Queue resource utilization metrics for ALICE-Container orchestration.
pub struct QueueContainerMetrics {
    /// Content hash (FNV-1a of memory_usage_bytes + message_count + consumer_count).
    pub content_hash: u64,
    /// Memory consumed by the queue in bytes.
    pub memory_usage_bytes: u64,
    /// Number of messages currently in the queue.
    pub message_count: u32,
    /// Number of active consumers attached to the queue.
    pub consumer_count: u16,
    /// Cache TTL in seconds (branchless: 30 if consumer_count > 0, else 0).
    pub ttl_secs: u32,
}

/// Build container metrics from queue state associated with the given message.
///
/// `ttl_secs` is computed branchlessly: 30 when `consumer_count > 0`, else 0.
#[inline]
#[must_use]
pub fn queue_to_container_metrics(
    msg: &Message,
    memory_usage_bytes: u64,
    message_count: u32,
    consumer_count: u16,
) -> QueueContainerMetrics {
    // コンシューマーがいる場合は TTL=30、いない場合は TTL=0（ブランチレス）
    let has_consumers = (consumer_count > 0) as u32;
    let ttl_secs = has_consumers * 30;
    let mut hash_data = [0u8; 8 + 4 + 2 + 8];
    hash_data[..8].copy_from_slice(&memory_usage_bytes.to_le_bytes());
    hash_data[8..12].copy_from_slice(&message_count.to_le_bytes());
    hash_data[12..14].copy_from_slice(&consumer_count.to_le_bytes());
    hash_data[14..22].copy_from_slice(&msg.header.seq.to_le_bytes());
    QueueContainerMetrics {
        content_hash: fnv1a(&hash_data),
        memory_usage_bytes,
        message_count,
        consumer_count,
        ttl_secs,
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

    // ── Bridge 7: QueueRiskEscalation ─────────────────────────────────

    #[test]
    fn test_queue_to_risk_escalation_basic() {
        // 基本フィールドとハッシュの非ゼロ検証
        let msg = test_message();
        let esc = queue_to_risk_escalation(&msg, 128, 200, 5_000, 2);
        assert_ne!(esc.content_hash, 0);
        assert_eq!(esc.queue_depth, 128);
        assert_eq!(esc.max_priority, 200);
        assert_eq!(esc.oldest_message_age_ms, 5_000);
        assert_eq!(esc.escalation_level, 2);
    }

    #[test]
    fn test_queue_to_risk_escalation_determinism() {
        // 同一入力→同一ハッシュ（決定性）
        let msg = test_message();
        let e1 = queue_to_risk_escalation(&msg, 64, 100, 1_000, 1);
        let e2 = queue_to_risk_escalation(&msg, 64, 100, 1_000, 1);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 8: QueueContainerMetrics ───────────────────────────────

    #[test]
    fn test_queue_to_container_metrics_basic() {
        // 基本フィールドとブランチレスTTLの検証（consumer_count > 0 → TTL=30）
        let msg = test_message();
        let m = queue_to_container_metrics(&msg, 4_096_000, 512, 8);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.memory_usage_bytes, 4_096_000);
        assert_eq!(m.message_count, 512);
        assert_eq!(m.consumer_count, 8);
        assert_eq!(m.ttl_secs, 30);
    }

    #[test]
    fn test_queue_to_container_metrics_zero_consumers_ttl() {
        // consumer_count=0 のとき TTL はゼロ（ブランチレス検証）
        let msg = test_message();
        let m = queue_to_container_metrics(&msg, 1_000, 10, 0);
        assert_eq!(m.ttl_secs, 0);
    }
}
