//! Document bridges — ALICE-Document ↔ DB, Cache, CDN, Search, Font
//!
//! 5 bridges connecting the document processing layer to the ALICE ecosystem.
//! Covers document storage in DB, document caching, CDN delivery,
//! full-text search indexing, and text layout linkage to Font.

use alice_document::{PageSize, PdfDocument};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Document → DB (document storage record) ────────────────────

/// Document storage record for ALICE-DB.
///
/// Written when a document is persisted so the database layer can store
/// and query document metadata by document ID, page count, or byte size.
pub struct DocumentDbRecord {
    /// FNV-1a hash over document ID and total byte size.
    pub content_hash: u64,
    /// FNV-1a hash of the document ID string.
    pub document_id_hash: u64,
    /// Total number of pages in the document.
    pub page_count: u32,
    /// Total byte size of the serialized document.
    pub byte_size: usize,
}

/// Convert a PDF document into a DB storage record for ALICE-DB.
#[inline]
#[must_use]
pub fn document_pdf_to_db_record(doc: &PdfDocument, document_id: &str) -> DocumentDbRecord {
    let document_id_hash = fnv1a(document_id.as_bytes());
    let page_count = doc.page_count() as u32;
    // ページ内容の公開APIがないため、ページ数×64バイトをサイズの代理値とする。
    let byte_size: usize = page_count as usize * 64;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&document_id_hash.to_le_bytes());
    key[8..12].copy_from_slice(&page_count.to_le_bytes());
    key[12..16].copy_from_slice(&(byte_size as u32).to_le_bytes());
    DocumentDbRecord {
        content_hash: fnv1a(&key),
        document_id_hash,
        page_count,
        byte_size,
    }
}

// ── Bridge 2: Document → Cache (document cache entry) ────────────────────

/// Document cache entry for ALICE-Cache.
///
/// Caches the serialized document bytes so repeated fetch requests skip
/// re-generation.  Large documents (> 1 MiB) receive a shorter TTL to
/// avoid excessive memory pressure in the cache layer.
pub struct DocumentCacheEntry {
    /// FNV-1a hash over document ID and byte size — cache key.
    pub content_hash: u64,
    /// FNV-1a hash of the document ID.
    pub document_id_hash: u64,
    /// Total byte size of the cached document.
    pub byte_size: usize,
    /// Number of pages in the cached document.
    pub page_count: u32,
    /// Cache TTL in seconds: 60 for large docs (> 1 MiB), else 600.
    pub ttl_secs: u32,
}

/// Build a document cache entry for ALICE-Cache.
///
/// TTL is computed branchlessly: large=1 → 600-540=60, small=0 → 600.
#[inline]
#[must_use]
pub fn document_pdf_to_cache_entry(
    doc: &PdfDocument,
    document_id: &str,
    byte_size: usize,
) -> DocumentCacheEntry {
    let document_id_hash = fnv1a(document_id.as_bytes());
    let page_count = doc.page_count() as u32;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&document_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&(byte_size as u64).to_le_bytes());
    // Branchless TTL: large=1 → 600-540=60, small=0 → 600.
    let large = (byte_size > 1_048_576) as u32;
    let ttl_secs = 600 - large * 540;
    DocumentCacheEntry {
        content_hash: fnv1a(&key),
        document_id_hash,
        byte_size,
        page_count,
        ttl_secs,
    }
}

// ── Bridge 3: Document → CDN (document delivery descriptor) ──────────────

/// Document delivery descriptor for ALICE-CDN.
///
/// Packages a document for CDN delivery so the CDN layer can apply
/// appropriate content-type headers, cache-control directives, and
/// route requests to the nearest edge PoP.
pub struct DocumentCdnDelivery {
    /// FNV-1a hash over document ID and format byte.
    pub content_hash: u64,
    /// FNV-1a hash of the document ID — CDN object key.
    pub document_id_hash: u64,
    /// Document format: 0=PDF, 1=HTML, 2=EPUB.
    pub format: u8,
    /// Byte size of the document for Content-Length header.
    pub byte_size: usize,
    /// CDN cache-control max-age in seconds.
    pub max_age_secs: u32,
}

/// Build a CDN delivery descriptor for a document.
#[inline]
#[must_use]
pub fn document_to_cdn_delivery(
    document_id: &str,
    format: u8,
    byte_size: usize,
) -> DocumentCdnDelivery {
    let document_id_hash = fnv1a(document_id.as_bytes());
    let fmt = format.min(2);
    let mut key = [0u8; 9];
    key[0..8].copy_from_slice(&document_id_hash.to_le_bytes());
    key[8] = fmt;
    DocumentCdnDelivery {
        content_hash: fnv1a(&key),
        document_id_hash,
        format: fmt,
        byte_size,
        max_age_secs: 86_400, // 24時間 — ドキュメントは生成後に不変。
    }
}

// ── Bridge 4: Document → Search (document search index record) ────────────

/// Document search index record for ALICE-Search.
///
/// Enables full-text search over document content with page-count faceting
/// and format-based filtering in the ALICE-Search index.
pub struct DocumentSearchRecord {
    /// FNV-1a hash over document ID bytes — search document ID.
    pub content_hash: u64,
    /// FNV-1a hash of the document ID.
    pub document_id_hash: u64,
    /// Number of pages — used for numeric range queries and faceting.
    pub page_count: u32,
    /// Estimated total text byte size — used for snippet sizing.
    pub text_byte_size: usize,
    /// Document format code: 0=PDF, 1=HTML, 2=EPUB.
    pub format: u8,
}

