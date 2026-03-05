//! Browser bridges — ALICE-Browser ↔ DB, Cache, CDN, Analytics, Font, Search, ML, DNS
//!
//! 8 bridges connecting the ALICE-Browser DOM classifier to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Browser → DB (page classification persistence) ─────────────

/// Page classification persistence record for ALICE-DB.
///
/// Stores per-URL classification results from the browser DOM pipeline.
/// All fields are plain integers/floats — no heap allocation.
pub struct BrowserDbPageRecord {
    /// FNV-1a hash of the full URL bytes — primary DB key.
    pub content_hash: u64,
    /// FNV-1a hash of the URL (alias field for DB join keys).
    pub url_hash: u64,
    /// Total DOM node count before filtering.
    pub node_count: usize,
    /// Nodes classified as main content.
    pub content_nodes: usize,
    /// Nodes classified as advertisements.
    pub ad_nodes: usize,
    /// Nodes classified as trackers.
    pub tracker_nodes: usize,
    /// Wall-clock time to run the DOM classifier, in milliseconds.
    pub classification_ms: f64,
}

/// Serialize a browser classification result for ALICE-DB persistence.
///
/// # Optimization notes
/// - `removed_nodes` distributes evenly between `ad_nodes` and `tracker_nodes`
///   via integer halve (`>> 1`) and a branchless remainder correction.
/// - `content_nodes = total - removed` uses wrapping subtraction; no branch.
/// - Both `content_hash` and `url_hash` are the same FNV-1a digest of the URL
///   bytes, computed once and stored in both fields for schema flexibility.
#[inline]
#[must_use]
pub fn browser_to_db_page_record(
    url: &str,
    total_nodes: usize,
    removed_nodes: usize,
    classification_ms: f64,
) -> BrowserDbPageRecord {
    let url_hash = fnv1a(url.as_bytes());

    // Distribute removed nodes between ads and trackers (50/50 split).
    // Half via right-shift; remainder (removed & 1) added to ad_nodes branchlessly.
    let half = removed_nodes >> 1;
    let remainder = removed_nodes & 1; // 0 or 1 — branchless
    let ad_nodes = half + remainder;
    let tracker_nodes = half;

    // Content nodes: saturating subtract to avoid underflow.
    let content_nodes = total_nodes.saturating_sub(removed_nodes);

    BrowserDbPageRecord {
        content_hash: url_hash,
        url_hash,
        node_count: total_nodes,
        content_nodes,
        ad_nodes,
        tracker_nodes,
        classification_ms,
    }
}

// ── Bridge 2: Browser → Cache (DOM tree cache entry) ─────────────────────

/// DOM tree cache entry for ALICE-Cache.
///
/// Encodes the minimum metadata needed to store and evict a rendered DOM
/// snapshot.  `ttl_seconds` is inversely proportional to page size so that
/// large pages are evicted sooner, keeping the cache hot for small pages.
pub struct BrowserCacheEntry {
    /// FNV-1a hash of the URL bytes — cache lookup key.
    pub url_hash: u64,
    /// FNV-1a hash of `node_count` and `byte_estimate` — content fingerprint.
    pub content_hash: u64,
    /// Total DOM node count.
    pub node_count: usize,
    /// Estimated serialised byte size of the DOM tree.
    pub byte_estimate: usize,
    /// Cache time-to-live in seconds (larger pages get shorter TTL).
    pub ttl_seconds: u32,
}

/// Produce a cache entry from URL and DOM statistics.
///
/// # Optimization notes
/// - `byte_estimate` is passed directly; no copy of DOM bytes occurs.
/// - TTL formula: `3600 * 4096 / max(byte_estimate, 1)` — one integer
///   divide on the cold path only, result clamped to [30, 3600].
/// - Content hash covers `node_count` and `byte_estimate` packed into 16
///   bytes — no heap allocation.
#[inline]
#[must_use]
pub fn browser_to_cache_entry(
    url: &str,
    node_count: usize,
    content_bytes: usize,
) -> BrowserCacheEntry {
    const BASE_PRODUCT: u64 = 3_600 * 4_096;
    let url_hash = fnv1a(url.as_bytes());

    // Content hash over node_count + byte_estimate.
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&node_count.to_le_bytes());
    key[8..16].copy_from_slice(&content_bytes.to_le_bytes());
    let content_hash = fnv1a(&key);

    // TTL: inversely proportional to content size; clamped to [30, 3600] s.
    // Reciprocal form: base_product / max(bytes, 1).
    // base_product = 3600 * 4096 = 14_745_600 — fits comfortably in u64.
    let bytes_safe = (content_bytes as u64).max(1);
    let raw_ttl = (BASE_PRODUCT / bytes_safe).clamp(30, 3_600) as u32;

    BrowserCacheEntry {
        url_hash,
        content_hash,
        node_count,
        byte_estimate: content_bytes,
        ttl_seconds: raw_ttl,
    }
}

