//! Test bridges — ALICE-Test ↔ DB, Analytics, Cache, ML, Edge
//!
//! 5 bridges connecting property testing, benchmarking, and regression
//! detection results to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Test → DB (test result storage) ─────────────────────────────

/// Test result storage record for ALICE-DB.
///
/// Persists property-test outcomes and bench statistics so that regression
/// history can be queried across builds and branches.
pub struct TestDbRecord {
    /// FNV-1a hash over suite name, test name, and run timestamp.
    pub content_hash: u64,
    /// Suite name hash.
    pub suite_hash: u64,
    /// Test name hash.
    pub test_hash: u64,
    /// Number of property-test iterations that passed.
    pub passed: u64,
    /// Number of property-test iterations that failed.
    pub failed: u64,
    /// Total iterations attempted.
    pub total: u64,
    /// True when all iterations passed (failed == 0).
    pub all_passed: bool,
    /// Run timestamp in milliseconds.
    pub run_at_ms: u64,
}

/// Serialize a property-test run result for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn test_to_db_record(
    suite_name: &str,
    test_name: &str,
    passed: u64,
    failed: u64,
    total: u64,
    run_at_ms: u64,
) -> TestDbRecord {
    let suite_hash = fnv1a(suite_name.as_bytes());
    let test_hash = fnv1a(test_name.as_bytes());
    let mut data = [0u8; 24];
    data[0..8].copy_from_slice(&suite_hash.to_le_bytes());
    data[8..16].copy_from_slice(&test_hash.to_le_bytes());
    data[16..24].copy_from_slice(&run_at_ms.to_le_bytes());
    TestDbRecord {
        content_hash: fnv1a(&data),
        suite_hash,
        test_hash,
        passed,
        failed,
        total,
        all_passed: failed == 0,
        run_at_ms,
    }
}

// ── Bridge 2: Test → Analytics (test metrics) ────────────────────────────

/// Test metrics payload for ALICE-Analytics.
///
/// Feeds bench statistics and pass rates into the analytics pipeline so
/// that CI trend dashboards and flakiness detectors have fresh data.
pub struct TestAnalyticsMetrics {
    /// FNV-1a hash over suite name, test name, and run timestamp.
    pub content_hash: u64,
    /// Suite name hash for analytics stream routing.
    pub suite_hash: u64,
    /// Pass rate as a fraction (passed / total); 1.0 when total is zero.
    pub pass_rate: f64,
    /// Benchmark mean latency in microseconds (0.0 if not a bench test).
    pub bench_mean_us: f64,
    /// Benchmark P99 latency in microseconds (0.0 if not a bench test).
    pub bench_p99_us: f64,
    /// True when a regression was detected against the baseline.
    pub regression_detected: bool,
    /// Run timestamp in milliseconds.
    pub run_at_ms: u64,
}

/// Build a test metrics payload for ALICE-Analytics.
///
/// `pass_rate` uses a reciprocal multiply to avoid division.
#[inline]
#[must_use]
pub fn test_to_analytics_metrics(
    suite_name: &str,
    test_name: &str,
    passed: u64,
    total: u64,
    bench_mean_us: f64,
    bench_p99_us: f64,
    regression_detected: bool,
    run_at_ms: u64,
) -> TestAnalyticsMetrics {
    let suite_hash = fnv1a(suite_name.as_bytes());
    let test_hash = fnv1a(test_name.as_bytes());
    let rcp_total = 1.0 / total.max(1) as f64;
    let pass_rate = passed as f64 * rcp_total;
    let mut data = [0u8; 24];
    data[0..8].copy_from_slice(&suite_hash.to_le_bytes());
    data[8..16].copy_from_slice(&test_hash.to_le_bytes());
    data[16..24].copy_from_slice(&run_at_ms.to_le_bytes());
    TestAnalyticsMetrics {
        content_hash: fnv1a(&data),
        suite_hash,
        pass_rate,
        bench_mean_us,
        bench_p99_us,
        regression_detected,
        run_at_ms,
    }
}

// ── Bridge 3: Test → Cache (test result cache) ────────────────────────────

