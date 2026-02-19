//! Manga bridges — ALICE-Manga ↔ SDF, CDN, Cache, DB, Text, Search, Print, Codec, Font
//!
//! 9 bridges connecting SDF manga creation engine to the ALICE ecosystem.

use alice_manga::MangaPage;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

#[inline(always)]
fn page_hash(page: &MangaPage) -> u64 {
    let size = page.estimate_size();
    let elem = page.element_count();
    let data = [size.to_le_bytes().as_slice(), &elem.to_le_bytes()].concat();
    fnv1a(&data)
}

// ── Bridge 1: Manga → SDF (page → SDF compilation) ─────────────────────

/// SDF compilation stats from ALICE-Manga page.
pub struct MangaSdfResult {
    /// Element count.
    pub element_count: usize,
    /// Page dimensions (mm).
    pub page_size: (f32, f32),
    /// Estimated bytes.
    pub estimated_bytes: usize,
}

/// Compile Manga page stats for ALICE-SDF.
#[inline]
pub fn manga_to_sdf_page(page: &MangaPage) -> MangaSdfResult {
    let (w, h) = page.size.dimensions();
    MangaSdfResult {
        element_count: page.element_count(),
        page_size: (w, h),
        estimated_bytes: page.estimate_size(),
    }
}

// ── Bridge 2: Manga → CDN (chapter delivery) ────────────────────────────

/// Chapter delivery package for ALICE-CDN.
pub struct MangaCdnPackage {
    /// Content hash.
    pub content_hash: u64,
    /// Page count.
    pub page_count: usize,
    /// Total estimated bytes.
    pub total_bytes: usize,
    /// MIME type.
    pub content_type: &'static str,
}

/// Package manga chapter for ALICE-CDN delivery.
#[inline]
pub fn manga_to_cdn_package(pages: &[&MangaPage]) -> MangaCdnPackage {
    let total: usize = pages.iter().map(|p| p.estimate_size()).sum();
    let data = [pages.len().to_le_bytes().as_slice(), &total.to_le_bytes()].concat();
    MangaCdnPackage {
        content_hash: fnv1a(&data),
        page_count: pages.len(),
        total_bytes: total,
        content_type: "application/x-alice-manga",
    }
}

// ── Bridge 3: Manga → Cache (page caching) ──────────────────────────────

/// Page cache entry for ALICE-Cache.
pub struct MangaCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Element count.
    pub element_count: usize,
    /// Estimated bytes.
    pub estimated_bytes: usize,
}

/// Cache Manga page for ALICE-Cache.
#[inline]
pub fn manga_to_cache_entry(page: &MangaPage) -> MangaCacheEntry {
    MangaCacheEntry {
        content_hash: page_hash(page),
        element_count: page.element_count(),
        estimated_bytes: page.estimate_size(),
    }
}

// ── Bridge 4: Manga → DB (chapter persistence) ──────────────────────────

/// Chapter persistence record for ALICE-DB.
pub struct MangaDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Page count.
    pub page_count: usize,
    /// Total elements across all pages.
    pub total_elements: usize,
    /// Total estimated bytes.
    pub total_bytes: usize,
}

/// Serialize manga chapter for ALICE-DB persistence.
#[inline]
pub fn manga_to_db_record(pages: &[&MangaPage]) -> MangaDbRecord {
    let total_elements: usize = pages.iter().map(|p| p.element_count()).sum();
    let total_bytes: usize = pages.iter().map(|p| p.estimate_size()).sum();
    let data = [pages.len().to_le_bytes().as_slice(), &total_elements.to_le_bytes()].concat();
    MangaDbRecord {
        content_hash: fnv1a(&data),
        page_count: pages.len(),
        total_elements,
        total_bytes,
    }
}

// ── Bridge 5: Manga → Text (dialogue extraction) ────────────────────────