// ── Bridge 3: Browser → CDN (pre-rendered page delivery) ─────────────────

/// Pre-rendered page package for ALICE-CDN delivery.
///
/// The browser pre-renders a filtered DOM and hands it off to the CDN for
/// edge caching and distribution.
pub struct BrowserCdnPage {
    /// FNV-1a hash of the URL bytes — CDN content identifier.
    pub content_hash: u64,
    /// MIME type of the delivered content.
    pub content_type: &'static str,
    /// Byte size of the pre-rendered page payload.
    pub byte_size: usize,
    /// Render mode flag: 0 = full render, 1 = partial, 2 = skeleton.
    pub render_mode: u8,
}

/// Package a pre-rendered browser page for ALICE-CDN distribution.
///
/// # Optimization notes
/// - `content_type` is a `'static str` — no heap allocation.
/// - `render_mode` is stored as-is; callers are responsible for clamping to
///   [0, 2] if needed (bridge does not validate to avoid a branch).
#[inline]
#[must_use]
pub fn browser_to_cdn_page(url: &str, content_bytes: usize, render_mode: u8) -> BrowserCdnPage {
    let content_hash = fnv1a(url.as_bytes());

    BrowserCdnPage {
        content_hash,
        content_type: "text/html; charset=utf-8",
        byte_size: content_bytes,
        render_mode,
    }
}

// ── Bridge 4: Browser → Analytics (browsing telemetry) ───────────────────

/// Browsing telemetry event for ALICE-Analytics ingestion.
///
/// Captures per-page performance and ad-blocking metrics for the analytics
/// time-series pipeline.
pub struct BrowserAnalyticsEvent {
    /// FNV-1a hash of the URL bytes — analytics stream key.
    pub url_hash: u64,
    /// Full page load time, in milliseconds.
    pub page_load_ms: f64,
    /// Total DOM node count (pre-filter).
    pub dom_nodes: usize,
    /// Number of nodes removed by the ad/tracker blocker.
    pub blocked_nodes: usize,
    /// Fraction of nodes blocked, in [0.0, 1.0].
    ///
    /// Computed branchlessly: `blocked / max(dom_nodes, 1)` as f32.
    pub block_ratio: f32,
    /// Render mode: 0 = full, 1 = partial, 2 = skeleton.
    pub render_mode: u8,
}

/// Derive a browsing telemetry event from page load statistics.
///
/// # Optimization notes
/// - `block_ratio` uses `max(dom_nodes, 1)` to avoid division by zero
///   without a branch; compiler emits a `cmov` for the `.max()`.
/// - Reciprocal multiply: `blocked as f32 * (1.0 / max(nodes, 1) as f32)`.
#[inline]
#[must_use]
pub fn browser_to_analytics_event(
    url: &str,
    load_ms: f64,
    dom_nodes: usize,
    blocked: usize,
    render_mode: u8,
) -> BrowserAnalyticsEvent {
    let url_hash = fnv1a(url.as_bytes());

    // Branchless block ratio: reciprocal of safe node count.
    let nodes_safe = dom_nodes.max(1) as f32;
    let rcp_nodes = 1.0_f32 / nodes_safe; // computed once
    let block_ratio = (blocked as f32 * rcp_nodes).min(1.0_f32);

    BrowserAnalyticsEvent {
        url_hash,
        page_load_ms: load_ms,
        dom_nodes,
        blocked_nodes: blocked,
        block_ratio,
        render_mode,
    }
}

// ── Bridge 5: Browser → Font (web font render request) ───────────────────