/// Test result cache entry for ALICE-Cache.
///
/// Caches the latest result for a (suite, test) pair so that CI status
/// pages avoid re-querying DB on every request.  TTL is computed
/// branchlessly: passing results are cached longer than failing ones.
pub struct TestCacheEntry {
    /// FNV-1a hash over suite name and test name — cache key.
    pub content_hash: u64,
    /// True when the cached run passed (failed == 0).
    pub all_passed: bool,
    /// Cache TTL in seconds (branchless: longer for passing results).
    pub ttl_secs: u32,
    /// Number of iterations run in the cached result.
    pub total: u64,
    /// Entry size in bytes (estimated).
    pub entry_bytes: usize,
}

/// Build a test result cache entry for ALICE-Cache.
///
/// Passing results get a 600 s TTL; failing results get 60 s so that
/// a fix is reflected quickly.  The TTL selection is branchless.
#[inline]
#[must_use]
pub fn test_to_cache_entry(
    suite_name: &str,
    test_name: &str,
    all_passed: bool,
    total: u64,
) -> TestCacheEntry {
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&fnv1a(suite_name.as_bytes()).to_le_bytes());
    data[8..16].copy_from_slice(&fnv1a(test_name.as_bytes()).to_le_bytes());
    let content_hash = fnv1a(&data);
    // Branchless TTL: pass → 600 s, fail → 60 s.
    let passed = all_passed as u32;
    let ttl_secs = 60u32 + passed * 540u32;
    TestCacheEntry {
        content_hash,
        all_passed,
        ttl_secs,
        total,
        entry_bytes: 40,
    }
}

// ── Bridge 4: Test → ML (test data for property testing) ─────────────────

/// Property-test data record for ALICE-ML model validation.
///
/// Feeds generated test inputs and their pass/fail outcomes to the ML layer
/// so that property-based coverage can guide model boundary detection.
pub struct TestMlRecord {
    /// FNV-1a hash over suite name, test name, and seed.
    pub content_hash: u64,
    /// Suite name hash.
    pub suite_hash: u64,
    /// Property-test RNG seed used for this run.
    pub seed: u64,
    /// Total iterations in the run.
    pub total: u64,
    /// Number of unique failure-inducing inputs found.
    pub unique_failures: u64,
    /// Shrink steps performed to minimise failing inputs (0 when all passed).
    pub shrink_steps: u64,
    /// Pass rate fraction.
    pub pass_rate: f64,
}

/// Build a property-test data record for ALICE-ML.
///
/// `pass_rate` uses a reciprocal multiply to avoid division.
#[inline]
#[must_use]
pub fn test_to_ml_record(
    suite_name: &str,
    test_name: &str,
    seed: u64,
    passed: u64,
    total: u64,
    unique_failures: u64,
    shrink_steps: u64,
) -> TestMlRecord {
    let suite_hash = fnv1a(suite_name.as_bytes());
    let test_hash = fnv1a(test_name.as_bytes());
    let mut data = [0u8; 24];
    data[0..8].copy_from_slice(&suite_hash.to_le_bytes());
    data[8..16].copy_from_slice(&test_hash.to_le_bytes());
    data[16..24].copy_from_slice(&seed.to_le_bytes());
    let rcp_total = 1.0 / total.max(1) as f64;
    let pass_rate = passed as f64 * rcp_total;
    TestMlRecord {
        content_hash: fnv1a(&data),
        suite_hash,
        seed,
        total,
        unique_failures,
        shrink_steps,
        pass_rate,
    }
}

// ── Bridge 5: Test → Edge (test events) ──────────────────────────────────

/// Compact test event payload for ALICE-Edge.
///
/// Edge nodes emit lightweight test completion events so that the central
/// CI platform can track distributed test runs without full result payloads.
pub struct TestEdgeEvent {
    /// FNV-1a hash over suite name, test name, and timestamp.
    pub content_hash: u64,
    /// Suite name hash for edge-side routing.
    pub suite_hash: u64,
    /// Test name hash.
    pub test_hash: u64,
    /// True when the test run passed.
    pub all_passed: bool,
    /// Total iterations run.
    pub total: u64,
    /// Event timestamp in milliseconds.
    pub event_at_ms: u64,
    /// Estimated wire size in bytes.
    pub wire_bytes: usize,
}

