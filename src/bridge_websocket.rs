//! WebSocket bridges — WebSocket ↔ DB, Cache, Analytics, Monitor, Streaming
//!
//! 5 bridges connecting the WebSocket layer to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: WebSocket → DB (connection log) ─────────────────────────────

/// Connection log record for ALICE-DB.
pub struct WebSocketDbRecord {
    /// Content hash (FNV-1a of connection_id_hash + frame_count + payload_bytes).
    pub content_hash: u64,
    /// Connection ID hash (FNV-1a of raw connection ID bytes).
    pub connection_id_hash: u64,
    /// Number of frames exchanged on this connection.
    pub frame_count: u64,
    /// Total payload bytes transferred.
    pub payload_bytes: u64,
    /// WebSocket opcode of the last frame (0x0–0xF).
    pub opcode: u8,
    /// Whether the last frame had masking applied (1 = masked).
    pub mask_applied: u8,
}

/// Serialize WebSocket connection data for ALICE-DB logging.
#[inline]
#[must_use]
pub fn websocket_to_db_record(
    connection_id: &[u8],
    frame_count: u64,
    payload_bytes: u64,
    opcode: u8,
    mask_applied: bool,
) -> WebSocketDbRecord {
    let connection_id_hash = fnv1a(connection_id);
    let mut buf = [0u8; 8 + 8 + 8 + 1 + 1];
    buf[..8].copy_from_slice(&connection_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&frame_count.to_le_bytes());
    buf[16..24].copy_from_slice(&payload_bytes.to_le_bytes());
    buf[24] = opcode;
    buf[25] = mask_applied as u8;
    WebSocketDbRecord {
        content_hash: fnv1a(&buf),
        connection_id_hash,
        frame_count,
        payload_bytes,
        opcode,
        mask_applied: mask_applied as u8,
    }
}

// ── Bridge 2: WebSocket → Cache (session) ────────────────────────────────

/// Session cache entry for ALICE-Cache.
pub struct WebSocketCacheEntry {
    /// Content hash (FNV-1a of connection_id_hash + payload_bytes).
    pub content_hash: u64,
    /// Connection ID hash (cache key).
    pub connection_id_hash: u64,
    /// Total payload bytes (used for TTL decision).
    pub payload_bytes: u64,
    /// Cache TTL in seconds (branchless: 300 if mask_applied, else 60).
    pub ttl_secs: u32,
    /// Frame count.
    pub frame_count: u64,
}

/// Build a session cache entry for ALICE-Cache.
///
/// `ttl_secs` is computed branchlessly: 300 when `mask_applied`, else 60.
#[inline]
#[must_use]
pub fn websocket_to_cache_entry(
    connection_id: &[u8],
    payload_bytes: u64,
    mask_applied: bool,
    frame_count: u64,
) -> WebSocketCacheEntry {
    let connection_id_hash = fnv1a(connection_id);
    let mut buf = [0u8; 8 + 8 + 8];
    buf[..8].copy_from_slice(&connection_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&payload_bytes.to_le_bytes());
    buf[16..24].copy_from_slice(&frame_count.to_le_bytes());
    // ブランチレスTTL: マスク適用済みなら300秒（クライアント送信）、なしなら60秒
    let is_masked = mask_applied as u32;
    let ttl_secs = 60 + is_masked * 240;
    WebSocketCacheEntry {
        content_hash: fnv1a(&buf),
        connection_id_hash,
        payload_bytes,
        ttl_secs,
        frame_count,
    }
}

// ── Bridge 3: WebSocket → Analytics (traffic metrics) ────────────────────

/// Traffic metrics for ALICE-Analytics.
pub struct WebSocketAnalyticsMetrics {
    /// Content hash (FNV-1a of all traffic fields).
    pub content_hash: u64,
    /// Connection ID hash.
    pub connection_id_hash: u64,
    /// Number of frames exchanged.
    pub frame_count: u64,
    /// Total payload bytes.
    pub payload_bytes: u64,
    /// Last opcode observed.
    pub opcode: u8,
    /// Whether masking was applied to the last frame.
    pub mask_applied: u8,
    /// Number of close frames received.
    pub close_frame_count: u32,
}

/// Extract traffic metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn websocket_to_analytics_metrics(
    connection_id: &[u8],
    frame_count: u64,
    payload_bytes: u64,
    opcode: u8,
    mask_applied: bool,
    close_frame_count: u32,
) -> WebSocketAnalyticsMetrics {
    let connection_id_hash = fnv1a(connection_id);
    let mut buf = [0u8; 8 + 8 + 8 + 1 + 1 + 4];
    buf[..8].copy_from_slice(&connection_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&frame_count.to_le_bytes());
    buf[16..24].copy_from_slice(&payload_bytes.to_le_bytes());
    buf[24] = opcode;
    buf[25] = mask_applied as u8;
    buf[26..30].copy_from_slice(&close_frame_count.to_le_bytes());
    WebSocketAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        connection_id_hash,
        frame_count,
        payload_bytes,
        opcode,
        mask_applied: mask_applied as u8,
        close_frame_count,
    }
}

// ── Bridge 4: WebSocket → Monitor (health) ───────────────────────────────