/// Web font render request for ALICE-Font.
///
/// The browser submits this record when it encounters a text node that
/// requires font rasterisation at a known container width.
pub struct BrowserFontRequest {
    /// FNV-1a hash of the URL bytes — associates the request with a page.
    pub url_hash: u64,
    /// UTF-8 byte length of the text to render.
    pub text_length: usize,
    /// Width of the containing box in CSS pixels.
    pub container_width: f32,
    /// Estimated glyph count (`text_length` as a conservative upper bound).
    ///
    /// A multi-byte UTF-8 codepoint may produce one glyph; using byte length
    /// as the upper bound avoids decoding cost on the bridge hot path.
    pub estimated_glyphs: usize,
}

/// Build a font render request from URL, text content, and container width.
///
/// # Optimization notes
/// - `estimated_glyphs` is set to `text.len()` (byte count) — an O(1)
///   upper bound that avoids UTF-8 decoding overhead on the bridge path.
/// - No heap allocation: text bytes are hashed in-place via the FNV loop.
#[inline]
#[must_use]
pub fn browser_to_font_request(url: &str, text: &str, container_width: f32) -> BrowserFontRequest {
    let url_hash = fnv1a(url.as_bytes());
    let text_length = text.len();
    // Conservative glyph estimate: one glyph per byte (upper bound).
    let estimated_glyphs = text_length;

    BrowserFontRequest {
        url_hash,
        text_length,
        container_width,
        estimated_glyphs,
    }
}

// ── Bridge 6: Browser → Search (page content indexing) ───────────────────

/// Page content index entry for ALICE-Search.
///
/// Produced after the browser extracts clean text from a filtered DOM tree.
pub struct BrowserSearchIndex {
    /// FNV-1a hash of the URL bytes — document identifier in the search index.
    pub url_hash: u64,
    /// FNV-1a hash of the text content bytes — content fingerprint for dedup.
    pub content_hash: u64,
    /// UTF-8 byte length of the extracted text.
    pub text_bytes: usize,
    /// Approximate word count (`text_bytes` / 5, rounded up).
    ///
    /// Average English word length is ~5 bytes; this avoids a full tokenisation
    /// pass on the bridge hot path.
    pub word_count: usize,
}

/// Produce a search index entry from URL and extracted page text.
///
/// # Optimization notes
/// - `word_count` approximated as `(text_bytes + 4) / 5` via integer
///   arithmetic — a single add and shift, no division by zero possible.
/// - Both hashes computed with one FNV-1a pass each; no heap allocation.
#[inline]
#[must_use]
pub fn browser_to_search_index(url: &str, text_content: &str) -> BrowserSearchIndex {
    let url_hash = fnv1a(url.as_bytes());
    let content_hash = fnv1a(text_content.as_bytes());
    let text_bytes = text_content.len();

    // Approximate word count: ceil(text_bytes / 5).
    // (text_bytes + 4) >> … won't work cleanly for 5, so use integer div+1.
    // One integer divide — cold path only (bridge produces one record per page).
    let word_count = text_bytes.div_ceil(5);

    BrowserSearchIndex {
        url_hash,
        content_hash,
        text_bytes,
        word_count,
    }
}

// ── Bridge 7: Browser → ML (classification feedback) ─────────────────────

/// Classification feedback record for ALICE-ML training pipeline.
///
/// Carries per-page node-type ratios so the ML pipeline can refine
/// the ad/tracker classifier without exposing raw DOM trees.
pub struct BrowserMlFeedback {
    /// FNV-1a hash of the URL bytes — sample identifier.
    pub url_hash: u64,
    /// Total DOM node count.
    pub total_nodes: usize,
    /// Fraction of nodes classified as main content, in [0.0, 1.0].
    pub content_ratio: f32,
    /// Fraction of nodes classified as advertisements, in [0.0, 1.0].
    pub ad_density: f32,
    /// Fraction of nodes classified as trackers, in [0.0, 1.0].
    pub tracker_density: f32,
}

