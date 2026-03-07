//! Cross-domain bridges — ALICE-Document ↔ Text, Search, i18n, Analytics, Cache
//!
//! 5 bridges connecting PDF/document generation to text compression,
//! full-text search indexing, locale detection, analytics events,
//! and cache entries with page-count-based TTL.

use alice_document::PdfDocument;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Document page → Text tokenization input ─────────────────

/// Text tokenization input derived from a PDF document.
///
/// Extracts page count and rendered byte size so the Text layer can
/// estimate compression parameters without accessing raw page content.
pub struct DocumentTextTokens {
    /// FNV-1a hash over page_count, rendered_bytes bytes.
    pub content_hash: u64,
    /// Number of pages in the document.
    pub page_count: usize,
    /// Total rendered PDF byte size.
    pub rendered_bytes: usize,
    /// Estimated token count (heuristic: rendered_bytes / 5).
    pub estimated_tokens: usize,
    /// Estimated compression input size for ALICE-Text.
    pub compression_input_bytes: usize,
    /// Whether the document is likely text-heavy (rendered_bytes > page_count * 200).
    pub is_text_heavy: bool,
}

/// Convert a PDF document into text tokenization input metadata.
#[inline]
#[must_use]
pub fn document_page_to_text_tokens(doc: &PdfDocument) -> DocumentTextTokens {
    let page_count = doc.page_count();
    let rendered = doc.render();
    let rendered_bytes = rendered.len();
    let estimated_tokens = rendered_bytes / 5;
    let compression_input_bytes = rendered_bytes;
    let is_text_heavy = rendered_bytes > page_count * 200;

    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&(page_count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&(rendered_bytes as u64).to_le_bytes());

    DocumentTextTokens {
        content_hash: fnv1a(&key),
        page_count,
        rendered_bytes,
        estimated_tokens,
        compression_input_bytes,
        is_text_heavy,
    }
}

// ── Bridge 2: Document → Search index entry ───────────────────────────

/// Search index entry derived from a PDF document.
///
/// Extracts indexable metadata (page count, byte size, content hash) so
/// the Search layer can build an FM-Index over document content without
/// parsing PDF structure.
pub struct DocumentSearchIndex {
    /// FNV-1a hash over rendered PDF content.
    pub content_hash: u64,
    /// Number of pages.
    pub page_count: usize,
    /// Total rendered byte size (corpus size for FM-Index).
    pub corpus_bytes: usize,
    /// Recommended FM-Index sample step (clamped: max(1, corpus_bytes / 1024)).
    pub sample_step: usize,
    /// Whether the document is indexable (page_count > 0).
    pub is_indexable: bool,
}

/// Convert a PDF document into a search index entry.
#[inline]
#[must_use]
pub fn document_to_search_index(doc: &PdfDocument) -> DocumentSearchIndex {
    let page_count = doc.page_count();
    let rendered = doc.render();
    let corpus_bytes = rendered.len();
    let sample_step = if corpus_bytes > 1024 {
        corpus_bytes / 1024
    } else {
        1
    };

    DocumentSearchIndex {
        content_hash: fnv1a(&rendered),
        page_count,
        corpus_bytes,
        sample_step,
        is_indexable: page_count > 0,
    }
}

// ── Bridge 3: Document → i18n locale detection seed ───────────────────

/// Locale detection seed derived from a PDF document.
///
/// Analyzes rendered PDF bytes for ASCII ratio and byte frequency to
/// provide hints for locale detection (CJK vs Latin vs mixed).
pub struct DocumentLocaleSeed {
    /// FNV-1a hash over ascii_ratio, byte histogram sample bytes.
    pub content_hash: u64,
    /// Number of pages.
    pub page_count: usize,
    /// Total rendered bytes.
    pub total_bytes: usize,
    /// ASCII byte ratio [0.0, 1.0].
    pub ascii_ratio: f32,
    /// Locale hint discriminant: 0=Unknown, 1=Latin, 2=CJK, 3=Mixed.
    pub locale_hint: u8,
    /// Whether multi-byte sequences are detected (possible CJK/UTF-8).
    pub has_multibyte: bool,
}