/// WebSocket health snapshot for ALICE-Monitor.
pub struct WebSocketMonitorHealth {
    /// Content hash (FNV-1a of frame_count + payload_bytes + close_frame_count).
    pub content_hash: u64,
    /// Total frame count across all active connections.
    pub frame_count: u64,
    /// Total payload bytes across all active connections.
    pub payload_bytes: u64,
    /// Number of close frames observed (indicates connection teardown rate).
    pub close_frame_count: u32,
    /// Number of currently open connections.
    pub open_connection_count: u32,
    /// Last opcode observed.
    pub last_opcode: u8,
}

/// Build a WebSocket health snapshot for ALICE-Monitor.
#[inline]
#[must_use]
pub fn websocket_to_monitor_health(
    frame_count: u64,
    payload_bytes: u64,
    close_frame_count: u32,
    open_connection_count: u32,
    last_opcode: u8,
) -> WebSocketMonitorHealth {
    let mut buf = [0u8; 8 + 8 + 4 + 4 + 1];
    buf[..8].copy_from_slice(&frame_count.to_le_bytes());
    buf[8..16].copy_from_slice(&payload_bytes.to_le_bytes());
    buf[16..20].copy_from_slice(&close_frame_count.to_le_bytes());
    buf[20..24].copy_from_slice(&open_connection_count.to_le_bytes());
    buf[24] = last_opcode;
    WebSocketMonitorHealth {
        content_hash: fnv1a(&buf),
        frame_count,
        payload_bytes,
        close_frame_count,
        open_connection_count,
        last_opcode,
    }
}

// ── Bridge 5: WebSocket → Streaming (frame relay) ────────────────────────

/// Frame relay descriptor for ALICE-Streaming.
pub struct WebSocketStreamingRelay {
    /// Content hash (FNV-1a of connection_id_hash + frame_count + opcode).
    pub content_hash: u64,
    /// Connection ID hash.
    pub connection_id_hash: u64,
    /// Frame count at relay time.
    pub frame_count: u64,
    /// Payload bytes to relay.
    pub payload_bytes: u64,
    /// WebSocket opcode.
    pub opcode: u8,
    /// Whether the payload is masked.
    pub mask_applied: u8,
}

/// Build a frame relay descriptor for ALICE-Streaming.
#[inline]
#[must_use]
pub fn websocket_to_streaming_relay(
    connection_id: &[u8],
    frame_count: u64,
    payload_bytes: u64,
    opcode: u8,
    mask_applied: bool,
) -> WebSocketStreamingRelay {
    let connection_id_hash = fnv1a(connection_id);
    let mut buf = [0u8; 8 + 8 + 8 + 1 + 1];
    buf[..8].copy_from_slice(&connection_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&frame_count.to_le_bytes());
    buf[16..24].copy_from_slice(&payload_bytes.to_le_bytes());
    buf[24] = opcode;
    buf[25] = mask_applied as u8;
    WebSocketStreamingRelay {
        content_hash: fnv1a(&buf),
        connection_id_hash,
        frame_count,
        payload_bytes,
        opcode,
        mask_applied: mask_applied as u8,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const CONN_ID: &[u8] = b"conn-abc-123";

    #[test]
    fn test_websocket_to_db_record_basic() {
        let rec = websocket_to_db_record(CONN_ID, 1000, 512_000, 0x02, true);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.connection_id_hash, 0);
        assert_eq!(rec.frame_count, 1000);
        assert_eq!(rec.payload_bytes, 512_000);
        assert_eq!(rec.opcode, 0x02);
        assert_eq!(rec.mask_applied, 1);
    }

    #[test]
    fn test_websocket_to_db_record_determinism() {
        let r1 = websocket_to_db_record(CONN_ID, 500, 256_000, 0x01, false);
        let r2 = websocket_to_db_record(CONN_ID, 500, 256_000, 0x01, false);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_websocket_to_cache_entry_masked_ttl() {
        // mask_applied = true → ttl_secs = 300
        let e = websocket_to_cache_entry(CONN_ID, 128_000, true, 200);
        assert_eq!(e.ttl_secs, 300);
        assert_ne!(e.content_hash, 0);
    }

    #[test]
    fn test_websocket_to_cache_entry_unmasked_ttl() {
        // mask_applied = false → ttl_secs = 60
        let e = websocket_to_cache_entry(CONN_ID, 128_000, false, 200);
        assert_eq!(e.ttl_secs, 60);
    }

    #[test]
    fn test_websocket_to_analytics_metrics_basic() {
        let m = websocket_to_analytics_metrics(CONN_ID, 2000, 1_024_000, 0x08, false, 5);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.frame_count, 2000);
        assert_eq!(m.close_frame_count, 5);
        assert_eq!(m.opcode, 0x08);
    }

    #[test]
    fn test_websocket_to_monitor_health_basic() {
        let h = websocket_to_monitor_health(5000, 2_048_000, 10, 100, 0x01);
        assert_ne!(h.content_hash, 0);
        assert_eq!(h.open_connection_count, 100);
        assert_eq!(h.close_frame_count, 10);
    }

    #[test]
    fn test_websocket_to_streaming_relay_basic() {
        let r = websocket_to_streaming_relay(CONN_ID, 300, 64_000, 0x02, true);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.opcode, 0x02);
        assert_eq!(r.mask_applied, 1);
    }

    #[test]
    fn test_websocket_connection_id_hash_differs() {
        // 異なる接続IDは異なるハッシュを生成する
        let r1 = websocket_to_db_record(b"conn-1", 100, 1000, 0x01, false);
        let r2 = websocket_to_db_record(b"conn-2", 100, 1000, 0x01, false);
        assert_ne!(r1.connection_id_hash, r2.connection_id_hash);
    }
}