/// Build a search index record for a PDF document.
#[inline]
#[must_use]
pub fn document_pdf_to_search_record(
    doc: &PdfDocument,
    document_id: &str,
    format: u8,
) -> DocumentSearchRecord {
    let document_id_hash = fnv1a(document_id.as_bytes());
    let page_count = doc.page_count() as u32;
    // ページ内容の公開APIがないため、ページ数×64バイトをテキストサイズの代理値とする。
    let text_byte_size: usize = page_count as usize * 64;
    DocumentSearchRecord {
        content_hash: document_id_hash,
        document_id_hash,
        page_count,
        text_byte_size,
        format: format.min(2),
    }
}

// ── Bridge 5: Document → Font (text layout link) ──────────────────────────

/// Text layout request for ALICE-Font.
///
/// Links a document page to the font layer for text measurement and
/// shaping so the document engine can compute accurate line breaks.
pub struct DocumentFontLayoutRequest {
    /// FNV-1a hash over document ID, page index, and line count bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the document ID.
    pub document_id_hash: u64,
    /// Zero-based page index.
    pub page_index: u32,
    /// Number of lines on this page requiring layout.
    pub line_count: u32,
    /// Page width in points (1/72 inch).
    pub page_width: f64,
    /// Page height in points.
    pub page_height: f64,
}

/// Build a font layout request from a specific document page for ALICE-Font.
///
/// `page_width` および `page_height` は A4 固定値を使用する
/// (`PdfPage` のフィールドは非公開のため)。
/// ページが範囲外の場合は `None` を返す。
#[inline]
#[must_use]
pub fn document_page_to_font_layout_request(
    doc: &PdfDocument,
    document_id: &str,
    page_index: u32,
) -> Option<DocumentFontLayoutRequest> {
    // page_count() が公開されている唯一のページ情報 API。
    if page_index as usize >= doc.page_count() {
        return None;
    }
    let document_id_hash = fnv1a(document_id.as_bytes());
    // PdfPage のフィールドは非公開。行数は 0 で初期化し、寸法は A4 を使用する。
    let line_count: u32 = 0;
    let page_width = PageSize::A4.width_pt;
    let page_height = PageSize::A4.height_pt;
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&document_id_hash.to_le_bytes());
    key[8..12].copy_from_slice(&page_index.to_le_bytes());
    key[12..16].copy_from_slice(&line_count.to_le_bytes());
    key[16..20].copy_from_slice(&(page_width as u32).to_le_bytes());
    Some(DocumentFontLayoutRequest {
        content_hash: fnv1a(&key),
        document_id_hash,
        page_index,
        line_count,
        page_width,
        page_height,
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_document::{PageSize, PdfDocument};

    /// ページ数だけ指定して PdfDocument を構築するヘルパー。
    /// PdfPage のフィールドは非公開のため add_page() で追加する。
    fn make_doc(page_count: usize) -> PdfDocument {
        let mut doc = PdfDocument::new();
        for _ in 0..page_count {
            doc.add_page(PageSize::A4);
        }
        doc
    }

    #[test]
    fn test_document_pdf_to_db_record() {
        let doc = make_doc(2);
        let rec = document_pdf_to_db_record(&doc, "doc-001");
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.document_id_hash, 0);
        assert_eq!(rec.page_count, 2);
        assert!(rec.byte_size > 0);
    }

    #[test]
    fn test_document_cache_entry_small_ttl() {
        // byte_size <= 1 MiB → ttl = 600
        let doc = make_doc(1);
        let entry = document_pdf_to_cache_entry(&doc, "doc-002", 100_000);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 600);
        assert_eq!(entry.byte_size, 100_000);
    }

    #[test]
    fn test_document_cache_entry_large_ttl() {
        // byte_size > 1 MiB → ttl = 60
        let doc = make_doc(1);
        let entry = document_pdf_to_cache_entry(&doc, "doc-003", 2_000_000);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_document_cdn_delivery_format_clamped() {
        let d = document_to_cdn_delivery("doc-004", 99, 50_000);
        assert_eq!(d.format, 2); // EPUB に丸め込み
        assert_eq!(d.max_age_secs, 86_400);
        assert_eq!(d.byte_size, 50_000);
    }

    #[test]
    fn test_document_cdn_delivery_pdf() {
        let d = document_to_cdn_delivery("doc-005", 0, 12_000);
        assert_eq!(d.format, 0);
        assert_ne!(d.content_hash, 0);
    }

    #[test]
    fn test_document_pdf_to_search_record() {
        let doc = make_doc(1);
        let rec = document_pdf_to_search_record(&doc, "doc-006", 0);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.page_count, 1);
        assert_eq!(rec.format, 0);
        assert!(rec.text_byte_size > 0);
    }

    #[test]
    fn test_document_page_to_font_layout_request_valid() {
        let doc = make_doc(2);
        let req = document_page_to_font_layout_request(&doc, "doc-007", 0);
        assert!(req.is_some());
        let req = req.unwrap();
        assert_ne!(req.content_hash, 0);
        assert_eq!(req.page_index, 0);
        // PdfPage フィールドは非公開のため line_count=0、寸法は A4 固定値。
        assert_eq!(req.line_count, 0);
        assert!((req.page_width - PageSize::A4.width_pt).abs() < 0.01);
    }

    #[test]
    fn test_document_page_to_font_layout_request_out_of_bounds() {
        let doc = make_doc(1);
        let req = document_page_to_font_layout_request(&doc, "doc-008", 5);
        assert!(req.is_none());
    }

    #[test]
    fn test_hash_determinism() {
        let doc = make_doc(1);
        let r1 = document_pdf_to_db_record(&doc, "doc-det");
        let r2 = document_pdf_to_db_record(&doc, "doc-det");
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.document_id_hash, r2.document_id_hash);
    }
}