/// Convert a PDF document into an i18n locale detection seed.
#[inline]
#[must_use]
pub fn document_to_i18n_locale(doc: &PdfDocument) -> DocumentLocaleSeed {
    let page_count = doc.page_count();
    let rendered = doc.render();
    let total_bytes = rendered.len();

    let mut ascii_count: usize = 0;
    let mut high_byte_count: usize = 0;
    for &b in &rendered {
        let is_ascii = (b < 128) as usize;
        ascii_count += is_ascii;
        high_byte_count += 1 - is_ascii;
    }

    let ascii_ratio = if total_bytes > 0 {
        ascii_count as f32 / total_bytes as f32
    } else {
        1.0
    };

    let has_multibyte = high_byte_count > 0;
    let locale_hint = match () {
        _ if total_bytes == 0 => 0,   // Unknown
        _ if ascii_ratio > 0.95 => 1, // Latin
        _ if ascii_ratio < 0.50 => 2, // CJK
        _ => 3,                       // Mixed
    };

    let mut key = [0u8; 14];
    key[0..8].copy_from_slice(&(total_bytes as u64).to_le_bytes());
    key[8..12].copy_from_slice(&ascii_ratio.to_bits().to_le_bytes());
    key[12] = locale_hint;
    key[13] = has_multibyte as u8;

    DocumentLocaleSeed {
        content_hash: fnv1a(&key),
        page_count,
        total_bytes,
        ascii_ratio,
        locale_hint,
        has_multibyte,
    }
}

// ── Bridge 4: Document metadata → Analytics event ─────────────────────

/// Analytics event derived from document metadata.
///
/// Captures page count, byte size, and generation complexity for
/// analytics dashboards.
pub struct DocumentAnalyticsEvent {
    /// FNV-1a hash over page_count, rendered_bytes bytes.
    pub content_hash: u64,
    /// Number of pages.
    pub page_count: usize,
    /// Rendered PDF byte size.
    pub rendered_bytes: usize,
    /// Bytes per page ratio.
    pub bytes_per_page: f32,
    /// Event kind discriminant: 0 = document_generated.
    pub event_kind: u8,
    /// Whether the document is multi-page.
    pub is_multipage: bool,
}

/// Convert document metadata into an analytics event.
#[inline]
#[must_use]
pub fn document_metadata_to_analytics(doc: &PdfDocument) -> DocumentAnalyticsEvent {
    let page_count = doc.page_count();
    let rendered = doc.render();
    let rendered_bytes = rendered.len();
    let bytes_per_page = if page_count > 0 {
        rendered_bytes as f32 / page_count as f32
    } else {
        0.0
    };
    let is_multipage = page_count > 1;

    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&(page_count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&(rendered_bytes as u64).to_le_bytes());
    key[16] = 0; // event_kind = document_generated

    DocumentAnalyticsEvent {
        content_hash: fnv1a(&key),
        page_count,
        rendered_bytes,
        bytes_per_page,
        event_kind: 0,
        is_multipage,
    }
}

// ── Bridge 5: Document → Cache with TTL based on page count ──────────

/// Cache entry for a document with page-count-based branchless TTL.
///
/// Multi-page documents (> 10 pages) get shorter TTL to conserve cache.
pub struct DocumentCacheEntry {
    /// FNV-1a hash over rendered PDF content.
    pub content_hash: u64,
    /// Number of pages.
    pub page_count: usize,
    /// Rendered byte size.
    pub rendered_bytes: usize,
    /// TTL in seconds (branchless: base 7200 - large_flag * 5400).
    pub ttl_secs: u32,
    /// Estimated memory footprint (= rendered_bytes).
    pub memory_bytes: usize,
}