/// Dialogue metadata from ALICE-Manga page.
pub struct MangaTextPayload {
    /// Number of balloons.
    pub balloon_count: usize,
    /// Total elements (panels + balloons + tones + strokes).
    pub total_elements: usize,
    /// Content hash.
    pub content_hash: u64,
}

/// Extract dialogue metadata for ALICE-Text compression.
#[inline]
pub fn manga_to_text_payload(page: &MangaPage) -> MangaTextPayload {
    let balloons = page.balloons().len();
    let elements = page.element_count();
    MangaTextPayload {
        balloon_count: balloons,
        total_elements: elements,
        content_hash: page_hash(page),
    }
}

// ── Bridge 6: Manga → Search (content indexing) ─────────────────────────

/// Search index metadata from ALICE-Manga page.
pub struct MangaSearchIndex {
    /// Element count.
    pub element_count: usize,
    /// Panel count.
    pub panel_count: usize,
    /// Page content hash.
    pub page_hash: u64,
}

/// Index Manga page metadata for ALICE-Search.
#[inline]
pub fn manga_to_search_index(page: &MangaPage) -> MangaSearchIndex {
    MangaSearchIndex {
        element_count: page.element_count(),
        panel_count: page.panels().len(),
        page_hash: page_hash(page),
    }
}

// ── Bridge 7: Manga → Print (manga print output) ────────────────────────

/// Print configuration for ALICE-Manga page.
pub struct MangaPrintConfig {
    /// Page dimensions (mm).
    pub page_size: (f32, f32),
    /// Element count.
    pub element_count: usize,
    /// DPI setting.
    pub dpi: u32,
    /// Estimated print layers.
    pub estimated_layers: usize,
}

/// Configure Manga page for ALICE-Print output.
#[inline]
pub fn manga_to_print_config(page: &MangaPage, dpi: u32) -> MangaPrintConfig {
    let (w, h) = page.size.dimensions();
    let layers = (h / 0.2) as usize; // 0.2mm layer height
    MangaPrintConfig {
        page_size: (w, h),
        element_count: page.element_count(),
        dpi,
        estimated_layers: layers,
    }
}

// ── Bridge 8: Manga → Codec (image compression config) ──────────────────

/// Image compression config for ALICE-Manga page.
pub struct MangaCodecConfig {
    /// Image width in pixels.
    pub width: usize,
    /// Image height in pixels.
    pub height: usize,
    /// Element count.
    pub element_count: usize,
    /// Estimated bits per pixel.
    pub estimated_bits_per_pixel: f32,
}

/// Configure image compression for ALICE-Codec.
#[inline]
pub fn manga_to_codec_config(page: &MangaPage) -> MangaCodecConfig {
    let (w, h) = page.size.dimensions();
    let px_w = (w * 10.0) as usize; // ~10 pixels per mm at 254 DPI
    let px_h = (h * 10.0) as usize;
    let elements = page.element_count();
    // Manga is mostly black/white → low bpp
    let bpp = if elements > 10 { 2.0 } else { 1.0 };
    MangaCodecConfig { width: px_w, height: px_h, element_count: elements, estimated_bits_per_pixel: bpp }
}

// ── Bridge 9: Manga → Font (balloon glyph metrics request) ──────────────

/// Glyph metrics request from ALICE-Manga to ALICE-Font.
///
/// Provides balloon dimensions and text content so that ALICE-Font can
/// compute optimal font size, line breaks, and vertical column layout
/// without Manga needing to know font internals.
pub struct MangaFontMetricsRequest {
    /// Balloon width in mm.
    pub balloon_width_mm: f32,
    /// Balloon height in mm.
    pub balloon_height_mm: f32,
    /// Number of characters in the dialogue.
    pub char_count: usize,
    /// Content hash for cache lookup.
    pub content_hash: u64,
    /// Suggested font size in mm (estimated from balloon area).
    pub suggested_font_size_mm: f32,
    /// Estimated column count for vertical layout.
    pub estimated_columns: usize,
}

