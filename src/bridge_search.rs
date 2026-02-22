//! Search bridges — ALICE-Search ↔ DB, Browser, VCS, Cache, Analytics
//!
//! 5 bridges connecting FM-Index full-text search to the ALICE ecosystem.

use alice_search::AliceIndex;

// ── Bridge 1: Search → DB (FM-Index → DB query integration) ─────────────

/// Search result from FM-Index for ALICE-DB storage/query.
pub struct SearchDbResult {
    /// Number of occurrences found.
    pub occurrence_count: usize,
    /// Pattern matched.
    pub pattern_len: usize,
    /// Index size in bytes (approximate).
    pub index_size_bytes: usize,
    /// Whether the pattern exists.
    pub found: bool,
}

/// Search text corpus indexed by FM-Index for ALICE-DB query results.
#[inline]
#[must_use]
pub fn search_db_query(index: &AliceIndex, pattern: &[u8]) -> SearchDbResult {
    let count = index.count(pattern);
    SearchDbResult {
        occurrence_count: count,
        pattern_len: pattern.len(),
        index_size_bytes: 0, // opaque, estimated externally
        found: count > 0,
    }
}

// ── Bridge 2: Search → Browser (AliceIndex → in-page search) ────────────

/// In-page search result for ALICE-Browser.
pub struct SearchBrowserResult {
    /// Total matches found.
    pub match_count: usize,
    /// First N match positions (for highlighting).
    pub positions: Vec<usize>,
    /// Pattern that was searched.
    pub pattern_len: usize,
    /// Whether the pattern exists in the page.
    pub found: bool,
}

/// Perform in-page text search for ALICE-Browser using FM-Index.
#[inline]
#[must_use]
pub fn search_browser_page(
    index: &AliceIndex,
    pattern: &[u8],
    max_positions: usize,
) -> SearchBrowserResult {
    let found = index.contains(pattern);
    let count = if found { index.count(pattern) } else { 0 };
    let positions: Vec<usize> = if found {
        index.locate(pattern).take(max_positions).collect()
    } else {
        vec![]
    };
    SearchBrowserResult {
        match_count: count,
        positions,
        pattern_len: pattern.len(),
        found,
    }
}

// ── Bridge 3: Search → VCS (AliceIndex → commit message search) ─────────

/// VCS commit search result for ALICE-VCS.
pub struct SearchVcsResult {
    /// Number of matching commits.
    pub match_count: usize,
    /// Match positions in concatenated commit log.
    pub positions: Vec<usize>,
    /// Pattern searched.
    pub pattern_len: usize,
}

/// Search VCS commit messages via FM-Index.
#[inline]
#[must_use]
pub fn search_vcs_commits(commit_log: &str, pattern: &str) -> SearchVcsResult {
    let bytes = commit_log.as_bytes();
    let index = AliceIndex::build(bytes, 4);
    let pat = pattern.as_bytes();
    let count = index.count(pat);
    let positions: Vec<usize> = index.locate(pat).take(100).collect();
    SearchVcsResult {
        match_count: count,
        positions,
        pattern_len: pattern.len(),
    }
}

// ── Bridge 4: Search → Cache (search result caching) ─────────────────────

/// Cached search result for ALICE-Cache.
pub struct SearchCacheEntry {
    /// Content hash for cache key (pattern-based).
    pub content_hash: u64,
    /// Match count.
    pub match_count: usize,
    /// Pattern length.
    pub pattern_len: usize,
    /// Result found.
    pub found: bool,
}

/// Cache FM-Index search result for ALICE-Cache.
#[inline]
#[must_use]
pub fn search_to_cache_entry(index: &AliceIndex, pattern: &[u8]) -> SearchCacheEntry {
    let count = index.count(pattern);
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in pattern {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    SearchCacheEntry {
        content_hash: hash,
        match_count: count,
        pattern_len: pattern.len(),
        found: count > 0,
    }
}

// ── Bridge 5: Search → Analytics (search metrics) ───────────────────────

/// Search metrics for ALICE-Analytics monitoring.
pub struct SearchAnalyticsMetrics {
    /// Pattern length.
    pub pattern_len: usize,
    /// Match count.
    pub match_count: usize,
    /// Search hit ratio (found = 1.0, not found = 0.0).
    pub hit_ratio: f32,
    /// Content hash of pattern.
    pub pattern_hash: u64,
}

/// Extract search metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn search_to_analytics_metrics(index: &AliceIndex, pattern: &[u8]) -> SearchAnalyticsMetrics {
    let count = index.count(pattern);
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in pattern {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    SearchAnalyticsMetrics {
        pattern_len: pattern.len(),
        match_count: count,
        hit_ratio: if count > 0 { 1.0 } else { 0.0 },
        pattern_hash: hash,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_db_query() {
        let text = b"the quick brown fox jumps over the lazy dog";
        let index = AliceIndex::build(text, 4);
        let result = search_db_query(&index, b"the");
        assert!(result.found);
        assert!(result.occurrence_count >= 2);
    }

    #[test]
    fn test_search_browser_page() {
        let text = b"hello world hello ALICE hello";
        let index = AliceIndex::build(text, 1);
        let result = search_browser_page(&index, b"hello", 10);
        assert!(result.found);
        assert_eq!(result.match_count, 3);
        assert_eq!(result.positions.len(), 3);
    }

    #[test]
    fn test_search_vcs_commits() {
        let commits = "fix: resolve bug in parser\nfeat: add new bridge\nfix: memory leak in cache";
        let result = search_vcs_commits(commits, "fix");
        assert_eq!(result.match_count, 2);
        assert_eq!(result.positions.len(), 2);
    }

    #[test]
    fn test_search_to_cache_entry() {
        let text = b"hello world hello ALICE";
        let index = AliceIndex::build(text, 1);
        let entry = search_to_cache_entry(&index, b"hello");
        assert!(entry.found);
        assert_eq!(entry.match_count, 2);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_search_to_analytics_metrics() {
        let text = b"the quick brown fox";
        let index = AliceIndex::build(text, 4);
        let m = search_to_analytics_metrics(&index, b"quick");
        assert_eq!(m.match_count, 1);
        assert!((m.hit_ratio - 1.0).abs() < 0.01);
        assert_ne!(m.pattern_hash, 0);
    }
}
