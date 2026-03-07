//! Realtime bridges — ALICE-Realtime ↔ Analytics, DB, Cache, CDN, Edge
//!
//! 5 bridges connecting the real-time pub/sub layer to the ALICE ecosystem.
//! Covers session metric telemetry, session record persistence, session state
//! caching, CDN-backed realtime delivery, and Edge event forwarding.

use alice_realtime::{backpressure_check, BackpressureAction, Client, PubSubChannel, Room};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Realtime → Analytics (session metrics) ─────────────────────

/// Realtime session metric event for ALICE-Analytics.
///
/// Emitted when a client connects, disconnects, or changes subscription state.
/// Enables the analytics layer to build concurrent-user and channel-popularity
/// dashboards without coupling directly to the realtime engine.
pub struct RealtimeAnalyticsSessionEvent {
    /// FNV-1a hash of the client ID — analytics stream key.
    pub content_hash: u64,
    /// Client identifier.
    pub client_id: u64,
    /// Number of active subscriptions for this client.
    pub subscription_count: u32,
    /// Send-buffer utilization in permille.
    pub buffer_utilization_permille: u32,
    /// Smoothed RTT estimate in milliseconds (rounded).
    pub rtt_avg_ms: u32,
    /// Backpressure state: 0=Allow, 1=Throttle, 2=Disconnect.
    pub backpressure: u8,
}

/// Build a realtime session metric event for ALICE-Analytics from a `Client`.
#[inline]
#[must_use]
pub fn realtime_to_analytics_session_event(client: &Client) -> RealtimeAnalyticsSessionEvent {
    let mut id_bytes = [0u8; 8];
    id_bytes.copy_from_slice(&client.id.to_le_bytes());
    let content_hash = fnv1a(&id_bytes);
    let buffer_utilization_permille = (client.buffer_usage() * 1_000.0) as u32;
    let bp = backpressure_check(client.buffer_usage());
    let backpressure: u8 = match bp {
        BackpressureAction::Allow => 0,
        BackpressureAction::Throttle => 1,
        BackpressureAction::Disconnect => 2,
    };
    RealtimeAnalyticsSessionEvent {
        content_hash,
        client_id: client.id,
        subscription_count: client.subscriptions.len() as u32,
        buffer_utilization_permille,
        rtt_avg_ms: client.rtt.avg_ms as u32,
        backpressure,
    }
}

// ── Bridge 2: Realtime → DB (session records) ────────────────────────────

/// Realtime session record for ALICE-DB.
///
/// Written at connection close to persist session duration and subscription
/// history for audit trails and capacity planning.
pub struct RealtimeDbSessionRecord {
    /// FNV-1a hash of the client ID.
    pub content_hash: u64,
    /// Client identifier.
    pub client_id: u64,
    /// Session start timestamp in milliseconds.
    pub connected_at_ms: u64,
    /// Number of subscriptions held during the session.
    pub subscription_count: u32,
    /// Peak RTT observed during the session in milliseconds (rounded).
    pub peak_rtt_ms: u32,
    /// True when the client was ever in Throttle or Disconnect backpressure state.
    pub had_backpressure: bool,
}

/// Build a realtime session record for ALICE-DB from a `Client`.
#[inline]
#[must_use]
pub fn realtime_to_db_session_record(client: &Client, peak_rtt_ms: u32) -> RealtimeDbSessionRecord {
    let mut id_bytes = [0u8; 8];
    id_bytes.copy_from_slice(&client.id.to_le_bytes());
    let content_hash = fnv1a(&id_bytes);
    let bp = backpressure_check(client.buffer_usage());
    let had_backpressure = !matches!(bp, BackpressureAction::Allow);
    RealtimeDbSessionRecord {
        content_hash,
        client_id: client.id,
        connected_at_ms: client.connected_at_ms,
        subscription_count: client.subscriptions.len() as u32,
        peak_rtt_ms,
        had_backpressure,
    }
}

// ── Bridge 3: Realtime → Cache (session cache) ───────────────────────────