/// Request glyph metrics from ALICE-Font for manga balloon text sizing.
///
/// Estimates optimal font size and column layout from balloon dimensions
/// and character count, using area-based heuristics.
#[inline]
pub fn manga_to_font_metrics_request(
    page: &MangaPage,
    balloon_width_mm: f32,
    balloon_height_mm: f32,
    text: &str,
) -> MangaFontMetricsRequest {
    let char_count = text.chars().count();
    let area = balloon_width_mm * balloon_height_mm;
    // Estimate font size: sqrt(area / char_count) with 60% fill factor
    let rcp_chars = 1.0 / (char_count.max(1) as f32);
    let font_size = (area * rcp_chars * 0.6).sqrt().max(3.0).min(24.0);
    // Vertical columns: height / (font_size * 1.5 line spacing)
    let chars_per_col = (balloon_height_mm / (font_size * 1.5)).max(1.0) as usize;
    let columns = ((char_count + chars_per_col - 1) / chars_per_col).max(1);
    let hash = page_hash(page);
    MangaFontMetricsRequest {
        balloon_width_mm,
        balloon_height_mm,
        char_count,
        content_hash: hash,
        suggested_font_size_mm: font_size,
        estimated_columns: columns,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_manga::PageSize;

    fn test_page() -> MangaPage {
        MangaPage::new(PageSize::B4)
    }

    #[test]
    fn test_manga_to_sdf_page() {
        let page = test_page();
        let result = manga_to_sdf_page(&page);
        assert!(result.page_size.0 > 0.0);
        assert!(result.page_size.1 > 0.0);
    }

    #[test]
    fn test_manga_to_cdn_package() {
        let page = test_page();
        let pages: Vec<&MangaPage> = vec![&page];
        let pkg = manga_to_cdn_package(&pages);
        assert_eq!(pkg.page_count, 1);
        assert_eq!(pkg.content_type, "application/x-alice-manga");
    }

    #[test]
    fn test_manga_to_cache_entry() {
        let page = test_page();
        let entry = manga_to_cache_entry(&page);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_manga_to_db_record() {
        let page = test_page();
        let pages: Vec<&MangaPage> = vec![&page];
        let rec = manga_to_db_record(&pages);
        assert_eq!(rec.page_count, 1);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_manga_to_text_payload() {
        let page = test_page();
        let payload = manga_to_text_payload(&page);
        assert_ne!(payload.content_hash, 0);
    }

    #[test]
    fn test_manga_to_search_index() {
        let page = test_page();
        let idx = manga_to_search_index(&page);
        assert_ne!(idx.page_hash, 0);
    }

    #[test]
    fn test_manga_to_print_config() {
        let page = test_page();
        let cfg = manga_to_print_config(&page, 300);
        assert_eq!(cfg.dpi, 300);
        assert!(cfg.page_size.0 > 0.0);
        assert!(cfg.estimated_layers > 0);
    }

    #[test]
    fn test_manga_to_codec_config() {
        let page = test_page();
        let cfg = manga_to_codec_config(&page);
        assert!(cfg.width > 0);
        assert!(cfg.height > 0);
        assert!(cfg.estimated_bits_per_pixel > 0.0);
    }

    #[test]
    fn test_manga_to_font_metrics_request() {
        let page = test_page();
        let req = manga_to_font_metrics_request(&page, 60.0, 80.0, "こんにちは世界");
        assert!((req.balloon_width_mm - 60.0).abs() < 0.01);
        assert!((req.balloon_height_mm - 80.0).abs() < 0.01);
        assert_eq!(req.char_count, 7);
        assert!(req.suggested_font_size_mm >= 3.0);
        assert!(req.suggested_font_size_mm <= 24.0);
        assert!(req.estimated_columns >= 1);
        assert_ne!(req.content_hash, 0);
    }
}
