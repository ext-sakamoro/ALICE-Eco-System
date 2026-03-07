//! Circuit bridges — ALICE-Circuit ↔ Analytics, DB, Cache, Risk, Edge
//!
//! 5 bridges connecting the fault-tolerance layer to the ALICE ecosystem.

use alice_circuit::{
    health_from_error_rate, retry_delay, Bulkhead, CircuitBreaker, CircuitState, HealthStatus,
    RetryPolicy,
};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// `CircuitState` を u8 に変換（`as u8` キャスト禁止ルール準拠）
#[inline(always)]
fn state_to_u8(state: CircuitState) -> u8 {
    match state {
        CircuitState::Closed => 0,
        CircuitState::Open => 1,
        CircuitState::HalfOpen => 2,
    }
}

/// `HealthStatus` を u8 に変換
#[inline(always)]
fn health_to_u8(status: HealthStatus) -> u8 {
    match status {
        HealthStatus::Healthy => 0,
        HealthStatus::Degraded => 1,
        HealthStatus::Unhealthy => 2,
    }
}

// ── Bridge 1: Circuit → Analytics (circuit state metrics) ─────────────────

/// Circuit state metrics event for ALICE-Analytics.
///
/// Emitted on every circuit state transition and periodically so the analytics
/// layer can compute MTTR, failure rates, and Half-Open probe success ratios.
pub struct CircuitAnalyticsEvent {
    /// FNV-1a hash of the service name — analytics stream key.
    pub content_hash: u64,
    /// Circuit state as u8: 0=Closed, 1=Open, 2=HalfOpen.
    pub state: u8,
    /// Current failure count.
    pub failure_count: u32,
    /// Current success count.
    pub success_count: u32,
    /// Time since last failure in milliseconds.
    pub since_failure_ms: u64,
    /// Health status derived from failure_count / failure_threshold ratio.
    pub health: u8,
}

/// Build a circuit state metrics event for ALICE-Analytics.
///
/// `service_name` identifies the downstream dependency being guarded.
/// `now_ms` is the current epoch timestamp in milliseconds.
#[inline]
#[must_use]
pub fn circuit_to_analytics_event(
    service_name: &str,
    breaker: &CircuitBreaker,
    now_ms: u64,
) -> CircuitAnalyticsEvent {
    let content_hash = fnv1a(service_name.as_bytes());
    let since_failure_ms = now_ms.saturating_sub(breaker.last_failure_time);
    // エラー率をfailure_count/failure_thresholdで近似
    let error_rate = breaker.failure_count as f64 / breaker.failure_threshold.max(1) as f64;
    let health = health_to_u8(health_from_error_rate(error_rate.min(1.0)));
    CircuitAnalyticsEvent {
        content_hash,
        state: state_to_u8(breaker.state),
        failure_count: breaker.failure_count,
        success_count: breaker.success_count,
        since_failure_ms,
        health,
    }
}

// ── Bridge 2: Circuit → DB (circuit state persistence) ────────────────────

/// Circuit state persistence record for ALICE-DB.
///
/// Written when the circuit transitions between states so that distributed
/// nodes can reload the circuit state after a restart instead of starting
/// from Closed and immediately re-tripping.
pub struct CircuitDbRecord {
    /// FNV-1a hash of the service name — DB row key.
    pub content_hash: u64,
    /// Circuit state as u8.
    pub state: u8,
    /// Failure count at the time of persistence.
    pub failure_count: u32,
    /// Failure threshold configuration.
    pub failure_threshold: u32,
    /// Recovery timeout configuration in milliseconds.
    pub recovery_timeout_ms: u64,
    /// Last failure timestamp in milliseconds.
    pub last_failure_ms: u64,
}

/// Build a circuit state persistence record for ALICE-DB.
#[inline]
#[must_use]
pub fn circuit_to_db_record(service_name: &str, breaker: &CircuitBreaker) -> CircuitDbRecord {
    let content_hash = fnv1a(service_name.as_bytes());
    CircuitDbRecord {
        content_hash,
        state: state_to_u8(breaker.state),
        failure_count: breaker.failure_count,
        failure_threshold: breaker.failure_threshold,
        recovery_timeout_ms: breaker.recovery_timeout,
        last_failure_ms: breaker.last_failure_time,
    }
}

// ── Bridge 3: Circuit → Cache (circuit state cache) ───────────────────────

/// Cached circuit state for ALICE-Cache.
///
/// Hot-path request handlers read the cached state to avoid locking the
/// circuit breaker mutex on every call.
/// TTL is branchlessly set to 5 seconds when the circuit is Open (short TTL
/// so recovery is detected quickly) and 30 seconds when Closed or HalfOpen.
pub struct CircuitCacheEntry {
    /// FNV-1a hash of the service name — cache key.
    pub content_hash: u64,
    /// Circuit state as u8.
    pub state: u8,
    /// Failure count snapshot.
    pub failure_count: u32,
    /// Cache TTL in seconds (branchless: 5 when Open, 30 otherwise).
    pub ttl_secs: u32,
    /// True when the circuit allows requests.
    pub allows_requests: bool,
}