/// Realtime session cache entry for ALICE-Cache.
///
/// Caches subscription state so that reconnecting clients can restore their
/// subscriptions without replaying the full signalling sequence.
/// TTL is set branchlessly: 30 s for active (low-backpressure) sessions,
/// 10 s when the client is under backpressure.
pub struct RealtimeCacheSession {
    /// FNV-1a hash of the client ID — primary cache key.
    pub content_hash: u64,
    /// Client identifier.
    pub client_id: u64,
    /// Number of active subscriptions.
    pub subscription_count: u32,
    /// Cache TTL in seconds (branchless: 10 under backpressure, 30 otherwise).
    pub ttl_secs: u32,
    /// Send-buffer utilization in permille (for cache freshness decisions).
    pub buffer_permille: u32,
}

/// Build a realtime session cache entry for ALICE-Cache from a `Client`.
///
/// `ttl_secs` is branchless: 10 when under backpressure, else 30.
#[inline]
#[must_use]
pub fn realtime_to_cache_session(client: &Client) -> RealtimeCacheSession {
    let mut id_bytes = [0u8; 8];
    id_bytes.copy_from_slice(&client.id.to_le_bytes());
    let content_hash = fnv1a(&id_bytes);
    let bp = backpressure_check(client.buffer_usage());
    let under_pressure = (!matches!(bp, BackpressureAction::Allow)) as u32;
    // ブランチレス TTL: 圧力あり=10s, 通常=30s
    let ttl_secs = 30 - under_pressure * 20;
    let buffer_permille = (client.buffer_usage() * 1_000.0) as u32;
    RealtimeCacheSession {
        content_hash,
        client_id: client.id,
        subscription_count: client.subscriptions.len() as u32,
        ttl_secs,
        buffer_permille,
    }
}

// ── Bridge 4: Realtime → CDN (realtime delivery) ─────────────────────────

/// Realtime channel delivery descriptor for ALICE-CDN.
///
/// Fan-out messages for large channels (e.g. broadcast rooms) are offloaded
/// to the CDN edge so that the realtime server is not overwhelmed.  This
/// descriptor carries subscriber count and content-type for CDN routing.
pub struct RealtimeCdnDelivery {
    /// FNV-1a hash of the channel name.
    pub content_hash: u64,
    /// Number of subscribers on this channel.
    pub subscriber_count: u32,
    /// MIME type for CDN content negotiation.
    pub content_type: &'static str,
    /// Suggested CDN TTL in seconds (short because realtime data is volatile).
    pub ttl_secs: u32,
    /// True when fan-out should be CDN-assisted (subscriber_count > 1000).
    pub cdn_fanout: bool,
}

/// Build a realtime CDN delivery descriptor from a `PubSubChannel`.
#[inline]
#[must_use]
pub fn realtime_to_cdn_delivery(channel: &PubSubChannel) -> RealtimeCdnDelivery {
    let content_hash = fnv1a(channel.name.as_bytes());
    let subscriber_count = channel.subscriber_count() as u32;
    let cdn_fanout = subscriber_count > 1_000;
    RealtimeCdnDelivery {
        content_hash,
        subscriber_count,
        content_type: "application/x-alice-realtime",
        ttl_secs: 5,
        cdn_fanout,
    }
}

// ── Bridge 5: Realtime → Edge (realtime events) ──────────────────────────

/// Realtime room event for ALICE-Edge.
///
/// Edge nodes cache room membership so they can route inbound messages
/// to the correct realtime server without a central lookup on every frame.
pub struct RealtimeEdgeRoomEvent {
    /// FNV-1a hash of the room name.
    pub content_hash: u64,
    /// Current member count.
    pub member_count: u32,
    /// Maximum allowed members.
    pub max_members: u32,
    /// Fill ratio in permille (member_count / max_members × 1000).
    pub fill_permille: u32,
    /// True when the room is full and should be closed to new members.
    pub is_full: bool,
}

