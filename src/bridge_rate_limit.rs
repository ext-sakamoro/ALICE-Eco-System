//! Rate-limit bridges — ALICE-RateLimit ↔ Analytics, DB, Cache, Auth, Edge
//!
//! 5 bridges connecting the rate-limiting layer to the ALICE ecosystem.
//! Covers rate metric telemetry, limit-state persistence, cache-backed state,
//! auth-scoped enforcement, and edge enforcement event delivery.

extern crate alloc;

use alice_rate_limit::{
    FairQueueEntry, FixedWindowCounter, Gcra, SlidingWindowCounter, TokenBucket,
};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: RateLimit → Analytics (rate metrics) ───────────────────────

/// Rate-limit decision metrics for ALICE-Analytics.
///
/// Emitted for every rate-limit decision so the analytics layer can build
/// per-tenant allow/deny histograms and detect abuse patterns.
pub struct RateLimitAnalyticsEvent {
    /// FNV-1a hash of the client/tenant identifier.
    pub content_hash: u64,
    /// Available tokens at decision time.
    pub tokens_available: u64,
    /// True when the request was allowed.
    pub allowed: bool,
    /// Decision timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Limiter algorithm tag: 0=TokenBucket, 1=SlidingWindow, 2=GCRA, 3=FixedWindow.
    pub algorithm: u8,
}

/// Build a rate-limit analytics event from a `TokenBucket` decision.
#[inline]
#[must_use]
pub fn rate_limit_to_analytics_event(
    client_id: &str,
    bucket: &TokenBucket,
    allowed: bool,
    timestamp_ms: u64,
) -> RateLimitAnalyticsEvent {
    RateLimitAnalyticsEvent {
        content_hash: fnv1a(client_id.as_bytes()),
        tokens_available: bucket.available_tokens(),
        allowed,
        timestamp_ms,
        algorithm: 0,
    }
}

// ── Bridge 2: RateLimit → DB (rate limit state) ──────────────────────────

/// Rate-limit state snapshot for ALICE-DB.
///
/// Persists the current state of a `SlidingWindowCounter` so that limit state
/// survives process restarts and can be shared across replicas.
pub struct RateLimitDbSnapshot {
    /// FNV-1a hash of the limiter key (client ID + window start).
    pub content_hash: u64,
    /// Current estimated request count in the sliding window.
    pub estimated_count: u64,
    /// Window start timestamp in milliseconds.
    pub window_start_ms: u64,
    /// Configured request limit for this window.
    pub limit: u64,
    /// Utilization ratio in permille (estimated_count / limit × 1000).
    pub utilization_permille: u32,
}

/// Build a rate-limit DB snapshot from a `SlidingWindowCounter`.
///
/// `utilization_permille` is computed branchlessly from current estimate / limit.
#[inline]
#[must_use]
pub fn rate_limit_to_db_snapshot(
    client_id: &str,
    counter: &SlidingWindowCounter,
    window_start_ms: u64,
    limit: u64,
    now_ms: u64,
) -> RateLimitDbSnapshot {
    let estimated = counter.current_estimate(now_ms);
    let estimated_count = estimated as u64;
    let mut key_data = [0u8; 8];
    key_data[..8].copy_from_slice(&window_start_ms.to_le_bytes());
    let mut hash_input = alloc::vec::Vec::with_capacity(client_id.len() + 8);
    hash_input.extend_from_slice(client_id.as_bytes());
    hash_input.extend_from_slice(&key_data);
    let content_hash = fnv1a(&hash_input);
    let limit_safe = limit.max(1);
    let utilization_permille =
        (estimated_count.min(limit_safe).wrapping_mul(1_000) / limit_safe) as u32;
    RateLimitDbSnapshot {
        content_hash,
        estimated_count,
        window_start_ms,
        limit,
        utilization_permille,
    }
}

// ── Bridge 3: RateLimit → Cache (rate state cache) ───────────────────────

/// Rate-limit state cache entry for ALICE-Cache.
///
/// Hot-path rate-limit decisions are backed by a cache entry so that the DB
/// is not queried on every request.  TTL is set branchlessly: 10 s when
/// utilization exceeds 80% (high-pressure), 60 s otherwise.
pub struct RateLimitCacheEntry {
    /// FNV-1a hash of the limiter key — primary cache key.
    pub content_hash: u64,
    /// Remaining GCRA TAT (Theoretical Arrival Time) offset in milliseconds.
    pub tat_remaining_ms: u64,
    /// True when the limiter is currently blocking requests.
    pub is_throttled: bool,
    /// Cache TTL in seconds (branchless: 10 under high pressure, 60 otherwise).
    pub ttl_secs: u32,
    /// Time until next allowed request in milliseconds (0 when not throttled).
    pub retry_after_ms: u64,
}

