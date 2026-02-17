//! DNS bridges — ALICE-DNS ↔ Browser, Cache
//!
//! 2 bridges connecting Bloom filter DNS ad-blocker to the ALICE ecosystem.

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
}
