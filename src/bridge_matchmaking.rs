//! Matchmaking bridges — ALICE-Matchmaking ↔ DB, Cache, Analytics, API, Notify
//!
//! 5 bridges connecting the matchmaking engine to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Matchmaking → DB (match history) ───────────────────────────

/// Match history record for ALICE-DB persistence.
pub struct MatchmakingDbRecord {
    /// Content hash over match-id bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the match identifier.
    pub match_id_hash: u64,
    /// Number of players in the match.
    pub player_count: u8,
    /// Number of players per team.
    pub team_size: u8,
    /// Average player rating (fixed-point × 100).
    pub rating_avg_x100: u32,
    /// Match quality score in the range [0, 10000] (fixed-point × 100).
    pub match_quality_x100: u32,
    /// Wait time experienced by the last matched player in milliseconds.
    pub wait_time_ms: u32,
    /// Match creation timestamp in nanoseconds.
    pub created_ns: u64,
}

/// Serialize a completed match for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn matchmaking_to_db_record(
    match_id: &[u8],
    player_count: u8,
    team_size: u8,
    rating_avg_x100: u32,
    match_quality_x100: u32,
    wait_time_ms: u32,
    created_ns: u64,
) -> MatchmakingDbRecord {
    let match_id_hash = fnv1a(match_id);
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&match_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&created_ns.to_le_bytes());
    MatchmakingDbRecord {
        content_hash: fnv1a(&key),
        match_id_hash,
        player_count,
        team_size,
        rating_avg_x100,
        match_quality_x100,
        wait_time_ms,
        created_ns,
    }
}

// ── Bridge 2: Matchmaking → Cache (player pool) ──────────────────────────

/// Player pool cache entry for ALICE-Cache.
pub struct MatchmakingCacheEntry {
    /// Content hash over pool snapshot.
    pub content_hash: u64,
    /// Current number of players waiting in the pool.
    pub pool_size: u32,
    /// Minimum rating in the current pool (fixed-point × 100).
    pub rating_min_x100: u32,
    /// Maximum rating in the current pool (fixed-point × 100).
    pub rating_max_x100: u32,
    /// Average wait time of queued players in milliseconds.
    pub avg_wait_ms: u32,
    /// Cache TTL in seconds (shortened when pool is large for faster refresh).
    pub ttl_secs: u32,
}

/// Build a player pool cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 5 s when pool_size exceeds 100 players
/// so matchmaking sees fresh data under high load.
#[inline]
#[must_use]
pub fn matchmaking_to_cache_entry(
    pool_size: u32,
    rating_min_x100: u32,
    rating_max_x100: u32,
    avg_wait_ms: u32,
) -> MatchmakingCacheEntry {
    // Branchless TTL: 30 s normal, 5 s when pool > 100.
    let large_pool = (pool_size > 100) as u32;
    let ttl_secs = 30_u32 - large_pool * 25_u32;
    let mut key = [0u8; 16];
    key[0..4].copy_from_slice(&pool_size.to_le_bytes());
    key[4..8].copy_from_slice(&rating_min_x100.to_le_bytes());
    key[8..12].copy_from_slice(&rating_max_x100.to_le_bytes());
    key[12..16].copy_from_slice(&avg_wait_ms.to_le_bytes());
    MatchmakingCacheEntry {
        content_hash: fnv1a(&key),
        pool_size,
        rating_min_x100,
        rating_max_x100,
        avg_wait_ms,
        ttl_secs,
    }
}

// ── Bridge 3: Matchmaking → Analytics (quality metrics) ──────────────────

/// Match quality metrics for ALICE-Analytics ingestion.
pub struct MatchmakingAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total matches formed in the reporting window.
    pub matches_formed: u64,
    /// Average match quality score [0.0, 100.0].
    pub avg_quality: f32,
    /// Average wait time across all matched players in milliseconds.
    pub avg_wait_ms: f32,
    /// Average player rating (fixed-point × 100).
    pub avg_rating_x100: u32,
    /// Window start timestamp in nanoseconds.
    pub window_start_ns: u64,
}

/// Build match quality metrics for ALICE-Analytics ingestion.
///
/// Averages use reciprocal multiply against `matches_formed`.
#[inline]
#[must_use]
pub fn matchmaking_to_analytics_metrics(
    matches_formed: u64,
    sum_quality_x100: u64,
    sum_wait_ms: u64,
    sum_rating_x100: u64,
    window_start_ns: u64,
) -> MatchmakingAnalyticsMetrics {
    let rcp = 1.0 / matches_formed.max(1) as f64;
    // Quality is stored × 100; divide back to [0, 100].
    let avg_quality = (sum_quality_x100 as f64 * rcp * 0.01) as f32;
    let avg_wait_ms = (sum_wait_ms as f64 * rcp) as f32;
    let avg_rating_x100 = (sum_rating_x100 as f64 * rcp) as u32;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&matches_formed.to_le_bytes());
    key[8..16].copy_from_slice(&window_start_ns.to_le_bytes());
    MatchmakingAnalyticsMetrics {
        content_hash: fnv1a(&key),
        matches_formed,
        avg_quality,
        avg_wait_ms,
        avg_rating_x100,
        window_start_ns,
    }
}

// ── Bridge 4: Matchmaking → API (match result response) ──────────────────

/// Match result payload for ALICE-API responses.
pub struct MatchmakingApiResult {
    /// Content hash over match-id + player list hash.
    pub content_hash: u64,
    /// FNV-1a hash of the match identifier.
    pub match_id_hash: u64,
    /// FNV-1a hash of the serialised player list.
    pub player_list_hash: u64,
    /// Number of players in the match.
    pub player_count: u8,
    /// Match quality score [0, 10000] (fixed-point × 100).
    pub match_quality_x100: u32,
    /// Server region code (FNV-1a hash of region string).
    pub region_hash: u64,
    /// Match start timestamp in nanoseconds.
    pub start_ns: u64,
}

