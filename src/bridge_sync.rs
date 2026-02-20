//! Sync bridges — ALICE-Sync ↔ DB, Analytics, CDN, Cache
//!
//! 7 bridges connecting the state-synchronization layer to the ALICE ecosystem.
//! Covers sync session persistence, frame telemetry, CDN-based peer discovery,
//! cache-assisted state prefetch, and analytics feed from input frame data.

use alice_sync::InputFrame;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Sync → DB (session persistence) ────────────────────────────

/// Sync session record for ALICE-DB persistence.
///
/// One record per completed sync session.  `session_hash` is derived from
/// `session_id` bytes so duplicate submissions are detectable.
pub struct SyncDbSessionRecord {
    /// FNV-1a hash of the session identifier.
    pub session_hash: u64,
    /// Number of input frames processed in this session.
    pub frame_count: u64,
    /// Session duration in milliseconds.
    pub duration_ms: u64,
    /// Mean round-trip time across all peers, in milliseconds.
    pub mean_rtt_ms: f64,
    /// Packet loss rate in permille.
    pub loss_permille: u32,
    /// Number of peers in the session.
    pub peer_count: u8,
}

/// Build a sync session record for ALICE-DB persistence.
///
/// `loss_permille` is computed branchlessly from `lost_frames / total_frames`.
#[inline]
pub fn sync_to_db_session_record(
    session_id: &str,
    frame_count: u64,
    duration_ms: u64,
    mean_rtt_ms: f64,
    lost_frames: u64,
    peer_count: u8,
) -> SyncDbSessionRecord {
    let session_hash = fnv1a(session_id.as_bytes());
    // Branchless permille: guard frame_count=0 with max(1).
    let total_safe = frame_count.max(1);
    let loss_permille = (lost_frames.min(total_safe).wrapping_mul(1_000) / total_safe) as u32;
    SyncDbSessionRecord {
        session_hash,
        frame_count,
        duration_ms,
        mean_rtt_ms,
        loss_permille,
        peer_count,
    }
}

// ── Bridge 2: Sync → Analytics (frame telemetry) ─────────────────────────

/// Input frame telemetry for ALICE-Analytics.
///
/// Derived from `alice_sync::InputFrame` so the analytics pipeline can
/// build per-player latency histograms and detect input prediction errors.
pub struct SyncAnalyticsFrameEvent {
    /// FNV-1a hash of the frame's action + movement payload — deduplication key.
    pub payload_hash: u64,
    /// Simulation frame number from the input frame.
    pub frame: u64,
    /// Player identifier.
    pub player_id: u8,
    /// Action bitmask from the input frame.
    pub actions: u32,
    /// Non-zero axis count (movement + aim components that are non-zero).
    pub active_axes: u8,
}

/// Convert an `InputFrame` into a telemetry event for ALICE-Analytics.
///
/// The movement and aim arrays are hashed as raw i16 bytes for a compact
/// deduplication key without heap allocation.
#[inline]
pub fn sync_to_analytics_frame_event(frame: &InputFrame) -> SyncAnalyticsFrameEvent {
    // Hash movement + aim as raw bytes (6 × i16 = 12 bytes per array = 24 bytes total).
    let mut data = [0u8; 24];
    for (i, &v) in frame.movement.iter().enumerate() {
        let bytes = v.to_le_bytes();
        data[i * 2]     = bytes[0];
        data[i * 2 + 1] = bytes[1];
    }
    for (i, &v) in frame.aim.iter().enumerate() {
        let bytes = v.to_le_bytes();
        data[6 + i * 2]     = bytes[0];
        data[6 + i * 2 + 1] = bytes[1];
    }
    let payload_hash = fnv1a(&data);
    // Count non-zero axes in movement and aim (branchless population count proxy).
    let active_axes = frame.movement.iter().filter(|&&v| v != 0).count()
        + frame.aim.iter().filter(|&&v| v != 0).count();
    SyncAnalyticsFrameEvent {
        payload_hash,
        frame: frame.frame,
        player_id: frame.player_id,
        actions: frame.actions,
        active_axes: active_axes.min(255) as u8,
    }
}

