//! HTTP bridges — ALICE-HTTP ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting HTTP request/response data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: HTTP → DB (request log) ────────────────────────────────────

/// Request log record for ALICE-DB persistence.
///
/// Written for every HTTP request so that audit consumers can replay
/// traffic patterns and detect anomalies offline.
pub struct HttpDbRequestLog {
    /// FNV-1a hash of the method + URI combined.
    pub content_hash: u64,
    /// FNV-1a hash of the HTTP method string (e.g. b"GET").
    pub method_hash: u64,
    /// FNV-1a hash of the request URI.
    pub uri_hash: u64,
    /// HTTP response status code (e.g. 200, 404, 500).
    pub status_code: u16,
    /// Response body length in bytes.
    pub content_length: u64,
    /// Number of HTTP headers present in the request.
    pub header_count: u16,
    /// Unix timestamp in milliseconds when the request was received.
    pub timestamp_ms: u64,
    /// Request processing duration in microseconds.
    pub duration_us: u64,
}

/// Build an HTTP request log record for ALICE-DB.
///
/// `content_hash` is derived by XOR-chaining `method_hash` and `uri_hash`
/// so that neither a method change nor a URI change alone produces the same
/// composite hash — branchless, single XOR, no allocation.
#[inline]
#[must_use]
pub fn http_to_db_request_log(
    method: &[u8],
    uri: &[u8],
    status_code: u16,
    content_length: u64,
    header_count: u16,
    timestamp_ms: u64,
    duration_us: u64,
) -> HttpDbRequestLog {
    let method_hash = fnv1a(method);
    let uri_hash = fnv1a(uri);
    // Branchless composite: XOR preserves avalanche from both components.
    let content_hash = method_hash ^ uri_hash;
    HttpDbRequestLog {
        content_hash,
        method_hash,
        uri_hash,
        status_code,
        content_length,
        header_count,
        timestamp_ms,
        duration_us,
    }
}

// ── Bridge 2: HTTP → Cache (response cache) ───────────────────────────────

/// Response cache entry for ALICE-Cache.
///
/// TTL is derived from the status code: successful responses (2xx) receive
/// the full base TTL; redirects (3xx) receive half; errors receive none.
pub struct HttpCacheEntry {
    /// FNV-1a hash of the URI used as the primary cache key.
    pub content_hash: u64,
    /// FNV-1a hash of the HTTP method (only GET/HEAD are typically cached).
    pub method_hash: u64,
    /// HTTP response status code.
    pub status_code: u16,
    /// Cached response body size in bytes.
    pub body_bytes: u64,
    /// Time-to-live in seconds for this cache entry.
    pub ttl_seconds: u32,
    /// Number of cache hits served from this entry since insertion.
    pub hit_count: u32,
    /// Whether the response carries a Vary header (0 = no, 1 = yes).
    pub has_vary: u8,
}

/// Build an HTTP response cache entry with status-derived TTL.
///
/// TTL derivation (branchless arithmetic):
/// - 2xx: base_ttl = 300 s
/// - 3xx: base_ttl / 2 = 150 s
/// - 4xx/5xx: 0 s (not cached)
///
/// `status_class` = `status_code / 100` via integer division (compiler emits
/// a multiply-by-reciprocal — no hardware division instruction).
#[inline]
#[must_use]
pub fn http_to_cache_entry(
    uri: &[u8],
    method: &[u8],
    status_code: u16,
    body_bytes: u64,
    has_vary: u8,
) -> HttpCacheEntry {
    let content_hash = fnv1a(uri);
    let method_hash = fnv1a(method);

    // status_class: 2 → 2xx, 3 → 3xx, else → no cache.
    let status_class = status_code / 100;
    // Branchless TTL: success=300, redirect=150, error=0.
    let is_2xx = (status_class == 2) as u32;
    let is_3xx = (status_class == 3) as u32;
    let ttl_seconds = is_2xx * 300 + is_3xx * 150;

    HttpCacheEntry {
        content_hash,
        method_hash,
        status_code,
        body_bytes,
        ttl_seconds,
        hit_count: 0,
        has_vary,
    }
}

// ── Bridge 3: HTTP → Analytics (traffic metrics) ─────────────────────────

/// Traffic metrics event for ALICE-Analytics ingestion.
///
/// Aggregates per-request counters so that the analytics layer can build
/// histograms and rate charts without storing raw request bodies.
pub struct HttpAnalyticsEvent {
    /// FNV-1a hash of the URI identifying this request family.
    pub content_hash: u64,
    /// FNV-1a hash of the HTTP method.
    pub method_hash: u64,
    /// HTTP response status code.
    pub status_code: u16,
    /// Response body length in bytes.
    pub content_length: u64,
    /// Request processing duration in microseconds.
    pub duration_us: u64,
    /// Unix timestamp in milliseconds when the request completed.
    pub timestamp_ms: u64,
    /// Estimated transfer size including headers, in bytes.
    pub transfer_bytes: u64,
    /// Route segment count (number of `/`-separated path components).
    pub path_depth: u8,
}