/// Build a match result payload for ALICE-API.
#[inline]
#[must_use]
pub fn matchmaking_to_api_result(
    match_id: &[u8],
    player_list: &[u8],
    player_count: u8,
    match_quality_x100: u32,
    region: &[u8],
    start_ns: u64,
) -> MatchmakingApiResult {
    let match_id_hash = fnv1a(match_id);
    let player_list_hash = fnv1a(player_list);
    let region_hash = fnv1a(region);
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&match_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&player_list_hash.to_le_bytes());
    key[16..24].copy_from_slice(&start_ns.to_le_bytes());
    MatchmakingApiResult {
        content_hash: fnv1a(&key),
        match_id_hash,
        player_list_hash,
        player_count,
        match_quality_x100,
        region_hash,
        start_ns,
    }
}

// ── Bridge 5: Matchmaking → Notify (match found notification) ────────────

/// Match found notification payload for ALICE-Notify.
pub struct MatchmakingNotifyPayload {
    /// Content hash over match-id + recipient.
    pub content_hash: u64,
    /// FNV-1a hash of the match identifier.
    pub match_id_hash: u64,
    /// FNV-1a hash of the recipient player identifier.
    pub player_id_hash: u64,
    /// Time the player waited before being matched in milliseconds.
    pub wait_time_ms: u32,
    /// Match quality score [0, 10000] (fixed-point × 100).
    pub match_quality_x100: u32,
    /// Notification enqueue timestamp in nanoseconds.
    pub enqueued_ns: u64,
}

/// Build a match found notification for ALICE-Notify.
#[inline]
#[must_use]
pub fn matchmaking_to_notify_payload(
    match_id: &[u8],
    player_id: &[u8],
    wait_time_ms: u32,
    match_quality_x100: u32,
    enqueued_ns: u64,
) -> MatchmakingNotifyPayload {
    let match_id_hash = fnv1a(match_id);
    let player_id_hash = fnv1a(player_id);
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&match_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&player_id_hash.to_le_bytes());
    MatchmakingNotifyPayload {
        content_hash: fnv1a(&key),
        match_id_hash,
        player_id_hash,
        wait_time_ms,
        match_quality_x100,
        enqueued_ns,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const MATCH_ID: &[u8] = b"match:0001";
    const PLAYER_ID: &[u8] = b"player:007";
    const REGION: &[u8] = b"ap-northeast-1";
    const PLAYERS: &[u8] = b"[player:001,player:002]";

    #[test]
    fn test_matchmaking_to_db_record_hash_nonzero() {
        let rec = matchmaking_to_db_record(MATCH_ID, 10, 5, 150_000, 8_500, 2_300, 1_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.match_id_hash, 0);
    }

    #[test]
    fn test_matchmaking_to_db_record_fields() {
        let rec = matchmaking_to_db_record(MATCH_ID, 6, 3, 120_000, 9_200, 1_800, 2_000_000_000);
        assert_eq!(rec.player_count, 6);
        assert_eq!(rec.team_size, 3);
        assert_eq!(rec.rating_avg_x100, 120_000);
        assert_eq!(rec.match_quality_x100, 9_200);
        assert_eq!(rec.wait_time_ms, 1_800);
    }

    #[test]
    fn test_matchmaking_to_cache_entry_normal_ttl() {
        let entry = matchmaking_to_cache_entry(50, 100_000, 200_000, 500);
        assert_eq!(entry.ttl_secs, 30);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_matchmaking_to_cache_entry_large_pool_ttl() {
        // pool_size > 100 → TTL = 5 s.
        let entry = matchmaking_to_cache_entry(200, 90_000, 250_000, 800);
        assert_eq!(entry.ttl_secs, 5);
    }

    #[test]
    fn test_matchmaking_to_analytics_metrics_avg() {
        // 10 matches, sum_quality = 85000 (avg 8500 → 85.0), sum_wait = 20000 ms (avg 2000).
        let m = matchmaking_to_analytics_metrics(10, 85_000, 20_000, 1_200_000, 0);
        assert_ne!(m.content_hash, 0);
        assert!(
            (m.avg_quality - 85.0).abs() < 0.1,
            "avg_quality={}",
            m.avg_quality
        );
        assert!(
            (m.avg_wait_ms - 2_000.0).abs() < 1.0,
            "avg_wait_ms={}",
            m.avg_wait_ms
        );
    }

    #[test]
    fn test_matchmaking_to_analytics_metrics_zero_matches() {
        let m = matchmaking_to_analytics_metrics(0, 0, 0, 0, 0);
        assert_eq!(m.matches_formed, 0);
        assert_eq!(m.avg_quality, 0.0);
    }

    #[test]
    fn test_matchmaking_to_api_result_fields() {
        let r = matchmaking_to_api_result(MATCH_ID, PLAYERS, 10, 9_000, REGION, 5_000_000_000);
        assert_ne!(r.content_hash, 0);
        assert_ne!(r.region_hash, 0);
        assert_eq!(r.player_count, 10);
        assert_eq!(r.match_quality_x100, 9_000);
    }

    #[test]
    fn test_matchmaking_to_notify_payload_deterministic() {
        let a = matchmaking_to_notify_payload(MATCH_ID, PLAYER_ID, 1_500, 8_800, 100);
        let b = matchmaking_to_notify_payload(MATCH_ID, PLAYER_ID, 1_500, 8_800, 100);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.wait_time_ms, 1_500);
        assert_eq!(a.match_quality_x100, 8_800);
    }
}