// ── Bridge 3: Sync → CDN (peer discovery via CDN metadata) ───────────────

/// Peer discovery request for ALICE-CDN metadata service.
///
/// When a client cannot reach the signalling server directly it issues a
/// CDN metadata lookup to discover peer endpoints stored as lightweight
/// blobs in the CDN edge network.
pub struct SyncCdnPeerDiscovery {
    /// FNV-1a hash of the room/session identifier.
    pub room_hash: u64,
    /// Byte length of the room identifier string.
    pub room_id_bytes: usize,
    /// Maximum number of peers to return.
    pub max_peers: u8,
    /// Preferred CDN region for locality-aware selection (0=any).
    pub preferred_region: u8,
    /// MIME type for CDN content negotiation.
    pub content_type: &'static str,
    /// Suggested TTL for the peer list in seconds.
    pub ttl_secs: u32,
}

/// Build a peer discovery request for ALICE-CDN.
///
/// TTL is 10 s — peer lists are volatile (peers join/leave frequently).
#[inline]
pub fn sync_to_cdn_peer_discovery(
    room_id: &str,
    max_peers: u8,
    preferred_region: u8,
) -> SyncCdnPeerDiscovery {
    SyncCdnPeerDiscovery {
        room_hash: fnv1a(room_id.as_bytes()),
        room_id_bytes: room_id.len(),
        max_peers,
        preferred_region,
        content_type: "application/x-alice-peers",
        ttl_secs: 10,
    }
}

// ── Bridge 4: Sync → Cache (state snapshot caching) ─────────────────────

/// State snapshot cache entry for ALICE-Cache.
///
/// Stores a compact serialized game/application state so that newly joining
/// peers can bootstrap quickly from cache without full replay.
pub struct SyncCacheStateSnapshot {
    /// FNV-1a hash of the state payload — primary cache key.
    pub state_hash: u64,
    /// Snapshot frame number (last included simulation frame).
    pub snapshot_frame: u64,
    /// Serialized state size in bytes.
    pub state_bytes: usize,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of peers whose state is included in this snapshot.
    pub peer_count: u8,
}

/// Build a state snapshot cache entry for ALICE-Cache.
///
/// TTL is set to 30 s — short enough to stay fresh during active play,
/// long enough to serve late-joining peers without reprobing the server.
#[inline]
pub fn sync_to_cache_state_snapshot(
    state_data: &[u8],
    snapshot_frame: u64,
    peer_count: u8,
) -> SyncCacheStateSnapshot {
    SyncCacheStateSnapshot {
        state_hash: fnv1a(state_data),
        snapshot_frame,
        state_bytes: state_data.len(),
        ttl_secs: 30,
        peer_count,
    }
}

// ── Bridge 5: Sync → DB (input frame log) ────────────────────────────────

/// Input frame log record for ALICE-DB.
///
/// Persists a compact summary of each input frame for replay and debugging.
/// Raw axis data is not stored — only FNV-1a hashes to avoid excessive write
/// amplification.
pub struct SyncDbFrameLog {
    /// FNV-1a hash of the movement + aim payload.
    pub payload_hash: u64,
    /// Simulation frame number.
    pub frame: u64,
    /// Player identifier.
    pub player_id: u8,
    /// Action bitmask.
    pub actions: u32,
}

/// Build an input frame log record for ALICE-DB.
#[inline]
pub fn sync_to_db_frame_log(frame: &InputFrame) -> SyncDbFrameLog {
    let mut data = [0u8; 24];
    for (i, &v) in frame.movement.iter().enumerate() {
        let b = v.to_le_bytes();
        data[i * 2]     = b[0];
        data[i * 2 + 1] = b[1];
    }
    for (i, &v) in frame.aim.iter().enumerate() {
        let b = v.to_le_bytes();
        data[6 + i * 2]     = b[0];
        data[6 + i * 2 + 1] = b[1];
    }
    SyncDbFrameLog {
        payload_hash: fnv1a(&data),
        frame: frame.frame,
        player_id: frame.player_id,
        actions: frame.actions,
    }
}