/// Build a cached circuit state entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn circuit_to_cache_entry(service_name: &str, breaker: &CircuitBreaker) -> CircuitCacheEntry {
    let content_hash = fnv1a(service_name.as_bytes());
    let state_u8 = state_to_u8(breaker.state);
    // ブランチレスTTL: Open(1) → 5秒、それ以外 → 30秒
    let is_open = (state_u8 == 1) as u32;
    let ttl_secs = 30 - is_open * 25;
    let allows_requests = breaker.state != CircuitState::Open;
    CircuitCacheEntry {
        content_hash,
        state: state_u8,
        failure_count: breaker.failure_count,
        ttl_secs,
        allows_requests,
    }
}

// ── Bridge 4: Circuit → Risk (resilience → risk integration) ──────────────

/// Circuit risk assessment for ALICE-Risk.
///
/// Maps circuit state and bulkhead utilization to a composite risk score so
/// that the risk engine can factor service reliability into pricing,
/// rate-limiting, and SLA decisions.
pub struct CircuitRiskAssessment {
    /// FNV-1a hash of the service name — risk model key.
    pub content_hash: u64,
    /// Circuit state as u8.
    pub circuit_state: u8,
    /// Bulkhead utilization in permille (0-1000).
    pub bulkhead_permille: u32,
    /// Risk score: 0-100 (higher = more risk).
    pub risk_score: u8,
    /// Failure count.
    pub failure_count: u32,
    /// Health status as u8: 0=Healthy, 1=Degraded, 2=Unhealthy.
    pub health: u8,
}

/// Build a circuit risk assessment for ALICE-Risk.
///
/// Risk score is derived from circuit state (0→0, 2→40, 1→80) plus
/// bulkhead pressure (adds up to 20 points).
#[inline]
#[must_use]
pub fn circuit_to_risk_assessment(
    service_name: &str,
    breaker: &CircuitBreaker,
    bulkhead: &Bulkhead,
) -> CircuitRiskAssessment {
    let content_hash = fnv1a(service_name.as_bytes());
    let state_u8 = state_to_u8(breaker.state);
    // ベースリスクスコア: Closed=0, HalfOpen=40, Open=80
    let base_risk: u8 = match breaker.state {
        CircuitState::Closed => 0,
        CircuitState::HalfOpen => 40,
        CircuitState::Open => 80,
    };
    // バルクヘッド圧力の追加リスク (最大20点)
    let util = bulkhead.current.min(bulkhead.max_concurrent) as u64 * 1_000
        / bulkhead.max_concurrent.max(1) as u64;
    let pressure_risk = (util * 20 / 1_000) as u8;
    let risk_score = base_risk.saturating_add(pressure_risk);
    let bulkhead_permille = util as u32;
    let error_rate = breaker.failure_count as f64 / breaker.failure_threshold.max(1) as f64;
    let health = health_to_u8(health_from_error_rate(error_rate.min(1.0)));
    CircuitRiskAssessment {
        content_hash,
        circuit_state: state_u8,
        bulkhead_permille,
        risk_score,
        failure_count: breaker.failure_count,
        health,
    }
}

// ── Bridge 5: Circuit → Edge (circuit events) ─────────────────────────────

/// Circuit state change event for ALICE-Edge.
///
/// Forwarded to edge agents so they can fail-fast locally without sending
/// requests to a known-open circuit, reducing unnecessary WAN traffic.
pub struct CircuitEdgeEvent {
    /// FNV-1a hash of the service name — edge routing key.
    pub content_hash: u64,
    /// New circuit state as u8.
    pub state: u8,
    /// Event timestamp in milliseconds.
    pub event_at_ms: u64,
    /// Retry delay in milliseconds for the next probe (from retry policy).
    pub retry_delay_ms: u64,
    /// Failure count at transition time.
    pub failure_count: u32,
    /// True when edge agents should block outgoing requests.
    pub block_edge: bool,
}

