//! DNS bridges — ALICE-DNS ↔ Browser, Cache, DB
//!
//! 3 bridges connecting Bloom filter DNS ad-blocker to the ALICE ecosystem.

use alice_dns::{DnsBloomEngine, DnsAction};

// ── Bridge 1: DNS → Browser (Bloom filter → domain classification) ──────

/// Domain classification result for ALICE-Browser ad blocking.
pub struct DnsBrowserClassification {
    /// Domain name checked.
    pub domain: String,
    /// Whether the domain is blocked (ad/tracker).
    pub blocked: bool,
    /// Action taken.
    pub action: &'static str,
}

/// Classify domain for ALICE-Browser ad blocking via Bloom filter.
#[inline]
pub fn dns_browser_classify(engine: &mut DnsBloomEngine, domain: &str) -> DnsBrowserClassification {
    let action = engine.check_domain(domain);
    let (blocked, action_str) = match action {
        DnsAction::Block => (true, "blocked"),
        DnsAction::Allow => (false, "allow"),
        DnsAction::Spoof => (true, "spoof"),
    };
    DnsBrowserClassification {
        domain: domain.to_string(),
        blocked,
        action: action_str,
    }
}

/// Batch classify domains for ALICE-Browser.
#[inline]
pub fn dns_browser_classify_batch(engine: &mut DnsBloomEngine, domains: &[&str]) -> Vec<DnsBrowserClassification> {
    domains.iter().map(|d| dns_browser_classify(engine, d)).collect()
}

// ── Bridge 2: DNS → Cache (DnsAction → cache prefetch hint) ─────────────

/// Cache prefetch hint from DNS resolution for ALICE-Cache.
pub struct DnsCacheHint {
    /// Domain that was resolved.
    pub domain: String,
    /// Whether to cache (not blocked).
    pub should_cache: bool,
    /// Priority (higher = more important to cache).
    pub priority: u8,
    /// TTL hint in seconds.
    pub ttl_secs: u32,
}

/// Generate cache prefetch hint from DNS action for ALICE-Cache.
#[inline]
pub fn dns_to_cache_hint(domain: &str, action: DnsAction) -> DnsCacheHint {
    let (should_cache, priority) = match action {
        DnsAction::Block => (false, 0),
        DnsAction::Allow => (true, 100),
        DnsAction::Spoof => (false, 0),
    };
    DnsCacheHint {
        domain: domain.to_string(),
        should_cache,
        priority,
        ttl_secs: 300,
    }
}

// ── Bridge 3: DNS → DB (DNS record persistence) ────────────────────────

/// DNS record for ALICE-DB persistence.
pub struct DnsDbRecord {
    /// Domain name.
    pub domain: String,
    /// Action classification.
    pub action: &'static str,
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Whether domain is blocked.
    pub blocked: bool,
}

/// Serialize DNS classification for ALICE-DB persistence.
#[inline]
pub fn dns_to_db_record(engine: &mut DnsBloomEngine, domain: &str) -> DnsDbRecord {
    let action = engine.check_domain(domain);
    let (blocked, action_str) = match action {
        DnsAction::Block => (true, "blocked"),
        DnsAction::Allow => (false, "allow"),
        DnsAction::Spoof => (true, "spoof"),
    };
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in domain.as_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    DnsDbRecord {
        domain: domain.to_string(),
        action: action_str,
        content_hash: hash,
        blocked,
    }
}

// ── Bridge 4: DNS → API (DNS resolution → API route target) ─────────────

/// API route target derived from a DNS resolution result for ALICE-API.
pub struct DnsApiRoute {
    /// FNV-1a hash of the resolved domain bytes (stable cache / routing key).
    pub content_hash: u64,
    /// Hash of the resolved IP representation derived from the domain bytes.
    pub resolved_ip_hash: u64,
    /// Destination port (443 for allowed domains, 0 for blocked).
    pub port: u16,
    /// Whether the result was served from the DNS Bloom filter cache.
    pub is_cached: bool,
    /// Time-to-live for the route entry in seconds.
    pub ttl_secs: u32,
}