// ── Bridge 6: Sync → Cache (peer RTT cache entry) ────────────────────────

/// Peer RTT cache entry for ALICE-Cache.
///
/// Stores the most recent round-trip time measurement to a peer so that
/// subsequent connection attempts can select the lowest-latency peer from
/// cache without issuing new probe packets.
pub struct SyncCachePeerRtt {
    /// FNV-1a hash of the peer identifier — cache key.
    pub peer_hash: u64,
    /// Most recently measured RTT in milliseconds.
    pub rtt_ms: u32,
    /// Exponentially smoothed RTT (EWMA, α=1/8) in milliseconds.
    pub rtt_ewma_ms: u32,
    /// Number of measurements averaged.
    pub sample_count: u32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build a peer RTT cache entry for ALICE-Cache.
///
/// `rtt_ewma_ms` is updated as `ewma = 7/8 * prev_ewma + 1/8 * rtt_ms`
/// using integer arithmetic (branchless, no float).
#[inline]
pub fn sync_to_cache_peer_rtt(
    peer_id: &str,
    rtt_ms: u32,
    prev_ewma_ms: u32,
    sample_count: u32,
) -> SyncCachePeerRtt {
    let peer_hash = fnv1a(peer_id.as_bytes());
    // EWMA: 7/8 * prev + 1/8 * new (integer arithmetic, no division).
    let rtt_ewma_ms = prev_ewma_ms.wrapping_mul(7).wrapping_add(rtt_ms) >> 3;
    SyncCachePeerRtt {
        peer_hash,
        rtt_ms,
        rtt_ewma_ms,
        sample_count: sample_count.saturating_add(1),
        ttl_secs: 60,
    }
}

// ── Bridge 7: Sync → Analytics (session health summary) ──────────────────

/// Session health summary for ALICE-Analytics.
///
/// Aggregates per-session counters into a compact summary struct so that
/// the analytics pipeline can track session quality trends over time.
pub struct SyncAnalyticsSessionHealth {
    /// FNV-1a hash of the session identifier.
    pub session_hash: u64,
    /// Total frames processed.
    pub total_frames: u64,
    /// Frames delivered late (after their deadline).
    pub late_frames: u64,
    /// Late frame rate in permille.
    pub late_rate_permille: u32,
    /// Mean inter-frame gap in milliseconds.
    pub mean_gap_ms: f64,
    /// Peak inter-frame gap (jitter spike) in milliseconds.
    pub peak_gap_ms: f64,
    /// Number of active peers at session end.
    pub peer_count: u8,
}

/// Build a session health summary for ALICE-Analytics.
#[inline]
pub fn sync_to_analytics_session_health(
    session_id: &str,
    total_frames: u64,
    late_frames: u64,
    mean_gap_ms: f64,
    peak_gap_ms: f64,
    peer_count: u8,
) -> SyncAnalyticsSessionHealth {
    let session_hash = fnv1a(session_id.as_bytes());
    let total_safe = total_frames.max(1);
    let late_rate_permille = (late_frames.min(total_safe).wrapping_mul(1_000) / total_safe) as u32;
    SyncAnalyticsSessionHealth {
        session_hash,
        total_frames,
        late_frames,
        late_rate_permille,
        mean_gap_ms,
        peak_gap_ms,
        peer_count,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(frame: u64, player_id: u8) -> InputFrame {
        InputFrame::new(frame, player_id)
            .with_movement(100, -50, 0)
    }

    #[test]
    fn test_sync_to_db_session_record_loss_permille() {
        let rec = sync_to_db_session_record("game-room-1", 1_000, 60_000, 25.0, 10, 4);
        assert_ne!(rec.session_hash, 0);
        assert_eq!(rec.frame_count, 1_000);
        // 10 lost / 1000 total * 1000 = 10 permille.
        assert_eq!(rec.loss_permille, 10);
        assert_eq!(rec.peer_count, 4);
    }

    #[test]
    fn test_sync_to_db_session_record_zero_frames_no_panic() {
        let rec = sync_to_db_session_record("empty-session", 0, 0, 0.0, 0, 0);
        assert_eq!(rec.frame_count, 0);
        assert_eq!(rec.loss_permille, 0);
    }

    #[test]
    fn test_sync_to_analytics_frame_event() {
        let frame = make_frame(42, 1);
        let ev = sync_to_analytics_frame_event(&frame);
        assert_ne!(ev.payload_hash, 0);
        assert_eq!(ev.frame, 42);
        assert_eq!(ev.player_id, 1);
        assert_eq!(ev.actions, 0);
        // movement = [100, -50, 0] → 2 non-zero, aim = [0,0,0] → 0 non-zero.
        assert_eq!(ev.active_axes, 2);
    }

    #[test]
    fn test_sync_to_analytics_frame_event_different_movement_different_hash() {
        let f1 = make_frame(1, 0);
        let f2 = InputFrame::new(1, 0).with_movement(0, 0, 0);
        let e1 = sync_to_analytics_frame_event(&f1);
        let e2 = sync_to_analytics_frame_event(&f2);
        assert_ne!(e1.payload_hash, e2.payload_hash, "different movement → different hash");
    }

    #[test]
    fn test_sync_to_cdn_peer_discovery() {
        let req = sync_to_cdn_peer_discovery("room-abc", 8, 0);
        assert_ne!(req.room_hash, 0);
        assert_eq!(req.room_id_bytes, "room-abc".len());
        assert_eq!(req.max_peers, 8);
        assert_eq!(req.ttl_secs, 10);
        assert_eq!(req.content_type, "application/x-alice-peers");
    }

    #[test]
    fn test_sync_to_cache_state_snapshot() {
        let state = vec![0u8; 2048];
        let snap = sync_to_cache_state_snapshot(&state, 500, 4);
        assert_ne!(snap.state_hash, 0);
        assert_eq!(snap.snapshot_frame, 500);
        assert_eq!(snap.state_bytes, 2048);
        assert_eq!(snap.ttl_secs, 30);
        assert_eq!(snap.peer_count, 4);
    }

    #[test]
    fn test_sync_to_db_frame_log() {
        let frame = make_frame(7, 2);
        let log = sync_to_db_frame_log(&frame);
        assert_ne!(log.payload_hash, 0);
        assert_eq!(log.frame, 7);
        assert_eq!(log.player_id, 2);
        assert_eq!(log.actions, 0);
    }

    #[test]
    fn test_sync_to_cache_peer_rtt_ewma() {
        // First measurement: prev_ewma = 0, rtt = 20ms.
        // ewma = (0*7 + 20) >> 3 = 2 ms.
        let e1 = sync_to_cache_peer_rtt("peer-x", 20, 0, 0);
        assert_ne!(e1.peer_hash, 0);
        assert_eq!(e1.rtt_ms, 20);
        assert_eq!(e1.rtt_ewma_ms, 2);
        assert_eq!(e1.sample_count, 1);
        assert_eq!(e1.ttl_secs, 60);

        // Stable measurement: prev_ewma = 20, rtt = 20ms → ewma stays 20.
        let e2 = sync_to_cache_peer_rtt("peer-x", 20, 20, 1);
        assert_eq!(e2.rtt_ewma_ms, 20);
        assert_eq!(e2.sample_count, 2);
    }

    #[test]
    fn test_sync_to_analytics_session_health_late_rate() {
        let h = sync_to_analytics_session_health("session-z", 200, 20, 16.5, 50.0, 3);
        assert_ne!(h.session_hash, 0);
        // 20 late / 200 total * 1000 = 100 permille.
        assert_eq!(h.late_rate_permille, 100);
        assert!((h.mean_gap_ms - 16.5).abs() < f64::EPSILON);
        assert!((h.peak_gap_ms - 50.0).abs() < f64::EPSILON);
        assert_eq!(h.peer_count, 3);
    }

    #[test]
    fn test_sync_to_analytics_session_health_zero_frames_no_panic() {
        let h = sync_to_analytics_session_health("empty", 0, 0, 0.0, 0.0, 0);
        assert_eq!(h.late_rate_permille, 0);
    }
}