/// Derive an ML feedback record from per-class node counts.
///
/// # Optimization notes
/// - All three ratios computed via reciprocal multiply `* rcp_total`;
///   `rcp_total` is derived once from `max(total, 1)`.
/// - All ratios clamped to 1.0 with a single `min` call (no branch).
#[inline]
#[must_use]
pub fn browser_to_ml_feedback(
    url: &str,
    total: usize,
    content: usize,
    ads: usize,
    trackers: usize,
) -> BrowserMlFeedback {
    let url_hash = fnv1a(url.as_bytes());

    // Reciprocal of total — computed once, reused for all three ratios.
    let rcp_total = 1.0_f32 / total.max(1) as f32;
    let content_ratio = (content as f32 * rcp_total).min(1.0_f32);
    let ad_density = (ads as f32 * rcp_total).min(1.0_f32);
    let tracker_density = (trackers as f32 * rcp_total).min(1.0_f32);

    BrowserMlFeedback {
        url_hash,
        total_nodes: total,
        content_ratio,
        ad_density,
        tracker_density,
    }
}

// ── Bridge 8: Browser → DNS (URL resolution request) ─────────────────────

/// DNS resolution request for ALICE-DNS.
///
/// Submitted by the browser before opening a TCP connection to a new host.
pub struct BrowserDnsRequest {
    /// FNV-1a hash of the full URL bytes — request correlation key.
    pub url_hash: u64,
    /// Byte length of the hostname portion extracted from the URL.
    ///
    /// Avoids allocating a separate hostname string; the DNS resolver can
    /// re-extract the hostname from the URL using this byte offset.
    pub hostname_bytes: usize,
    /// `true` when the URL scheme is `https`.
    pub is_https: bool,
}