/// Convert a document into a cache entry with branchless TTL.
#[inline]
#[must_use]
pub fn document_to_cache(doc: &PdfDocument) -> DocumentCacheEntry {
    let page_count = doc.page_count();
    let rendered = doc.render();
    let rendered_bytes = rendered.len();

    // Branchless TTL: 大ドキュメント (>10ページ) は短いTTL
    let is_large = (page_count > 10) as u32;
    let ttl_secs = 7200 - is_large * 5400;

    DocumentCacheEntry {
        content_hash: fnv1a(&rendered),
        page_count,
        rendered_bytes,
        ttl_secs,
        memory_bytes: rendered_bytes,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_document::{PageSize, PdfDocument, Point};

    fn make_doc() -> PdfDocument {
        let mut doc = PdfDocument::new();
        let page = doc.add_page(PageSize::A4);
        doc.add_text(page, "Hello World", Point::new(72.0, 72.0), 12.0);
        doc
    }

    fn make_multipage_doc() -> PdfDocument {
        let mut doc = PdfDocument::new();
        for i in 0..15 {
            let page = doc.add_page(PageSize::A4);
            doc.add_text(
                page,
                &format!("Page {}", i + 1),
                Point::new(72.0, 72.0),
                12.0,
            );
        }
        doc
    }

    // ── Bridge 1: Document → Text tokens ────────────────────────────

    #[test]
    fn test_document_to_text_tokens() {
        let doc = make_doc();
        let tokens = document_page_to_text_tokens(&doc);
        assert_ne!(tokens.content_hash, 0);
        assert_eq!(tokens.page_count, 1);
        assert!(tokens.rendered_bytes > 0);
        assert!(tokens.estimated_tokens > 0);
    }

    #[test]
    fn test_document_to_text_tokens_deterministic() {
        let doc = make_doc();
        let a = document_page_to_text_tokens(&doc);
        let b = document_page_to_text_tokens(&doc);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Bridge 2: Document → Search index ───────────────────────────

    #[test]
    fn test_document_to_search_index() {
        let doc = make_doc();
        let idx = document_to_search_index(&doc);
        assert_ne!(idx.content_hash, 0);
        assert_eq!(idx.page_count, 1);
        assert!(idx.corpus_bytes > 0);
        assert!(idx.is_indexable);
    }

    #[test]
    fn test_document_to_search_index_empty() {
        let doc = PdfDocument::new();
        let idx = document_to_search_index(&doc);
        assert_eq!(idx.page_count, 0);
        assert!(!idx.is_indexable);
    }

    // ── Bridge 3: Document → i18n locale ────────────────────────────

    #[test]
    fn test_document_to_i18n_locale() {
        let doc = make_doc();
        let locale = document_to_i18n_locale(&doc);
        assert_ne!(locale.content_hash, 0);
        assert_eq!(locale.page_count, 1);
        // PDF output is predominantly ASCII
        assert!(locale.ascii_ratio > 0.9);
        assert_eq!(locale.locale_hint, 1); // Latin
    }

    #[test]
    fn test_document_to_i18n_locale_empty() {
        let doc = PdfDocument::new();
        let locale = document_to_i18n_locale(&doc);
        assert_eq!(locale.page_count, 0);
        // 空ドキュメントでもPDFヘッダ/トレイラが出力されるためASCII主体 → Latin
        assert_eq!(locale.locale_hint, 1);
    }

    // ── Bridge 4: Document → Analytics ──────────────────────────────

    #[test]
    fn test_document_metadata_to_analytics() {
        let doc = make_doc();
        let evt = document_metadata_to_analytics(&doc);
        assert_ne!(evt.content_hash, 0);
        assert_eq!(evt.page_count, 1);
        assert!(evt.rendered_bytes > 0);
        assert_eq!(evt.event_kind, 0);
        assert!(!evt.is_multipage);
    }

    #[test]
    fn test_document_metadata_to_analytics_multipage() {
        let doc = make_multipage_doc();
        let evt = document_metadata_to_analytics(&doc);
        assert_eq!(evt.page_count, 15);
        assert!(evt.is_multipage);
        assert!(evt.bytes_per_page > 0.0);
    }

    // ── Bridge 5: Document → Cache ──────────────────────────────────

    #[test]
    fn test_document_to_cache_small() {
        let doc = make_doc();
        let cache = document_to_cache(&doc);
        assert_ne!(cache.content_hash, 0);
        assert_eq!(cache.page_count, 1);
        assert_eq!(cache.ttl_secs, 7200); // 小ドキュメント → 長いTTL
    }

    #[test]
    fn test_document_to_cache_large() {
        let doc = make_multipage_doc();
        let cache = document_to_cache(&doc);
        assert_eq!(cache.page_count, 15);
        // 10ページ超 → branchless TTL: 7200 - 1*5400 = 1800
        assert_eq!(cache.ttl_secs, 1800);
    }

    #[test]
    fn test_document_to_cache_deterministic() {
        let doc = make_doc();
        let a = document_to_cache(&doc);
        let b = document_to_cache(&doc);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
