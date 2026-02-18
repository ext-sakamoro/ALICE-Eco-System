//! API bridges — ALICE-API ↔ Auth, CDN, Queue, Analytics, DB
//!
//! 5 bridges connecting API gateway to the ALICE ecosystem.

use alice_api::{GcraCell, GcraDecision, HttpMethod};
use alice_queue::{Message, SenderKey};
use crate::hash::fnv1a;

// ── Bridge 1: API → Auth (rate limiting → auth check) ───────────────────

/// Rate-limited auth decision for ALICE-Auth verification.
pub struct ApiAuthDecision {
    /// Whether the request is allowed (rate limit passed).
    pub rate_allowed: bool,
    /// Client identifier (from request path/header).
    pub client_id: String,
    /// Whether more requests are remaining.
    pub requests_remaining: bool,
    /// Operation type derived from HTTP method.
    pub operation: &'static str,
}

/// Check API rate limit and prepare auth context for ALICE-Auth.
#[inline]
pub fn api_auth_check(rate_limiter: &GcraCell, client_id: &str, method: HttpMethod, now_ns: u64) -> ApiAuthDecision {
    let decision = rate_limiter.check(now_ns);
    let allowed = matches!(decision, GcraDecision::Allow { .. });
    let operation = match method {
        HttpMethod::Get | HttpMethod::Head => "read",
        HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch => "write",
        HttpMethod::Delete => "delete",
        _ => "other",
    };
    ApiAuthDecision {
        rate_allowed: allowed,
        client_id: client_id.to_string(),
        requests_remaining: allowed,
        operation,
    }
}

// ── Bridge 2: API → CDN (gateway request → CDN routing) ─────────────────

/// CDN routing request from API gateway for ALICE-CDN.
pub struct ApiCdnRoute {
    /// Asset path extracted from request.
    pub asset_path: String,
    /// Content type hint from extension.
    pub content_type_hint: &'static str,
    /// Whether request was rate-limited.
    pub rate_limited: bool,
    /// HTTP method.
    pub method: &'static str,
}

/// Route API gateway request to ALICE-CDN for content delivery.
#[inline]
pub fn api_to_cdn_route(path: &str, method: HttpMethod, rate_allowed: bool) -> ApiCdnRoute {
    let ext = path.rsplit('.').next().unwrap_or("");
    let content_type_hint = match ext {
        "asdf" | "sdf" => "application/x-alice-sdf",
        "json" => "application/json",
        "png" | "jpg" | "jpeg" => "image",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    };
    let method_str = match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Delete => "DELETE",
        _ => "OTHER",
    };
    ApiCdnRoute {
        asset_path: path.to_string(),
        content_type_hint,
        rate_limited: !rate_allowed,
        method: method_str,
    }
}

// ── Bridge 3: API → Queue (rate-limited request queuing) ─────────────────

/// Rate-limited request enqueued for ALICE-Queue processing.
pub struct ApiQueueRequest {
    /// Constructed queue message carrying request payload.
    pub message: Message,
    /// Path hash for routing / deduplication.
    pub path_hash: u64,
    /// Whether this request was admitted (not rate-limited).
    pub admitted: bool,
}

/// Build a queue `Message` from a gateway request for ALICE-Queue.
///
/// The payload encodes `method_byte | path_hash_bytes` (9 bytes total).
/// Branchless method encoding uses a lookup with no branches at runtime.
#[inline]
pub fn api_to_queue_message(
    path: &str,
    method: HttpMethod,
    client_id: &str,
    seq: u64,
    now_ns: u64,
    rate_allowed: bool,
) -> ApiQueueRequest {
    let path_hash = fnv1a(path.as_bytes());
    // Branchless method byte: map variant index to byte constant via small array.
    let method_byte: u8 = match method {
        HttpMethod::Get    => b'G',
        HttpMethod::Post   => b'P',
        HttpMethod::Put    => b'U',
        HttpMethod::Delete => b'D',
        HttpMethod::Patch  => b'A',
        HttpMethod::Head   => b'H',
        _                  => b'?',
    };
    // 9-byte payload: method_byte ++ path_hash (LE u64)
    let mut payload = [0u8; 9];
    payload[0] = method_byte;
    payload[1..9].copy_from_slice(&path_hash.to_le_bytes());

    // Build a 32-byte sender key from client_id hash
    let client_hash = fnv1a(client_id.as_bytes());
    let mut sender: SenderKey = [0u8; 32];
    sender[0..8].copy_from_slice(&client_hash.to_le_bytes());
    sender[8..16].copy_from_slice(&now_ns.to_le_bytes());

    let message = Message::new(sender, seq, payload.to_vec());
    ApiQueueRequest { message, path_hash, admitted: rate_allowed }
}

// ── Bridge 4: API → Analytics (request metrics) ──────────────────────────