/// Build a DNS resolution request from a URL string.
///
/// # Optimization notes
/// - Hostname detection uses `find("://")` for a fast linear scan; total
///   cost is O(url.len()) with no heap allocation.
/// - `is_https` is derived branchlessly via byte comparison on the scheme
///   prefix (`b"https"` length check against the `"://"` offset).
/// - `hostname_bytes` is the byte length of the authority component only;
///   computed with simple pointer arithmetic on the `&str` slice.
#[inline]
#[must_use]
pub fn browser_to_dns_request(url: &str) -> BrowserDnsRequest {
    let url_hash = fnv1a(url.as_bytes());
    let bytes = url.as_bytes();

    // Locate "://" separator to split scheme from authority.
    let scheme_end = bytes.windows(3).position(|w| w == b"://").unwrap_or(0);

    // is_https: scheme length == 5 and prefix == b"https" — branchless compare.
    let is_https = scheme_end == 5 && bytes.starts_with(b"https");

    // Authority starts after "://"; ends at '/', '?', '#', or end-of-string.
    let authority_start = scheme_end + 3;
    let authority_end = bytes[authority_start.min(bytes.len())..]
        .iter()
        .position(|&b| b == b'/' || b == b'?' || b == b'#')
        .map_or(bytes.len(), |p| authority_start + p);

    let hostname_bytes = authority_end.saturating_sub(authority_start);

    BrowserDnsRequest {
        url_hash,
        hostname_bytes,
        is_https,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    const TEST_URL: &str = "https://example.com/news/article?id=42";
    const TEST_TEXT: &str = "The quick brown fox jumps over the lazy dog";

    #[test]
    fn test_browser_to_db_page_record() {
        let rec = browser_to_db_page_record(TEST_URL, 200, 40, 18.5);

        assert_ne!(rec.content_hash, 0, "content_hash must be non-zero");
        assert_ne!(rec.url_hash, 0, "url_hash must be non-zero");
        assert_eq!(
            rec.content_hash, rec.url_hash,
            "both hashes derive from the URL"
        );
        assert_eq!(rec.node_count, 200);
        assert_eq!(rec.content_nodes, 160, "200 - 40 = 160 content nodes");
        // 40 removed: half=20, remainder=0 → ad=20, tracker=20
        assert_eq!(rec.ad_nodes, 20);
        assert_eq!(rec.tracker_nodes, 20);
        assert_eq!(rec.classification_ms, 18.5);
    }

    #[test]
    fn test_browser_to_cache_entry() {
        let entry = browser_to_cache_entry(TEST_URL, 150, 8_192);

        assert_ne!(entry.url_hash, 0, "url_hash must be non-zero");
        assert_ne!(entry.content_hash, 0, "content_hash must be non-zero");
        assert_eq!(entry.node_count, 150);
        assert_eq!(entry.byte_estimate, 8_192);
        // TTL = min(3600, max(30, 3600*4096 / 8192)) = max(30, 1800) = 1800
        assert_eq!(entry.ttl_seconds, 1_800, "ttl = 14745600 / 8192 = 1800");
    }

    #[test]
    fn test_browser_to_cdn_page() {
        let page = browser_to_cdn_page(TEST_URL, 32_768, 0);

        assert_ne!(page.content_hash, 0, "content_hash must be non-zero");
        assert_eq!(page.content_type, "text/html; charset=utf-8");
        assert_eq!(page.byte_size, 32_768);
        assert_eq!(page.render_mode, 0);
    }

    #[test]
    fn test_browser_to_analytics_event() {
        let event = browser_to_analytics_event(TEST_URL, 250.0, 100, 25, 1);

        assert_ne!(event.url_hash, 0, "url_hash must be non-zero");
        assert_eq!(event.page_load_ms, 250.0);
        assert_eq!(event.dom_nodes, 100);
        assert_eq!(event.blocked_nodes, 25);
        // block_ratio = 25 / 100 = 0.25
        assert!(
            (event.block_ratio - 0.25_f32).abs() < 1e-5,
            "block_ratio = {}",
            event.block_ratio
        );
        assert_eq!(event.render_mode, 1);

        // Zero dom_nodes must not panic and must yield ratio 0.0.
        let zero_event = browser_to_analytics_event(TEST_URL, 0.0, 0, 0, 0);
        assert_eq!(zero_event.block_ratio, 0.0);
    }

    #[test]
    fn test_browser_to_font_request() {
        let req = browser_to_font_request(TEST_URL, TEST_TEXT, 960.0);

        assert_ne!(req.url_hash, 0, "url_hash must be non-zero");
        assert_eq!(req.text_length, TEST_TEXT.len());
        assert_eq!(req.container_width, 960.0);
        // estimated_glyphs == text byte length (conservative upper bound).
        assert_eq!(req.estimated_glyphs, TEST_TEXT.len());
    }

    #[test]
    fn test_browser_to_search_index() {
        let idx = browser_to_search_index(TEST_URL, TEST_TEXT);

        assert_ne!(idx.url_hash, 0, "url_hash must be non-zero");
        assert_ne!(idx.content_hash, 0, "content_hash must be non-zero");
        assert_eq!(idx.text_bytes, TEST_TEXT.len());
        // word_count = ceil(43 / 5) = ceil(8.6) = 9 → (43+4)/5 = 47/5 = 9.
        assert_eq!(idx.word_count, 9, "word_count = (43+4)/5 = 9");
    }

    #[test]
    fn test_browser_to_ml_feedback() {
        let fb = browser_to_ml_feedback(TEST_URL, 100, 70, 20, 10);

        assert_ne!(fb.url_hash, 0, "url_hash must be non-zero");
        assert_eq!(fb.total_nodes, 100);
        assert!(
            (fb.content_ratio - 0.70_f32).abs() < 1e-5,
            "content_ratio = {}",
            fb.content_ratio
        );
        assert!(
            (fb.ad_density - 0.20_f32).abs() < 1e-5,
            "ad_density = {}",
            fb.ad_density
        );
        assert!(
            (fb.tracker_density - 0.10_f32).abs() < 1e-5,
            "tracker_density = {}",
            fb.tracker_density
        );

        // Zero total must not panic; all ratios must be 0.0.
        let zero_fb = browser_to_ml_feedback(TEST_URL, 0, 0, 0, 0);
        assert_eq!(zero_fb.content_ratio, 0.0);
        assert_eq!(zero_fb.ad_density, 0.0);
        assert_eq!(zero_fb.tracker_density, 0.0);
    }

    #[test]
    fn test_browser_to_dns_request() {
        let req = browser_to_dns_request(TEST_URL);

        assert_ne!(req.url_hash, 0, "url_hash must be non-zero");
        // "https://example.com/news/article?id=42" → hostname = "example.com" = 11 bytes.
        assert_eq!(
            req.hostname_bytes, 11,
            "hostname = 'example.com' (11 bytes)"
        );
        assert!(req.is_https, "URL scheme is https");

        // HTTP URL must not set is_https.
        let http_req = browser_to_dns_request("http://example.org/path");
        assert!(!http_req.is_https, "http URL must yield is_https = false");
        // "http://example.org/path" → hostname = "example.org" = 11 bytes.
        assert_eq!(http_req.hostname_bytes, 11);
    }
}