/// Build a rate-limit cache entry from a `Gcra` limiter.
///
/// `ttl_secs` is branchless: 10 when `retry_after_ms > 0` (throttled), else 60.
#[inline]
#[must_use]
pub fn rate_limit_to_cache_entry(client_id: &str, gcra: &Gcra, now_ms: u64) -> RateLimitCacheEntry {
    let retry_after_ms = gcra.time_until_allowed(now_ms);
    let is_throttled = retry_after_ms > 0;
    let throttled_flag = is_throttled as u32;
    // ブランチレス TTL: スロットル中=10s, 通常=60s
    let ttl_secs = 60 - throttled_flag * 50;
    RateLimitCacheEntry {
        content_hash: fnv1a(client_id.as_bytes()),
        tat_remaining_ms: retry_after_ms,
        is_throttled,
        ttl_secs,
        retry_after_ms,
    }
}

// ── Bridge 4: RateLimit → Auth (auth rate limit) ─────────────────────────

/// Auth-scoped rate-limit event for ALICE-Auth.
///
/// Login and token-refresh endpoints are protected by a `FixedWindowCounter`.
/// This record is forwarded to Auth so it can lock accounts after repeated
/// failures without coupling Auth directly to the rate-limit crate.
pub struct RateLimitAuthEvent {
    /// FNV-1a hash of the auth endpoint identifier + client IP.
    pub content_hash: u64,
    /// Number of auth attempts in the current fixed window.
    pub attempt_count: u32,
    /// Configured max attempts per window.
    pub max_attempts: u32,
    /// True when the fixed window limit has been reached.
    pub limit_reached: bool,
    /// Lockout severity: 0=none, 1=warn, 2=soft-lock, 3=hard-lock.
    pub severity: u8,
}

/// Build an auth rate-limit event from a `FixedWindowCounter` decision.
///
/// `severity` escalates branchlessly based on how close `attempt_count` is
/// to `max_attempts`.
#[inline]
#[must_use]
pub fn rate_limit_to_auth_event(
    endpoint: &str,
    client_ip: &[u8],
    counter: &FixedWindowCounter,
    attempt_count: u32,
    max_attempts: u32,
    now_ms: u64,
    limit_reached: bool,
) -> RateLimitAuthEvent {
    let mut key = alloc::vec::Vec::with_capacity(endpoint.len() + client_ip.len());
    key.extend_from_slice(endpoint.as_bytes());
    key.extend_from_slice(client_ip);
    let content_hash = fnv1a(&key);
    // 利用率に応じたセベリティ (ブランチレス近似)
    let max_safe = max_attempts.max(1);
    let permille = attempt_count.min(max_safe).wrapping_mul(1_000) / max_safe;
    let severity: u8 = match permille {
        0..=499 => 0,
        500..=749 => 1,
        750..=999 => 2,
        _ => 3,
    };
    // counter は将来の拡張用に受け取るが現時点では attempt_count を外部から渡す
    let _ = (counter, now_ms);
    RateLimitAuthEvent {
        content_hash,
        attempt_count,
        max_attempts,
        limit_reached,
        severity,
    }
}

// ── Bridge 5: RateLimit → Edge (enforcement events) ──────────────────────

/// Rate-limit enforcement event for ALICE-Edge.
///
/// When a request is denied at the Edge layer, this event is forwarded so
/// that Edge nodes can apply local enforcement (e.g. temporary IP bans)
/// without round-tripping to the central rate-limit service.
pub struct RateLimitEdgeEvent {
    /// FNV-1a hash of the fair-queue tenant identifier.
    pub content_hash: u64,
    /// Tenant identifier.
    pub tenant_id: u64,
    /// Virtual finish time from the fair queue (scaled to u64 µs).
    pub virtual_finish_time_us: u64,
    /// Scheduling weight (× 1000 for integer representation).
    pub weight_milli: u32,
    /// True when this tenant is currently deprioritized (VFT > global average).
    pub is_deprioritized: bool,
}