/// Build an HTTP traffic metrics event for ALICE-Analytics.
///
/// `transfer_bytes` is estimated as `content_length + header_count * 32`
/// (header_count * 32 approximates average header overhead) — integer
/// multiply, no division, no branches.
#[inline]
#[must_use]
pub fn http_to_analytics_event(
    uri: &[u8],
    method: &[u8],
    status_code: u16,
    content_length: u64,
    duration_us: u64,
    timestamp_ms: u64,
    header_count: u16,
    path_depth: u8,
) -> HttpAnalyticsEvent {
    let content_hash = fnv1a(uri);
    let method_hash = fnv1a(method);
    // Estimated transfer overhead: header_count * 32 bytes average per header.
    let transfer_bytes = content_length + (header_count as u64) * 32;
    HttpAnalyticsEvent {
        content_hash,
        method_hash,
        status_code,
        content_length,
        duration_us,
        timestamp_ms,
        transfer_bytes,
        path_depth,
    }
}

// ── Bridge 4: HTTP → Monitor (health) ────────────────────────────────────

/// Health probe record for ALICE-Monitor.
///
/// Encodes whether the endpoint is healthy based on status code and latency
/// so that the monitor layer can update its service health map without
/// parsing raw HTTP responses.
///
/// `health_level`: 0 = healthy, 1 = degraded, 2 = unhealthy.
pub struct HttpMonitorHealth {
    /// FNV-1a hash of the URI used to identify the monitored endpoint.
    pub content_hash: u64,
    /// HTTP response status code observed during the probe.
    pub status_code: u16,
    /// Request latency in microseconds.
    pub latency_us: u64,
    /// Health level derived from status and latency (0=healthy, 1=degraded, 2=unhealthy).
    pub health_level: u8,
    /// Unix timestamp in milliseconds when the probe completed.
    pub timestamp_ms: u64,
    /// Consecutive failure count at the time of this probe.
    pub consecutive_failures: u32,
}

/// Build an HTTP health probe record for ALICE-Monitor.
///
/// Health level derivation (branchless):
/// - status 2xx AND latency < 1_000_000 µs → healthy (0)
/// - status 2xx AND latency >= 1_000_000 µs → degraded (1)
/// - status != 2xx → unhealthy (2)
///
/// Uses integer comparisons and addition instead of match branches.
#[inline]
#[must_use]
pub fn http_to_monitor_health(
    uri: &[u8],
    status_code: u16,
    latency_us: u64,
    timestamp_ms: u64,
    consecutive_failures: u32,
) -> HttpMonitorHealth {
    let content_hash = fnv1a(uri);
    let is_2xx = (status_code / 100 == 2) as u8;
    let is_slow = (latency_us >= 1_000_000) as u8;
    // health_level: 0 if 2xx+fast, 1 if 2xx+slow, 2 if not 2xx.
    // Branchless: (1 - is_2xx)*2 + is_2xx*is_slow
    let health_level = (1 - is_2xx) * 2 + is_2xx * is_slow;
    HttpMonitorHealth {
        content_hash,
        status_code,
        latency_us,
        health_level,
        timestamp_ms,
        consecutive_failures,
    }
}

// ── Bridge 5: HTTP → API (gateway) ───────────────────────────────────────

/// Gateway routing record for ALICE-API.
///
/// Provides the API gateway layer with the pre-computed URI hash and routing
/// metadata so that upstream dispatch avoids redundant string hashing.
pub struct HttpApiGatewayRecord {
    /// FNV-1a hash of the URI used for upstream routing decisions.
    pub content_hash: u64,
    /// FNV-1a hash of the HTTP method.
    pub method_hash: u64,
    /// HTTP response status code returned by the upstream.
    pub status_code: u16,
    /// Number of HTTP headers forwarded to the upstream.
    pub header_count: u16,
    /// Upstream response latency in microseconds.
    pub upstream_latency_us: u64,
    /// Estimated payload size forwarded to the upstream in bytes.
    pub payload_bytes: u64,
    /// Whether the request required authentication (0 = no, 1 = yes).
    pub authenticated: u8,
    /// Route priority for load-balancing (0 = lowest, 255 = highest).
    pub route_priority: u8,
}