/// Build a compact test event payload for ALICE-Edge.
#[inline]
#[must_use]
pub fn test_to_edge_event(
    suite_name: &str,
    test_name: &str,
    all_passed: bool,
    total: u64,
    event_at_ms: u64,
) -> TestEdgeEvent {
    let suite_hash = fnv1a(suite_name.as_bytes());
    let test_hash = fnv1a(test_name.as_bytes());
    let mut data = [0u8; 24];
    data[0..8].copy_from_slice(&suite_hash.to_le_bytes());
    data[8..16].copy_from_slice(&test_hash.to_le_bytes());
    data[16..24].copy_from_slice(&event_at_ms.to_le_bytes());
    TestEdgeEvent {
        content_hash: fnv1a(&data),
        suite_hash,
        test_hash,
        all_passed,
        total,
        event_at_ms,
        // 8 suite + 8 test + 1 passed + 8 total + 8 timestamp = 33 bytes.
        wire_bytes: 33,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_to_db_record_content_hash_nonzero() {
        let rec = test_to_db_record("alice_sdf", "roundtrip", 1_000, 0, 1_000, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.suite_hash, 0);
        assert_ne!(rec.test_hash, 0);
    }

    #[test]
    fn test_test_to_db_record_all_passed_flag() {
        let pass = test_to_db_record("suite", "name", 500, 0, 500, 0);
        assert!(pass.all_passed);
        let fail = test_to_db_record("suite", "name", 490, 10, 500, 0);
        assert!(!fail.all_passed);
    }

    #[test]
    fn test_test_to_db_record_hash_determinism() {
        let a = test_to_db_record("s", "t", 100, 0, 100, 42_000);
        let b = test_to_db_record("s", "t", 100, 0, 100, 42_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_test_to_analytics_metrics_pass_rate() {
        // 800 passed out of 1000 → 0.8.
        let m = test_to_analytics_metrics("alice_zip", "compress", 800, 1_000, 12.5, 98.0, false, 0);
        assert_ne!(m.content_hash, 0);
        assert!((m.pass_rate - 0.8).abs() < 1e-9, "pass_rate={}", m.pass_rate);
        assert!((m.bench_mean_us - 12.5).abs() < 1e-9);
        assert!(!m.regression_detected);
    }

    #[test]
    fn test_test_to_analytics_metrics_zero_total_no_panic() {
        let m = test_to_analytics_metrics("s", "t", 0, 0, 0.0, 0.0, false, 0);
        assert!((m.pass_rate - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_test_to_cache_entry_passing_ttl() {
        let entry = test_to_cache_entry("alice_auth", "token_roundtrip", true, 1_000);
        assert_ne!(entry.content_hash, 0);
        // pass → ttl = 60 + 540 = 600
        assert_eq!(entry.ttl_secs, 600);
        assert!(entry.all_passed);
    }

    #[test]
    fn test_test_to_cache_entry_failing_ttl() {
        let entry = test_to_cache_entry("alice_ml", "gradient_check", false, 500);
        // fail → ttl = 60
        assert_eq!(entry.ttl_secs, 60);
        assert!(!entry.all_passed);
    }

    #[test]
    fn test_test_to_ml_record_pass_rate_and_seed() {
        let rec = test_to_ml_record("alice_physics", "collision", 0xDEAD_BEEF, 950, 1_000, 2, 15);
        assert_ne!(rec.content_hash, 0);
        assert!((rec.pass_rate - 0.95).abs() < 1e-9, "pass_rate={}", rec.pass_rate);
        assert_eq!(rec.seed, 0xDEAD_BEEF);
        assert_eq!(rec.unique_failures, 2);
        assert_eq!(rec.shrink_steps, 15);
    }

    #[test]
    fn test_test_to_edge_event_wire_bytes() {
        let ev = test_to_edge_event("alice_codec", "encode_decode", true, 256, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert!(ev.all_passed);
        assert_eq!(ev.total, 256);
        assert_eq!(ev.wire_bytes, 33);
        assert_eq!(ev.event_at_ms, 1_700_000_000_000);
    }
}