/// Build a realtime room event for ALICE-Edge from a `Room`.
#[inline]
#[must_use]
pub fn realtime_to_edge_room_event(room: &Room) -> RealtimeEdgeRoomEvent {
    let content_hash = fnv1a(room.name.as_bytes());
    let member_count = room.member_count() as u32;
    let max_members = room.max_members as u32;
    let max_safe = max_members.max(1);
    let fill_permille = member_count.min(max_safe).wrapping_mul(1_000) / max_safe;
    RealtimeEdgeRoomEvent {
        content_hash,
        member_count,
        max_members,
        fill_permille,
        is_full: room.is_full(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client(buffer_bytes: u64, buffer_cap: u64) -> Client {
        let mut c = Client::new(1_700_000_000_000, buffer_cap);
        c.send_buffer_bytes = buffer_bytes;
        c.subscribe("chat:general");
        c
    }

    // Bridge 1 ───────────────────────────────────────────────────────────

    #[test]
    fn test_realtime_to_analytics_session_event_basic() {
        let client = make_client(0, 65_536);
        let ev = realtime_to_analytics_session_event(&client);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.client_id, client.id);
        assert_eq!(ev.subscription_count, 1);
        assert_eq!(ev.backpressure, 0); // Allow
    }

    #[test]
    fn test_realtime_to_analytics_session_event_throttle() {
        // buffer at 85% → Throttle
        let client = make_client(8_500, 10_000);
        let ev = realtime_to_analytics_session_event(&client);
        assert_eq!(ev.backpressure, 1); // Throttle
        assert!(ev.buffer_utilization_permille > 800);
    }

    #[test]
    fn test_realtime_to_analytics_session_event_disconnect() {
        // buffer at 96% → Disconnect
        let client = make_client(9_600, 10_000);
        let ev = realtime_to_analytics_session_event(&client);
        assert_eq!(ev.backpressure, 2); // Disconnect
    }

    // Bridge 2 ───────────────────────────────────────────────────────────

    #[test]
    fn test_realtime_to_db_session_record_no_pressure() {
        let client = make_client(0, 65_536);
        let rec = realtime_to_db_session_record(&client, 42);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.peak_rtt_ms, 42);
        assert!(!rec.had_backpressure);
        assert_eq!(rec.connected_at_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_realtime_to_db_session_record_had_backpressure() {
        let client = make_client(9_800, 10_000);
        let rec = realtime_to_db_session_record(&client, 200);
        assert!(rec.had_backpressure);
    }

    // Bridge 3 ───────────────────────────────────────────────────────────

    #[test]
    fn test_realtime_to_cache_session_normal_ttl() {
        let client = make_client(0, 65_536);
        let entry = realtime_to_cache_session(&client);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 30); // 正常 → 30s
    }

    #[test]
    fn test_realtime_to_cache_session_pressure_ttl() {
        let client = make_client(8_500, 10_000); // Throttle
        let entry = realtime_to_cache_session(&client);
        assert_eq!(entry.ttl_secs, 10); // 圧力あり → 10s
    }

    #[test]
    fn test_realtime_to_cache_session_determinism() {
        let client = make_client(0, 65_536);
        let e1 = realtime_to_cache_session(&client);
        let e2 = realtime_to_cache_session(&client);
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.ttl_secs, e2.ttl_secs);
    }

    // Bridge 4 ───────────────────────────────────────────────────────────

    #[test]
    fn test_realtime_to_cdn_delivery_small_channel() {
        let mut ch = PubSubChannel::new("news:sports");
        ch.subscribe(1);
        ch.subscribe(2);
        let d = realtime_to_cdn_delivery(&ch);
        assert_ne!(d.content_hash, 0);
        assert_eq!(d.subscriber_count, 2);
        assert!(!d.cdn_fanout);
        assert_eq!(d.ttl_secs, 5);
    }

    #[test]
    fn test_realtime_to_cdn_delivery_large_channel_fanout() {
        let mut ch = PubSubChannel::new("broadcast:global");
        for id in 1..=1_001u64 {
            ch.subscribe(id);
        }
        let d = realtime_to_cdn_delivery(&ch);
        assert!(d.cdn_fanout);
    }

    // Bridge 5 ───────────────────────────────────────────────────────────

    #[test]
    fn test_realtime_to_edge_room_event_empty() {
        let room = Room::new("lobby", 100);
        let ev = realtime_to_edge_room_event(&room);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.member_count, 0);
        assert_eq!(ev.fill_permille, 0);
        assert!(!ev.is_full);
    }

    #[test]
    fn test_realtime_to_edge_room_event_full() {
        let mut room = Room::new("tiny-room", 2);
        room.join(1).unwrap();
        room.join(2).unwrap();
        let ev = realtime_to_edge_room_event(&room);
        assert_eq!(ev.member_count, 2);
        assert_eq!(ev.fill_permille, 1_000);
        assert!(ev.is_full);
    }
}