/// Request metrics record for ALICE-Analytics ingestion.
pub struct ApiAnalyticsRecord {
    /// HTTP method string ("GET", "POST", …).
    pub method: &'static str,
    /// End-to-end request latency in nanoseconds.
    pub latency_ns: u64,
    /// HTTP status code (200, 404, 429, …).
    pub status_code: u16,
    /// FNV-1a hash of the request path (avoids storing raw strings).
    pub path_hash: u64,
}

/// Produce an analytics record from gateway request metadata.
///
/// `path_hash` is computed via `fnv1a` so the analytics store never
/// receives raw PII-carrying paths.
#[inline]
pub fn api_analytics_record(
    method: HttpMethod,
    latency_ns: u64,
    status_code: u16,
    path: &str,
) -> ApiAnalyticsRecord {
    let method = match method {
        HttpMethod::Get    => "GET",
        HttpMethod::Post   => "POST",
        HttpMethod::Put    => "PUT",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Patch  => "PATCH",
        HttpMethod::Head   => "HEAD",
        _                  => "OTHER",
    };
    ApiAnalyticsRecord {
        method,
        latency_ns,
        status_code,
        path_hash: fnv1a(path.as_bytes()),
    }
}

// ── Bridge 5: API → DB (request log record) ──────────────────────────────

/// Structured request log entry for ALICE-DB persistence.
pub struct ApiDbLogRecord {
    /// HTTP method string.
    pub method: &'static str,
    /// Raw request path (stored in DB verbatim).
    pub path: String,
    /// Request arrival timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// FNV-1a hash of the client identifier for indexed lookup.
    pub client_hash: u64,
}

/// Build a DB log record from gateway request fields.
///
/// `client_hash` is derived via `fnv1a(client_id.as_bytes())` so the
/// DB index column is fixed-width u64 rather than a variable string.
#[inline]
pub fn api_db_log(
    method: HttpMethod,
    path: &str,
    timestamp_ns: u64,
    client_id: &str,
) -> ApiDbLogRecord {
    let method = match method {
        HttpMethod::Get    => "GET",
        HttpMethod::Post   => "POST",
        HttpMethod::Put    => "PUT",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Patch  => "PATCH",
        HttpMethod::Head   => "HEAD",
        _                  => "OTHER",
    };
    ApiDbLogRecord {
        method,
        path: path.to_string(),
        timestamp_ns,
        client_hash: fnv1a(client_id.as_bytes()),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_auth_check_allowed() {
        let limiter = GcraCell::new(10.0, 5); // 10 req/s, burst 5
        let result = api_auth_check(&limiter, "user-123", HttpMethod::Get, 1_000_000_000);
        assert!(result.rate_allowed);
        assert_eq!(result.operation, "read");
        assert_eq!(result.client_id, "user-123");
    }

    #[test]
    fn test_api_auth_check_write() {
        let limiter = GcraCell::new(10.0, 5);
        let result = api_auth_check(&limiter, "user-456", HttpMethod::Post, 1_000_000_000);
        assert!(result.rate_allowed);
        assert_eq!(result.operation, "write");
    }

    #[test]
    fn test_api_to_cdn_route_sdf() {
        let route = api_to_cdn_route("/assets/model.asdf", HttpMethod::Get, true);
        assert_eq!(route.content_type_hint, "application/x-alice-sdf");
        assert!(!route.rate_limited);
        assert_eq!(route.method, "GET");
    }

    #[test]
    fn test_api_to_cdn_route_rate_limited() {
        let route = api_to_cdn_route("/data/file.json", HttpMethod::Get, false);
        assert!(route.rate_limited);
        assert_eq!(route.content_type_hint, "application/json");
    }

    #[test]
    fn test_api_to_queue_message() {
        let req = api_to_queue_message("/api/infer", HttpMethod::Post, "client-7", 1, 5_000_000_000, true);
        assert!(req.admitted);
        assert_ne!(req.path_hash, 0);
        // Payload byte 0 is the method byte for POST
        assert_eq!(req.message.payload[0], b'P');
        // Path hash embedded in payload bytes 1..9 must match standalone fnv1a
        use crate::hash::fnv1a;
        let expected = fnv1a(b"/api/infer");
        let embedded = u64::from_le_bytes(req.message.payload[1..9].try_into().unwrap());
        assert_eq!(embedded, expected);
    }

    #[test]
    fn test_api_analytics_record() {
        use crate::hash::fnv1a;
        let rec = api_analytics_record(HttpMethod::Get, 250_000, 200, "/health");
        assert_eq!(rec.method, "GET");
        assert_eq!(rec.latency_ns, 250_000);
        assert_eq!(rec.status_code, 200);
        assert_eq!(rec.path_hash, fnv1a(b"/health"));
    }

    #[test]
    fn test_api_db_log() {
        use crate::hash::fnv1a;
        let record = api_db_log(HttpMethod::Delete, "/users/42", 9_000_000_000, "admin");
        assert_eq!(record.method, "DELETE");
        assert_eq!(record.path, "/users/42");
        assert_eq!(record.timestamp_ns, 9_000_000_000);
        assert_eq!(record.client_hash, fnv1a(b"admin"));
    }
}