/// Build an API route target from a DNS resolution via the Bloom filter engine.
///
/// Blocked or spoofed domains produce a zeroed port so the API gateway can
/// reject the route without a separate check.  `is_cached` is always `true`
/// because every result passes through the in-process Bloom filter (no network
/// round-trip).
#[inline]
pub fn dns_to_api_route(engine: &mut DnsBloomEngine, domain: &str) -> DnsApiRoute {
    let action = engine.check_domain(domain);
    // FNV-1a hash of the domain bytes as the content / cache key.
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in domain.as_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    // resolved_ip_hash: mix the domain hash one more round to simulate an IP derivation.
    let mut ip_hash = hash;
    ip_hash ^= 0xdeadbeef_cafebabe_u64;
    ip_hash = ip_hash.wrapping_mul(0x100000001b3);
    // Branchless port: 443 for Allow, 0 for Block/Spoof.
    let port: u16 = match action {
        DnsAction::Allow => 443,
        DnsAction::Block | DnsAction::Spoof => 0,
    };
    DnsApiRoute {
        content_hash: hash,
        resolved_ip_hash: ip_hash,
        port,
        is_cached: true,
        ttl_secs: 300,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_browser_classify() {
        let blocklist = vec!["ads.example.com".to_string(), "tracker.bad.com".to_string()];
        let mut engine = DnsBloomEngine::new();
        engine.load_domains(&blocklist);
        let result = dns_browser_classify(&mut engine, "ads.example.com");
        assert!(result.blocked);
        assert_eq!(result.action, "blocked");
        let result2 = dns_browser_classify(&mut engine, "safe.example.com");
        assert!(!result2.blocked);
    }

    #[test]
    fn test_dns_browser_classify_batch() {
        let blocklist = vec!["ads.example.com".to_string()];
        let mut engine = DnsBloomEngine::new();
        engine.load_domains(&blocklist);
        let results = dns_browser_classify_batch(&mut engine, &["ads.example.com", "clean.org"]);
        assert_eq!(results.len(), 2);
        assert!(results[0].blocked);
        assert!(!results[1].blocked);
    }

    #[test]
    fn test_dns_to_cache_hint() {
        let hint = dns_to_cache_hint("example.com", DnsAction::Allow);
        assert!(hint.should_cache);
        assert_eq!(hint.priority, 100);

        let hint2 = dns_to_cache_hint("ads.com", DnsAction::Block);
        assert!(!hint2.should_cache);
        assert_eq!(hint2.priority, 0);
    }

    #[test]
    fn test_dns_to_db_record() {
        let blocklist = vec!["ads.bad.com".to_string()];
        let mut engine = DnsBloomEngine::new();
        engine.load_domains(&blocklist);
        let rec = dns_to_db_record(&mut engine, "ads.bad.com");
        assert!(rec.blocked);
        assert_eq!(rec.action, "blocked");
        assert_ne!(rec.content_hash, 0);

        let rec2 = dns_to_db_record(&mut engine, "clean.com");
        assert!(!rec2.blocked);
        assert_eq!(rec2.action, "allow");
    }

    #[test]
    fn test_dns_to_api_route_allowed() {
        let mut engine = DnsBloomEngine::new();
        engine.load_domains(&[]);
        let route = dns_to_api_route(&mut engine, "api.example.com");
        assert_eq!(route.port, 443);
        assert!(route.is_cached);
        assert_eq!(route.ttl_secs, 300);
        assert_ne!(route.content_hash, 0);
        assert_ne!(route.resolved_ip_hash, 0);
    }

    #[test]
    fn test_dns_to_api_route_blocked() {
        let blocklist = vec!["malicious.ads.com".to_string()];
        let mut engine = DnsBloomEngine::new();
        engine.load_domains(&blocklist);
        let route = dns_to_api_route(&mut engine, "malicious.ads.com");
        assert_eq!(route.port, 0);
        assert!(route.is_cached);
    }

    #[test]
    fn test_dns_to_api_route_deterministic() {
        let mut engine = DnsBloomEngine::new();
        engine.load_domains(&[]);
        let a = dns_to_api_route(&mut engine, "stable.example.com");
        let b = dns_to_api_route(&mut engine, "stable.example.com");
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.resolved_ip_hash, b.resolved_ip_hash);
    }
}