/// Build an HTTP gateway routing record for ALICE-API.
///
/// `route_priority` is derived from `authenticated` and `status_code`:
/// authenticated 2xx requests receive priority 200, unauthenticated 2xx
/// receive 128, all others receive 0.  Computed with branchless multiply.
#[inline]
#[must_use]
pub fn http_to_api_gateway_record(
    uri: &[u8],
    method: &[u8],
    status_code: u16,
    header_count: u16,
    upstream_latency_us: u64,
    payload_bytes: u64,
    authenticated: u8,
) -> HttpApiGatewayRecord {
    let content_hash = fnv1a(uri);
    let method_hash = fnv1a(method);
    let is_2xx = (status_code / 100 == 2) as u8;
    let auth_flag = authenticated.min(1);
    // Branchless priority: auth+2xx=200, unauth+2xx=128, other=0.
    let route_priority = is_2xx * (auth_flag * 72 + 128);
    HttpApiGatewayRecord {
        content_hash,
        method_hash,
        status_code,
        header_count,
        upstream_latency_us,
        payload_bytes,
        authenticated: auth_flag,
        route_priority,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_to_db_request_log_basic() {
        let log = http_to_db_request_log(
            b"GET",
            b"/api/v1/users",
            200,
            1024,
            12,
            1_700_000_000_000,
            850,
        );
        assert_ne!(log.content_hash, 0);
        assert_eq!(log.method_hash, fnv1a(b"GET"));
        assert_eq!(log.uri_hash, fnv1a(b"/api/v1/users"));
        assert_eq!(log.content_hash, fnv1a(b"GET") ^ fnv1a(b"/api/v1/users"));
        assert_eq!(log.status_code, 200);
        assert_eq!(log.content_length, 1024);
        assert_eq!(log.header_count, 12);
        assert_eq!(log.duration_us, 850);
    }

    #[test]
    fn test_http_to_db_request_log_method_uri_distinct_hashes() {
        // content_hash = method_hash ^ uri_hash; XOR is commutative so swapping
        // method and URI yields the same content_hash, but method_hash and
        // uri_hash individually differ — verify individual hashes are distinct.
        let a = http_to_db_request_log(b"POST", b"/x", 201, 0, 0, 0, 0);
        let b = http_to_db_request_log(b"/x", b"POST", 201, 0, 0, 0, 0);
        assert_eq!(a.content_hash, b.content_hash);
        assert_ne!(a.method_hash, b.method_hash);
        assert_ne!(a.uri_hash, b.uri_hash);
    }

    #[test]
    fn test_http_to_cache_entry_2xx_gets_full_ttl() {
        let e = http_to_cache_entry(b"/static/logo.png", b"GET", 200, 4096, 0);
        assert_eq!(e.ttl_seconds, 300);
        assert_eq!(e.hit_count, 0);
        assert_ne!(e.content_hash, 0);
    }

    #[test]
    fn test_http_to_cache_entry_3xx_gets_half_ttl() {
        let e = http_to_cache_entry(b"/old-path", b"GET", 301, 0, 0);
        assert_eq!(e.ttl_seconds, 150);
    }

    #[test]
    fn test_http_to_cache_entry_error_gets_zero_ttl() {
        let e = http_to_cache_entry(b"/missing", b"GET", 404, 0, 0);
        assert_eq!(e.ttl_seconds, 0);
        let e5 = http_to_cache_entry(b"/error", b"GET", 500, 0, 0);
        assert_eq!(e5.ttl_seconds, 0);
    }

    #[test]
    fn test_http_to_analytics_event_transfer_bytes() {
        // transfer_bytes = content_length + header_count * 32
        let ev = http_to_analytics_event(b"/data", b"GET", 200, 1000, 500, 0, 10, 3);
        assert_eq!(ev.transfer_bytes, 1000 + 10 * 32);
        assert_eq!(ev.path_depth, 3);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_http_to_monitor_health_levels() {
        // Healthy: 2xx + fast
        let h = http_to_monitor_health(b"/health", 200, 100_000, 0, 0);
        assert_eq!(h.health_level, 0);
        // Degraded: 2xx + slow (>= 1_000_000 µs)
        let d = http_to_monitor_health(b"/health", 200, 2_000_000, 0, 0);
        assert_eq!(d.health_level, 1);
        // Unhealthy: non-2xx
        let u = http_to_monitor_health(b"/health", 503, 50_000, 0, 5);
        assert_eq!(u.health_level, 2);
        assert_eq!(u.consecutive_failures, 5);
    }

    #[test]
    fn test_http_to_api_gateway_record_priority() {
        // Authenticated 2xx → priority 200
        let r = http_to_api_gateway_record(b"/api", b"POST", 201, 8, 5000, 256, 1);
        assert_eq!(r.route_priority, 200);
        // Unauthenticated 2xx → priority 128
        let r2 = http_to_api_gateway_record(b"/api", b"GET", 200, 4, 1000, 0, 0);
        assert_eq!(r2.route_priority, 128);
        // Non-2xx → priority 0
        let r3 = http_to_api_gateway_record(b"/api", b"GET", 401, 4, 200, 0, 0);
        assert_eq!(r3.route_priority, 0);
    }

    #[test]
    fn test_fnv1a_deterministic_and_distinct() {
        let h1 = fnv1a(b"alice-http");
        let h2 = fnv1a(b"alice-http");
        assert_eq!(h1, h2);
        assert_ne!(h1, 0);
        assert_ne!(fnv1a(b"alice-http"), fnv1a(b"alice-cache"));
    }
}