/// Build a circuit state change event for ALICE-Edge.
///
/// `attempt` is the current retry attempt used to compute `retry_delay_ms`.
#[inline]
#[must_use]
pub fn circuit_to_edge_event(
    service_name: &str,
    breaker: &CircuitBreaker,
    policy: RetryPolicy,
    attempt: u32,
    event_at_ms: u64,
) -> CircuitEdgeEvent {
    let content_hash = fnv1a(service_name.as_bytes());
    let retry_delay_ms = retry_delay(policy, attempt).unwrap_or(0);
    let block_edge = breaker.state == CircuitState::Open;
    CircuitEdgeEvent {
        content_hash,
        state: state_to_u8(breaker.state),
        event_at_ms,
        retry_delay_ms,
        failure_count: breaker.failure_count,
        block_edge,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_closed_breaker() -> CircuitBreaker {
        CircuitBreaker::new(3, 2, 5_000)
    }

    fn make_open_breaker() -> CircuitBreaker {
        let mut cb = CircuitBreaker::new(3, 2, 5_000);
        cb.record_failure(100);
        cb.record_failure(200);
        cb.record_failure(300);
        cb
    }

    // ── Bridge 1 ──────────────────────────────────────────────────────────

    #[test]
    fn test_analytics_event_hash_nonzero() {
        let cb = make_closed_breaker();
        let ev = circuit_to_analytics_event("payment-service", &cb, 1_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_analytics_event_closed_state() {
        let cb = make_closed_breaker();
        let ev = circuit_to_analytics_event("auth-service", &cb, 2_000);
        assert_eq!(ev.state, 0); // Closed
        assert_eq!(ev.failure_count, 0);
        assert_eq!(ev.health, 0); // Healthy
    }

    #[test]
    fn test_analytics_event_open_state() {
        let cb = make_open_breaker();
        let ev = circuit_to_analytics_event("db-service", &cb, 5_000);
        assert_eq!(ev.state, 1); // Open
        assert_eq!(ev.failure_count, 3);
    }

    // ── Bridge 2 ──────────────────────────────────────────────────────────

    #[test]
    fn test_db_record_hash_nonzero() {
        let cb = make_closed_breaker();
        let rec = circuit_to_db_record("cache-service", &cb);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_db_record_fields() {
        let cb = make_open_breaker();
        let rec = circuit_to_db_record("mq-service", &cb);
        assert_eq!(rec.state, 1); // Open
        assert_eq!(rec.failure_count, 3);
        assert_eq!(rec.failure_threshold, 3);
        assert_eq!(rec.recovery_timeout_ms, 5_000);
    }

    // ── Bridge 3 ──────────────────────────────────────────────────────────

    #[test]
    fn test_cache_entry_closed_ttl() {
        let cb = make_closed_breaker();
        let entry = circuit_to_cache_entry("api-gateway", &cb);
        assert_ne!(entry.content_hash, 0);
        // Closed → TTL = 30
        assert_eq!(entry.ttl_secs, 30);
        assert!(entry.allows_requests);
    }

    #[test]
    fn test_cache_entry_open_ttl() {
        let cb = make_open_breaker();
        let entry = circuit_to_cache_entry("api-gateway", &cb);
        // Open → TTL = 5
        assert_eq!(entry.ttl_secs, 5);
        assert!(!entry.allows_requests);
    }

    #[test]
    fn test_cache_entry_determinism() {
        let cb = make_closed_breaker();
        let e1 = circuit_to_cache_entry("svc", &cb);
        let e2 = circuit_to_cache_entry("svc", &cb);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 4 ──────────────────────────────────────────────────────────

    #[test]
    fn test_risk_assessment_hash_nonzero() {
        let cb = make_closed_breaker();
        let bh = Bulkhead::new(10);
        let ra = circuit_to_risk_assessment("payment", &cb, &bh);
        assert_ne!(ra.content_hash, 0);
    }

    #[test]
    fn test_risk_assessment_open_high_score() {
        let cb = make_open_breaker();
        let bh = Bulkhead::new(10);
        let ra = circuit_to_risk_assessment("payment", &cb, &bh);
        // Open → base risk = 80
        assert!(ra.risk_score >= 80);
        assert_eq!(ra.circuit_state, 1);
    }

    // ── Bridge 5 ──────────────────────────────────────────────────────────

    #[test]
    fn test_edge_event_hash_nonzero() {
        let cb = make_open_breaker();
        let policy = RetryPolicy::Fixed {
            delay_ms: 500,
            max_retries: 3,
        };
        let ev = circuit_to_edge_event("search-service", &cb, policy, 0, 5_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_edge_event_open_blocks() {
        let cb = make_open_breaker();
        let policy = RetryPolicy::Exponential {
            base_ms: 100,
            max_delay_ms: 10_000,
            max_retries: 5,
        };
        let ev = circuit_to_edge_event("search-service", &cb, policy, 1, 5_000);
        assert!(ev.block_edge);
        assert_eq!(ev.retry_delay_ms, 200); // base * 2^1
    }

    #[test]
    fn test_edge_event_closed_no_block() {
        let cb = make_closed_breaker();
        let policy = RetryPolicy::Fixed {
            delay_ms: 100,
            max_retries: 3,
        };
        let ev = circuit_to_edge_event("search-service", &cb, policy, 0, 1_000);
        assert!(!ev.block_edge);
    }
}
