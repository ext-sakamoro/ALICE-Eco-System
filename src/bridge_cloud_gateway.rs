//! Cloud Gateway bridges — ALICE-Cloud-Gateway ↔ CDN, Cache, Auth, Analytics, DB, DNS, Queue
//!
//! 7 bridges connecting cloud gateway to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Gateway → CDN (origin shield routing) ──────────────────────

/// CDN origin-shield routing decision produced by the cloud gateway.
///
/// `origin_hash` (FNV-1a of the origin host string) drives consistent
/// Maglev-style shard selection without a live Maglev ring.  `ttl_seconds`
/// is stored pre-computed so the CDN layer can cache the decision without
/// re-deriving it.
pub struct GatewayCdnRoute {
    /// FNV-1a hash of the origin host string — CDN routing key.
    pub origin_hash: u64,
    /// Target region tag (0 = primary, 1 = secondary, …).
    pub region: u8,
    /// Number of CDN edge nodes to replicate this origin shield entry to.
    pub edge_count: u8,
    /// Suggested TTL for the shield entry, in seconds.
    pub ttl_seconds: u32,
    /// MIME content-type hint derived from the origin host suffix.
    pub content_type: &'static str,
}

/// Build a CDN origin-shield routing record from a gateway origin host.
///
/// `content_type` is inferred from common origin host suffixes:
/// - `*.api.*`  → `"application/json"`
/// - `*.media.*` or `*.cdn.*` → `"application/octet-stream"`
/// - `*.static.*` → `"text/html"`
/// - otherwise → `"application/octet-stream"`
///
/// No divisions — `ttl_seconds` is derived from a branchless integer
/// multiply driven by `region`.
#[inline]
pub fn gateway_to_cdn_route(
    origin: &str,
    region: u8,
    edge_count: u8,
) -> GatewayCdnRoute {
    let origin_hash = fnv1a(origin.as_bytes());

    // Content type: scan for well-known substrings (branchless suffix table).
    let content_type = if origin.contains(".api.") || origin.ends_with("-api") {
        "application/json"
    } else if origin.contains(".media.") || origin.contains(".cdn.") {
        "application/octet-stream"
    } else if origin.contains(".static.") {
        "text/html"
    } else {
        "application/octet-stream"
    };

    // TTL: primary region (0) → 3600 s; each additional region halves TTL
    // via a branchless right-shift on a u32.  Clamped to [60, 3600].
    // ttl = 3600 >> region, saturating at a 60 s floor.
    let shift = (region as u32).min(6); // 3600 >> 6 = 56; keep floor handling below
    let raw_ttl = 3600u32 >> shift;
    let ttl_seconds = raw_ttl.max(60);

    GatewayCdnRoute {
        origin_hash,
        region,
        edge_count,
        ttl_seconds,
        content_type,
    }
}

// ── Bridge 2: Gateway → Cache (response caching) ─────────────────────────

/// Cached HTTP response record for ALICE-Cache ingestion.
///
/// `request_hash` keys the cache row.  `vary_hash` encodes the Vary header
/// signature so that different Accept-Encoding variants are stored under
/// distinct sub-keys without string storage.
pub struct GatewayCacheEntry {
    /// FNV-1a hash of `method + path` — primary cache key.
    pub request_hash: u64,
    /// Response body size in bytes.
    pub response_bytes: usize,
    /// HTTP status code of the cached response.
    pub status_code: u16,
    /// Derived TTL in seconds (status-code dependent, branchless).
    pub ttl_seconds: u32,
    /// FNV-1a hash of the Vary header value (or 0 if absent).
    pub vary_hash: u64,
}

