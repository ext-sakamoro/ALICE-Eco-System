//! GraphQL cross-domain bridges — API ↔ DB, Cache, Analytics, Auth, Monitor
//!
//! 5 bridges providing GraphQL query semantics by combining existing ALICE
//! crates.  No dedicated GraphQL crate is required — these bridges wire
//! API query patterns through the data pipeline for persistence, caching,
//! authorization, and performance monitoring.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: GraphQL → DB (query log) ──────────────────────────────────

/// GraphQL query log record for ALICE-DB persistence.
///
/// Stores every resolved GraphQL operation so that query patterns can be
/// analysed offline for schema optimisation and deprecation tracking.
pub struct GraphqlDbQueryLog {
    /// FNV-1a hash of the query string.
    pub content_hash: u64,
    /// Number of fields selected in the query.
    pub field_count: u32,
    /// Query depth (max nesting level).
    pub depth: u32,
    /// Execution time in microseconds.
    pub execution_us: u64,
    /// Whether the query resulted in an error.
    pub is_error: bool,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a GraphQL query log record for ALICE-DB.
#[inline]
#[must_use]
pub fn graphql_to_db_query_log(
    query: &[u8],
    field_count: u32,
    depth: u32,
    execution_us: u64,
    is_error: bool,
    timestamp_ms: u64,
) -> GraphqlDbQueryLog {
    let content_hash = fnv1a(query);
    GraphqlDbQueryLog {
        content_hash,
        field_count,
        depth,
        execution_us,
        is_error,
        timestamp_ms,
    }
}

// ── Bridge 2: GraphQL → Cache (response cache) ──────────────────────────

/// Cached GraphQL response for ALICE-Cache.
///
/// Caches resolved query results so that identical queries from different
/// clients can be served without re-executing resolvers.  TTL is reduced
/// for deeply nested queries that are more likely to produce stale data.
pub struct GraphqlCacheResponse {
    /// FNV-1a hash of the query string used as the cache key.
    pub content_hash: u64,
    /// Number of fields selected.
    pub field_count: u32,
    /// Query depth.
    pub depth: u32,
    /// Time-to-live in seconds for this cache entry.
    pub ttl_seconds: u32,
    /// Estimated response payload size in bytes.
    pub payload_bytes: usize,
}

/// Build a GraphQL response cache entry with depth-adjusted TTL.
///
/// TTL derivation (branchless):
/// - depth > 5 → 30 s  (deep queries touch volatile nested data)
/// - else       → 300 s (shallow queries are stable)
#[inline]
#[must_use]
pub fn graphql_to_cache_response(
    query: &[u8],
    field_count: u32,
    depth: u32,
    response_bytes: usize,
) -> GraphqlCacheResponse {
    let content_hash = fnv1a(query);
    let is_deep = (depth > 5) as u32;
    // Branchless TTL: deep=30, shallow=300.
    let ttl_seconds = 300 - is_deep * 270;
    GraphqlCacheResponse {
        content_hash,
        field_count,
        depth,
        ttl_seconds,
        payload_bytes: response_bytes,
    }
}

// ── Bridge 3: GraphQL → Analytics (query metrics) ───────────────────────

/// GraphQL query metrics for ALICE-Analytics.
///
/// Aggregates query patterns per sampling interval so that the analytics
/// layer can chart resolver hotspots, error rates, and latency trends
/// without storing raw query payloads.
pub struct GraphqlAnalyticsMetrics {
    /// FNV-1a hash of the operation name or query fingerprint.
    pub content_hash: u64,
    /// Total queries in the sampling interval.
    pub query_count: u64,
    /// Number of queries that resulted in errors.
    pub error_count: u64,
    /// Error rate in integer percent (0–100).
    pub error_rate_pct: u8,
    /// Average execution time in microseconds.
    pub avg_execution_us: u64,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build GraphQL query metrics for ALICE-Analytics.
///
/// `error_rate_pct` = `error_count / query_count * 100` — integer
/// arithmetic, denominator clamped to 1, result clamped to 100.
#[inline]
#[must_use]
pub fn graphql_to_analytics_metrics(
    operation_name: &[u8],
    query_count: u64,
    error_count: u64,
    avg_execution_us: u64,
    timestamp_ms: u64,
) -> GraphqlAnalyticsMetrics {
    let content_hash = fnv1a(operation_name);
    let total = query_count.max(1);
    let error_rate_pct = ((error_count * 100) / total).min(100) as u8;
    GraphqlAnalyticsMetrics {
        content_hash,
        query_count,
        error_count,
        error_rate_pct,
        avg_execution_us,
        timestamp_ms,
    }
}

// ── Bridge 4: GraphQL → Auth (query authorization) ──────────────────────

/// GraphQL authorization record for ALICE-Auth.
///
/// Encodes the authorization decision for a given query so that the auth
/// layer can enforce field-level access control without re-parsing the
/// query AST.
pub struct GraphqlAuthDecision {
    /// FNV-1a hash of the query + principal combined.
    pub content_hash: u64,
    /// Number of fields requested.
    pub field_count: u32,
    /// Number of fields the principal is authorized to access.
    pub allowed_fields: u32,
    /// Whether the entire query is authorized.
    pub authorized: bool,
    /// Authorization decision code: 0=allow, 1=partial, 2=deny.
    pub decision: u8,
}

/// Build a GraphQL authorization decision for ALICE-Auth.
///
/// Decision derivation (branchless):
/// - allowed == field_count → allow (0)
/// - allowed > 0            → partial (1)
/// - allowed == 0           → deny (2)
#[inline]
#[must_use]
pub fn graphql_to_auth_decision(
    query: &[u8],
    principal: &[u8],
    field_count: u32,
    allowed_fields: u32,
) -> GraphqlAuthDecision {
    let content_hash = fnv1a(query) ^ fnv1a(principal);
    let authorized = allowed_fields >= field_count;
    let is_partial = (allowed_fields > 0 && allowed_fields < field_count) as u8;
    let is_deny = (allowed_fields == 0) as u8;
    // Branchless: deny(2) overrides partial(1) overrides allow(0).
    let decision = (is_deny * 2).max(is_partial);
    GraphqlAuthDecision {
        content_hash,
        field_count,
        allowed_fields,
        authorized,
        decision,
    }
}

// ── Bridge 5: GraphQL → Monitor (performance alert) ─────────────────────

/// GraphQL performance alert for ALICE-Monitor.
///
/// Emitted when query execution exceeds latency thresholds so that
/// operators can investigate slow resolvers and n+1 query patterns.
///
/// `severity`: 0 = normal, 1 = slow, 2 = critical.
pub struct GraphqlMonitorAlert {
    /// FNV-1a hash of the operation name.
    pub content_hash: u64,
    /// Query depth.
    pub depth: u32,
    /// Execution time in microseconds.
    pub execution_us: u64,
    /// Assessed severity (0=normal, 1=slow, 2=critical).
    pub severity: u8,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a GraphQL performance alert for ALICE-Monitor.
///
/// Severity derivation (branchless):
/// - execution_us > 5_000_000 (5s)  → critical (2)
/// - execution_us > 1_000_000 (1s)  → slow (1)
/// - else                           → normal (0)
///
/// Returns `None` for severity == 0 (normal) to avoid alert noise.
#[inline]
#[must_use]
pub fn graphql_to_monitor_alert(
    operation_name: &[u8],
    depth: u32,
    execution_us: u64,
    timestamp_ms: u64,
) -> Option<GraphqlMonitorAlert> {
    let is_critical = (execution_us > 5_000_000) as u8;
    let is_slow = (execution_us > 1_000_000) as u8;
    let severity = (is_critical * 2).max(is_slow);
    if severity == 0 {
        return None;
    }
    let content_hash = fnv1a(operation_name);
    Some(GraphqlMonitorAlert {
        content_hash,
        depth,
        execution_us,
        severity,
        timestamp_ms,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphql_db_query_log_basic() {
        let log =
            graphql_to_db_query_log(b"{ users { name } }", 2, 2, 500, false, 1_700_000_000_000);
        assert_ne!(log.content_hash, 0);
        assert_eq!(log.field_count, 2);
        assert_eq!(log.depth, 2);
        assert!(!log.is_error);
    }

    #[test]
    fn test_graphql_db_query_log_error() {
        let log = graphql_to_db_query_log(b"{ bad }", 0, 1, 100, true, 0);
        assert!(log.is_error);
    }

    #[test]
    fn test_graphql_cache_shallow_ttl() {
        let e = graphql_to_cache_response(b"{ users { id } }", 2, 2, 1024);
        assert_eq!(e.ttl_seconds, 300, "shallow query should have 300s TTL");
        assert_eq!(e.payload_bytes, 1024);
    }

    #[test]
    fn test_graphql_cache_deep_ttl() {
        let e = graphql_to_cache_response(b"{ a { b { c { d { e { f } } } } } }", 6, 6, 2048);
        assert_eq!(e.ttl_seconds, 30, "deep query should have 30s TTL");
    }

    #[test]
    fn test_graphql_analytics_error_rate() {
        let m = graphql_to_analytics_metrics(b"GetUsers", 200, 10, 300, 0);
        assert_eq!(m.error_rate_pct, 5);
        assert_ne!(m.content_hash, 0);
        // zero queries → 0%
        let m2 = graphql_to_analytics_metrics(b"Unused", 0, 0, 0, 0);
        assert_eq!(m2.error_rate_pct, 0);
    }

    #[test]
    fn test_graphql_auth_allow() {
        let d = graphql_to_auth_decision(b"{ users }", b"admin", 5, 5);
        assert!(d.authorized);
        assert_eq!(d.decision, 0);
    }

    #[test]
    fn test_graphql_auth_partial() {
        let d = graphql_to_auth_decision(b"{ users }", b"viewer", 5, 3);
        assert!(!d.authorized);
        assert_eq!(d.decision, 1);
    }

    #[test]
    fn test_graphql_auth_deny() {
        let d = graphql_to_auth_decision(b"{ secrets }", b"anonymous", 5, 0);
        assert!(!d.authorized);
        assert_eq!(d.decision, 2);
    }

    #[test]
    fn test_graphql_monitor_normal_returns_none() {
        let r = graphql_to_monitor_alert(b"GetUsers", 2, 500, 0);
        assert!(r.is_none(), "fast query should not produce alert");
    }

    #[test]
    fn test_graphql_monitor_slow() {
        let alert = graphql_to_monitor_alert(b"SlowQuery", 8, 2_000_000, 0)
            .expect("1s+ query should produce alert");
        assert_eq!(alert.severity, 1);
    }

    #[test]
    fn test_graphql_monitor_critical() {
        let alert = graphql_to_monitor_alert(b"CriticalQuery", 10, 6_000_000, 0)
            .expect("5s+ query should produce critical alert");
        assert_eq!(alert.severity, 2);
        assert_ne!(alert.content_hash, 0);
    }
}
