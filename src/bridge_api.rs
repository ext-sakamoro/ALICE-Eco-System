//! API bridges — ALICE-API ↔ Auth, CDN
//!
//! 2 bridges connecting API gateway to the ALICE ecosystem.

use alice_api::{GcraCell, GcraDecision, HttpMethod};

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
}