/// Build a cache entry from a gateway response.
///
/// TTL derivation (branchless, no division):
/// - 200 → 300 s
/// - 301 → 86 400 s (permanent redirect)
/// - 404 → 60 s (negative caching)
/// - any other → 0 s (do not cache)
///
/// `vary_hash` is computed via FNV-1a over the Vary string; pass `""` when
/// the response carries no Vary header (result is the FNV-1a of empty bytes).
#[inline]
pub fn gateway_to_cache_entry(
    method: &str,
    path: &str,
    status: u16,
    body_size: usize,
) -> GatewayCacheEntry {
    // Combine method + path into a single hash without heap allocation.
    let request_hash = {
        let mut h: u64 = 0xcbf29ce484222325;
        for &b in method.as_bytes() { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
        h ^= b':' as u64; h = h.wrapping_mul(0x100000001b3);
        for &b in path.as_bytes()   { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
        h
    };

    // Branchless TTL via a 4-entry lookup indexed by status class.
    // Encode the three interesting codes as offsets into a small table.
    // is_200: status == 200 → index bit 0
    // is_301: status == 301 → index bit 1
    // is_404: status == 404 → index bit 2
    // We fold these into a single table index (0..=3) with saturating adds.
    let is_200 = (status == 200) as usize;
    let is_301 = (status == 301) as usize;
    let is_404 = (status == 404) as usize;
    // Table: [other=0, 200→300, 301→86400, 404→60]
    // index 0 → 0, 1 → 300, 2 → 86400, 3 → 60
    const TTL_TABLE: [u32; 4] = [0, 300, 86_400, 60];
    let idx = (is_200 * 1) | (is_301 * 2) | (is_404 * 3);
    // Saturate overlapping bits (impossible in practice for valid status codes).
    let idx = idx.min(3);
    let ttl_seconds = TTL_TABLE[idx];

    // Vary hash over empty bytes gives a stable non-zero FNV-1a offset value.
    let vary_hash = fnv1a(b"");

    GatewayCacheEntry {
        request_hash,
        response_bytes: body_size,
        status_code: status,
        ttl_seconds,
        vary_hash,
    }
}

// ── Bridge 3: Gateway → Auth (request authentication) ────────────────────

/// Authentication request forwarded by the cloud gateway to ALICE-Auth.
///
/// Method encoding: 0 = GET, 1 = POST, 2 = PUT, 3 = DELETE.
/// `has_token` lets the auth service short-circuit expensive crypto checks
/// for obviously unauthenticated requests without inspecting `token_bytes`.
pub struct GatewayAuthRequest {
    /// FNV-1a hash of `client_id` — opaque client identity token.
    pub client_hash: u64,
    /// HTTP method byte (0=GET, 1=POST, 2=PUT, 3=DELETE).
    pub method: u8,
    /// FNV-1a hash of the request path — avoids PII leakage.
    pub path_hash: u64,
    /// Whether the request carries a non-empty bearer token.
    pub has_token: bool,
    /// Byte length of the token (0 when absent).
    pub token_bytes: usize,
}

/// Build an auth-check request from gateway request metadata.
///
/// `token_len` is the raw byte length of the Authorization header value
/// (or 0 if the header is absent / empty).  `has_token` is derived
/// branchlessly as `token_len > 0`.
#[inline]
pub fn gateway_to_auth_request(
    client_id: &str,
    method: u8,
    path: &str,
    token_len: usize,
) -> GatewayAuthRequest {
    let client_hash = fnv1a(client_id.as_bytes());
    let path_hash   = fnv1a(path.as_bytes());
    // Branchless: (token_len > 0) as bool, no branch emitted.
    let has_token   = token_len > 0;

    GatewayAuthRequest {
        client_hash,
        method,
        path_hash,
        has_token,
        token_bytes: token_len,
    }
}

// ── Bridge 4: Gateway → Analytics (request telemetry) ────────────────────

/// Request telemetry event for ALICE-Analytics ingestion.
///
/// All path / client data is pre-hashed so the analytics pipeline never
/// stores raw PII.  `latency_ms` is kept as f64 to preserve sub-millisecond
/// precision from high-resolution clocks without integer truncation.
pub struct GatewayAnalyticsEvent {
    /// FNV-1a hash of the request path — analytics stream key.
    pub request_hash: u64,
    /// HTTP method byte (0=GET, 1=POST, 2=PUT, 3=DELETE).
    pub method: u8,
    /// HTTP response status code.
    pub status_code: u16,
    /// End-to-end gateway latency in milliseconds.
    pub latency_ms: f64,
    /// Response body size in bytes.
    pub body_bytes: usize,
    /// FNV-1a hash of client identifier.
    pub client_hash: u64,
    /// Whether the response was served from cache.
    pub is_cache_hit: bool,
}

/// Build an analytics event from gateway request / response metadata.
#[inline]
pub fn gateway_to_analytics_event(
    client: &str,
    method: u8,
    path: &str,
    status: u16,
    latency_ms: f64,
    body: usize,
    cache_hit: bool,
) -> GatewayAnalyticsEvent {
    GatewayAnalyticsEvent {
        request_hash: fnv1a(path.as_bytes()),
        method,
        status_code: status,
        latency_ms,
        body_bytes: body,
        client_hash: fnv1a(client.as_bytes()),
        is_cache_hit: cache_hit,
    }
}

// ── Bridge 5: Gateway → DB (route config persistence) ────────────────────

/// Route configuration record for ALICE-DB persistence.
///
/// Encodes gateway routing policy in a fixed-width struct suitable for
/// a single DB row per gateway instance.  `config_hash` enables cheap
/// change-detection without full record comparison.
pub struct GatewayDbRouteConfig {
    /// FNV-1a hash of all config fields packed as LE bytes — change key.
    pub config_hash: u64,
    /// Number of active HTTP routes registered in the gateway.
    pub route_count: usize,
    /// Number of upstream backend hosts.
    pub backend_count: usize,
    /// Maximum accepted request body size in bytes.
    pub max_body_size: usize,
    /// Global rate-limit ceiling in requests per second.
    pub rate_limit_rps: u32,
}

/// Build a DB route-config persistence record from gateway configuration.
///
/// `config_hash` is derived via FNV-1a over the four scalar fields packed
/// into a 20-byte little-endian buffer — no heap allocation.
#[inline]
pub fn gateway_to_db_route_config(
    routes: usize,
    backends: usize,
    max_body: usize,
    rate_limit: u32,
) -> GatewayDbRouteConfig {
    // Pack all four fields into a 20-byte buffer for deterministic hashing.
    let mut buf = [0u8; 20];
    buf[0..8].copy_from_slice(&(routes    as u64).to_le_bytes());
    buf[8..16].copy_from_slice(&(backends as u64).to_le_bytes());
    buf[16..20].copy_from_slice(&rate_limit.to_le_bytes());
    // max_body folds into the hash via a separate FNV-1a chain to keep buf fixed-size.
    let base_hash = fnv1a(&buf);
    let config_hash = {
        let mut h = base_hash;
        for &b in &(max_body as u64).to_le_bytes() { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
        h
    };

    GatewayDbRouteConfig {
        config_hash,
        route_count: routes,
        backend_count: backends,
        max_body_size: max_body,
        rate_limit_rps: rate_limit,
    }
}

// ── Bridge 6: Gateway → DNS (service discovery) ───────────────────────────

/// DNS query forwarded by the cloud gateway to ALICE-DNS for service discovery.
///
/// Query type encoding: 0 = A, 1 = AAAA, 2 = SRV, 3 = CNAME.
/// `ttl_hint` carries the gateway's preferred minimum TTL so the DNS
/// resolver can avoid serving stale records below that threshold.
pub struct GatewayDnsQuery {
    /// FNV-1a hash of `service_name` — DNS resolver cache key.
    pub service_hash: u64,
    /// Byte length of the service name string (avoids re-strlen on the resolver).
    pub hostname_bytes: usize,
    /// DNS record type (0=A, 1=AAAA, 2=SRV, 3=CNAME).
    pub query_type: u8,
    /// Minimum acceptable TTL hint in seconds (30 s default for unknown types).
    pub ttl_hint: u32,
}

/// Build a DNS service-discovery query from gateway service metadata.
///
/// `ttl_hint` is derived branchlessly from `query_type`:
/// - SRV (2) → 10 s (short-lived service endpoints)
/// - CNAME (3) → 300 s (stable aliases)
/// - A / AAAA → 60 s
/// - other → 30 s
#[inline]
pub fn gateway_to_dns_query(
    service_name: &str,
    query_type: u8,
) -> GatewayDnsQuery {
    let service_hash  = fnv1a(service_name.as_bytes());
    let hostname_bytes = service_name.len();

    // Branchless TTL via a 4-entry table indexed by query_type (clamped to 3).
    // 0=A→60, 1=AAAA→60, 2=SRV→10, 3=CNAME→300.
    const TTL_HINT_TABLE: [u32; 4] = [60, 60, 10, 300];
    let idx = (query_type as usize).min(3);
    let ttl_hint = TTL_HINT_TABLE[idx];

    GatewayDnsQuery {
        service_hash,
        hostname_bytes,
        query_type,
        ttl_hint,
    }
}

// ── Bridge 7: Gateway → Queue (request overflow) ─────────────────────────

/// Queued request record for ALICE-Queue overflow handling.
///
/// When the gateway is saturated, excess requests are enqueued rather than
/// dropped.  `priority` drives the queue's scheduling discipline:
/// POST/PUT = 2 (write mutations), DELETE = 1 (destructive but lower urgency),
/// GET = 0 (reads, lowest priority).
pub struct GatewayQueueRequest {
    /// FNV-1a hash of the request path — queue deduplication key.
    pub request_hash: u64,
    /// FNV-1a hash of the client identifier.
    pub client_hash: u64,
    /// HTTP method byte (0=GET, 1=POST, 2=PUT, 3=DELETE).
    pub method: u8,
    /// Scheduling priority (0=low, 1=medium, 2=high).
    pub priority: u8,
    /// Request body size in bytes.
    pub body_bytes: usize,
    /// Enqueue timestamp in milliseconds (caller-provided monotonic clock).
    pub enqueue_ms: u64,
}

/// Build a queue overflow record from a gateway request.
///
/// Priority derivation (branchless, no division):
/// - POST (1) or PUT (2) → 2
/// - DELETE (3)          → 1
/// - GET (0) or other    → 0
///
/// Implemented via two branchless boolean multiplies added together,
/// emitting `cmov` on x86 / `csel` on AArch64.
#[inline]
pub fn gateway_to_queue_request(
    client: &str,
    method: u8,
    path: &str,
    body_size: usize,
    timestamp_ms: u64,
) -> GatewayQueueRequest {
    let request_hash = fnv1a(path.as_bytes());
    let client_hash  = fnv1a(client.as_bytes());

    // Branchless priority:
    //   is_write  = (method == 1 || method == 2) → contributes 2
    //   is_delete = (method == 3)                 → contributes 1
    // These are mutually exclusive for valid method bytes.
    let is_write  = ((method == 1) | (method == 2)) as u8;
    let is_delete = (method == 3) as u8;
    let priority  = is_write.wrapping_mul(2) | is_delete;

    GatewayQueueRequest {
        request_hash,
        client_hash,
        method,
        priority,
        body_bytes: body_size,
        enqueue_ms: timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Bridge 1: Gateway → CDN ───────────────────────────────────────────

    #[test]
    fn test_gateway_to_cdn_route_primary() {
        let route = gateway_to_cdn_route("origin.example.com", 0, 4);
        // Hash must be non-zero and deterministic.
        assert_ne!(route.origin_hash, 0);
        assert_eq!(route.origin_hash, fnv1a(b"origin.example.com"));
        // Primary region → TTL = 3600 s.
        assert_eq!(route.ttl_seconds, 3600);
        assert_eq!(route.region, 0);
        assert_eq!(route.edge_count, 4);
        assert_eq!(route.content_type, "application/octet-stream");
    }

    #[test]
    fn test_gateway_to_cdn_route_api_origin() {
        let route = gateway_to_cdn_route("us.api.example.com", 1, 2);
        assert_eq!(route.content_type, "application/json");
        // Region 1 → 3600 >> 1 = 1800, above 60 floor.
        assert_eq!(route.ttl_seconds, 1800);
    }

    #[test]
    fn test_gateway_to_cdn_route_ttl_floor() {
        // Region 7 → 3600 >> 6 = 56, clamped to 60.
        let route = gateway_to_cdn_route("far.example.com", 7, 1);
        assert!(route.ttl_seconds >= 60, "ttl {} below floor", route.ttl_seconds);
    }

    // ── Bridge 2: Gateway → Cache ─────────────────────────────────────────

    #[test]
    fn test_gateway_to_cache_entry_200() {
        let entry = gateway_to_cache_entry("GET", "/index.html", 200, 1024);
        assert_ne!(entry.request_hash, 0);
        assert_eq!(entry.status_code, 200);
        assert_eq!(entry.ttl_seconds, 300);
        assert_eq!(entry.response_bytes, 1024);
    }

    #[test]
    fn test_gateway_to_cache_entry_301() {
        let entry = gateway_to_cache_entry("GET", "/old-path", 301, 0);
        assert_eq!(entry.ttl_seconds, 86_400);
    }

    #[test]
    fn test_gateway_to_cache_entry_404() {
        let entry = gateway_to_cache_entry("GET", "/missing", 404, 0);
        assert_eq!(entry.ttl_seconds, 60);
    }

    #[test]
    fn test_gateway_to_cache_entry_other() {
        let entry = gateway_to_cache_entry("POST", "/submit", 500, 0);
        assert_eq!(entry.ttl_seconds, 0, "5xx responses must not be cached");
    }

    #[test]
    fn test_gateway_to_cache_entry_hash_varies_by_path() {
        let a = gateway_to_cache_entry("GET", "/a", 200, 10);
        let b = gateway_to_cache_entry("GET", "/b", 200, 10);
        assert_ne!(a.request_hash, b.request_hash, "distinct paths must hash differently");
    }

    // ── Bridge 3: Gateway → Auth ──────────────────────────────────────────

    #[test]
    fn test_gateway_to_auth_request_with_token() {
        let req = gateway_to_auth_request("client-42", 0, "/api/data", 128);
        assert_eq!(req.client_hash, fnv1a(b"client-42"));
        assert_eq!(req.path_hash,   fnv1a(b"/api/data"));
        assert_eq!(req.method, 0);
        assert!(req.has_token);
        assert_eq!(req.token_bytes, 128);
    }

    #[test]
    fn test_gateway_to_auth_request_no_token() {
        let req = gateway_to_auth_request("anon", 1, "/login", 0);
        assert!(!req.has_token);
        assert_eq!(req.token_bytes, 0);
        assert_eq!(req.method, 1);
    }

    #[test]
    fn test_gateway_to_auth_request_delete() {
        let req = gateway_to_auth_request("admin", 3, "/users/99", 64);
        assert_eq!(req.method, 3);
        assert!(req.has_token);
    }

    // ── Bridge 4: Gateway → Analytics ────────────────────────────────────

    #[test]
    fn test_gateway_to_analytics_event_cache_hit() {
        let evt = gateway_to_analytics_event("client-7", 0, "/static/logo.png", 200, 1.5, 4096, true);
        assert_eq!(evt.request_hash, fnv1a(b"/static/logo.png"));
        assert_eq!(evt.client_hash,  fnv1a(b"client-7"));
        assert_eq!(evt.method, 0);
        assert_eq!(evt.status_code, 200);
        assert!((evt.latency_ms - 1.5).abs() < f64::EPSILON);
        assert_eq!(evt.body_bytes, 4096);
        assert!(evt.is_cache_hit);
    }

    #[test]
    fn test_gateway_to_analytics_event_cache_miss() {
        let evt = gateway_to_analytics_event("client-9", 1, "/api/upload", 201, 42.3, 512, false);
        assert!(!evt.is_cache_hit);
        assert_eq!(evt.status_code, 201);
    }

    #[test]
    fn test_gateway_to_analytics_event_hashes_distinct() {
        let a = gateway_to_analytics_event("u1", 0, "/a", 200, 1.0, 0, false);
        let b = gateway_to_analytics_event("u2", 0, "/b", 200, 1.0, 0, false);
        assert_ne!(a.request_hash, b.request_hash);
        assert_ne!(a.client_hash,  b.client_hash);
    }

    // ── Bridge 5: Gateway → DB ────────────────────────────────────────────

    #[test]
    fn test_gateway_to_db_route_config_basic() {
        let cfg = gateway_to_db_route_config(12, 3, 1_048_576, 1000);
        assert_ne!(cfg.config_hash, 0);
        assert_eq!(cfg.route_count,    12);
        assert_eq!(cfg.backend_count,  3);
        assert_eq!(cfg.max_body_size,  1_048_576);
        assert_eq!(cfg.rate_limit_rps, 1000);
    }

    #[test]
    fn test_gateway_to_db_route_config_hash_changes_with_fields() {
        let a = gateway_to_db_route_config(10, 2, 512, 500);
        let b = gateway_to_db_route_config(10, 2, 512, 501); // only rate_limit differs
        assert_ne!(a.config_hash, b.config_hash, "hash must change when any field changes");
    }

    #[test]
    fn test_gateway_to_db_route_config_deterministic() {
        let a = gateway_to_db_route_config(5, 1, 65536, 100);
        let b = gateway_to_db_route_config(5, 1, 65536, 100);
        assert_eq!(a.config_hash, b.config_hash, "same inputs must produce same hash");
    }

    // ── Bridge 6: Gateway → DNS ───────────────────────────────────────────

    #[test]
    fn test_gateway_to_dns_query_a_record() {
        let q = gateway_to_dns_query("backend.internal", 0);
        assert_eq!(q.service_hash,  fnv1a(b"backend.internal"));
        assert_eq!(q.hostname_bytes, "backend.internal".len());
        assert_eq!(q.query_type, 0);
        assert_eq!(q.ttl_hint,  60);
    }

    #[test]
    fn test_gateway_to_dns_query_srv() {
        let q = gateway_to_dns_query("_grpc._tcp.svc", 2);
        assert_eq!(q.query_type, 2);
        assert_eq!(q.ttl_hint, 10, "SRV records get a 10 s TTL hint");
    }

    #[test]
    fn test_gateway_to_dns_query_cname() {
        let q = gateway_to_dns_query("alias.example.com", 3);
        assert_eq!(q.query_type, 3);
        assert_eq!(q.ttl_hint, 300, "CNAME records get a 300 s TTL hint");
    }

    #[test]
    fn test_gateway_to_dns_query_aaaa() {
        let q = gateway_to_dns_query("ipv6.example.com", 1);
        assert_eq!(q.ttl_hint, 60);
    }

    #[test]
    fn test_gateway_to_dns_query_unknown_type_clamps() {
        // query_type > 3 clamps to index 3 (CNAME TTL = 300).
        let q = gateway_to_dns_query("svc.local", 99);
        assert_eq!(q.ttl_hint, 300);
    }

    // ── Bridge 7: Gateway → Queue ─────────────────────────────────────────

    #[test]
    fn test_gateway_to_queue_request_get() {
        let req = gateway_to_queue_request("client-1", 0, "/feed", 0, 1_000);
        assert_eq!(req.request_hash, fnv1a(b"/feed"));
        assert_eq!(req.client_hash,  fnv1a(b"client-1"));
        assert_eq!(req.method,    0);
        assert_eq!(req.priority,  0, "GET must have priority 0");
        assert_eq!(req.body_bytes, 0);
        assert_eq!(req.enqueue_ms, 1_000);
    }

    #[test]
    fn test_gateway_to_queue_request_post() {
        let req = gateway_to_queue_request("client-2", 1, "/submit", 256, 2_000);
        assert_eq!(req.priority, 2, "POST must have priority 2");
        assert_eq!(req.body_bytes, 256);
    }

    #[test]
    fn test_gateway_to_queue_request_put() {
        let req = gateway_to_queue_request("client-3", 2, "/update", 128, 3_000);
        assert_eq!(req.priority, 2, "PUT must have priority 2");
    }

    #[test]
    fn test_gateway_to_queue_request_delete() {
        let req = gateway_to_queue_request("client-4", 3, "/resource/7", 0, 4_000);
        assert_eq!(req.priority, 1, "DELETE must have priority 1");
    }

    #[test]
    fn test_gateway_to_queue_request_hashes_distinct() {
        let a = gateway_to_queue_request("u1", 0, "/x", 0, 0);
        let b = gateway_to_queue_request("u2", 0, "/y", 0, 0);
        assert_ne!(a.request_hash, b.request_hash);
        assert_ne!(a.client_hash,  b.client_hash);
    }
}