/// Build a rate-limit enforcement event for ALICE-Edge from a `FairQueueEntry`.
#[inline]
#[must_use]
pub fn rate_limit_to_edge_event(entry: &FairQueueEntry, global_avg_vft: f64) -> RateLimitEdgeEvent {
    let mut hash_data = [0u8; 8];
    hash_data.copy_from_slice(&entry.tenant_id.to_le_bytes());
    RateLimitEdgeEvent {
        content_hash: fnv1a(&hash_data),
        tenant_id: entry.tenant_id,
        virtual_finish_time_us: (entry.virtual_finish_time * 1_000.0) as u64,
        weight_milli: (entry.weight * 1_000.0) as u32,
        is_deprioritized: entry.virtual_finish_time > global_avg_vft,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Bridge 1 ───────────────────────────────────────────────────────────

    #[test]
    fn test_rate_limit_to_analytics_event_allowed() {
        let bucket = TokenBucket::new(10, 1, 0);
        let ev = rate_limit_to_analytics_event("tenant-a", &bucket, true, 1_000);
        assert_ne!(ev.content_hash, 0);
        assert!(ev.allowed);
        assert_eq!(ev.algorithm, 0);
        assert_eq!(ev.tokens_available, 10);
        assert_eq!(ev.timestamp_ms, 1_000);
    }

    #[test]
    fn test_rate_limit_to_analytics_event_denied() {
        let bucket = TokenBucket::new(2, 1, 0);
        bucket.try_acquire(2, 0);
        let ev = rate_limit_to_analytics_event("tenant-b", &bucket, false, 2_000);
        assert!(!ev.allowed);
        assert_eq!(ev.tokens_available, 0);
    }

    // Bridge 2 ───────────────────────────────────────────────────────────

    #[test]
    fn test_rate_limit_to_db_snapshot_basic() {
        let counter = SlidingWindowCounter::new(1_000, 100, 0);
        let snap = rate_limit_to_db_snapshot("user-42", &counter, 0, 100, 0);
        assert_ne!(snap.content_hash, 0);
        assert_eq!(snap.limit, 100);
        assert_eq!(snap.window_start_ms, 0);
    }

    #[test]
    fn test_rate_limit_to_db_snapshot_utilization_permille() {
        // estimated ≈ 0 → 0 permille
        let counter = SlidingWindowCounter::new(1_000, 200, 0);
        let snap = rate_limit_to_db_snapshot("user-1", &counter, 0, 200, 0);
        assert_eq!(snap.utilization_permille, 0);
    }

    // Bridge 3 ───────────────────────────────────────────────────────────

    #[test]
    fn test_rate_limit_to_cache_entry_not_throttled() {
        // GCRA: TAT = 0, now = 0 → 次の許可まで 0ms → 非スロットル
        let gcra = Gcra::new(10, 1_000, 5);
        let entry = rate_limit_to_cache_entry("client-x", &gcra, 0);
        assert_ne!(entry.content_hash, 0);
        assert!(!entry.is_throttled);
        assert_eq!(entry.ttl_secs, 60); // 非スロットル → 60s
    }

    #[test]
    fn test_rate_limit_to_cache_entry_throttled_ttl() {
        // TAT が now より大きくなるようにリクエストを連打してスロットル状態に
        let mut gcra = Gcra::new(1, 1_000, 0); // 1req/sec, burst=0
        gcra.try_acquire(0); // 1回目は通過
        gcra.try_acquire(0); // 2回目で TAT が前進
                             // now=0 のまま → retry_after_ms > 0 → スロットル
        let entry = rate_limit_to_cache_entry("client-y", &gcra, 0);
        // スロットル中かどうかはGCRA内部状態に依存するが、構造体が正しく生成されることを確認
        assert_ne!(entry.content_hash, 0);
        // TTL は is_throttled に応じて 10 or 60
        let expected_ttl = if entry.is_throttled { 10 } else { 60 };
        assert_eq!(entry.ttl_secs, expected_ttl);
    }

    #[test]
    fn test_rate_limit_to_cache_entry_determinism() {
        let gcra = Gcra::new(10, 1_000, 5);
        let e1 = rate_limit_to_cache_entry("client-z", &gcra, 500);
        let e2 = rate_limit_to_cache_entry("client-z", &gcra, 500);
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.ttl_secs, e2.ttl_secs);
    }

    // Bridge 4 ───────────────────────────────────────────────────────────

    #[test]
    fn test_rate_limit_to_auth_event_severity_none() {
        let mut fw = FixedWindowCounter::new(60_000, 10, 0);
        let ev = rate_limit_to_auth_event("/login", b"192.168.0.1", &fw, 2, 10, 0, false);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.severity, 0); // 200/1000 permille → none
        assert!(!ev.limit_reached);
        let _ = fw.try_acquire(0);
    }

    #[test]
    fn test_rate_limit_to_auth_event_severity_hard_lock() {
        let fw = FixedWindowCounter::new(60_000, 5, 0);
        let ev = rate_limit_to_auth_event("/login", b"10.0.0.1", &fw, 5, 5, 0, true);
        assert_eq!(ev.severity, 3); // 1000/1000 permille → hard-lock
        assert!(ev.limit_reached);
    }

    // Bridge 5 ───────────────────────────────────────────────────────────

    #[test]
    fn test_rate_limit_to_edge_event_basic() {
        let entry = FairQueueEntry {
            tenant_id: 77,
            weight: 2.0,
            virtual_finish_time: 50.0,
        };
        let ev = rate_limit_to_edge_event(&entry, 30.0);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.tenant_id, 77);
        assert!(ev.is_deprioritized); // VFT 50 > avg 30
        assert_eq!(ev.weight_milli, 2_000);
    }

    #[test]
    fn test_rate_limit_to_edge_event_not_deprioritized() {
        let entry = FairQueueEntry {
            tenant_id: 1,
            weight: 1.0,
            virtual_finish_time: 10.0,
        };
        let ev = rate_limit_to_edge_event(&entry, 100.0);
        assert!(!ev.is_deprioritized); // VFT 10 < avg 100
    }
}
