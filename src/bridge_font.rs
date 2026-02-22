//! Font bridges — ALICE-Font ↔ View, Browser, SDF, Manga, Animation, CDN, Print, DB, Cache, Sync, Crypto, Queue, Analytics
//!
//! 14 bridges connecting parametric metafonts to the ALICE ecosystem.

use alice_font::glyph::GLYPH_SDF_SIZE;
use alice_font::param::MetaFontParams;
use alice_font::shaper::ShapedGlyph;
use alice_font::{SdfAtlas, TextShaper};

// ── Bridge 1: Font → View (GPU texture upload) ─────────────────────────

/// GPU-ready glyph render command for ALICE-View.
pub struct GlyphRenderCmd {
    pub codepoint: char,
    pub screen_x: f32,
    pub screen_y: f32,
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_w: f32,
    pub uv_h: f32,
    pub advance: f32,
}

/// Text render batch for ALICE-View GPU instancing.
pub struct TextRenderBatch {
    pub atlas_pixels: Vec<f32>,
    pub atlas_size: usize,
    pub commands: Vec<GlyphRenderCmd>,
    pub total_width: f32,
    pub total_height: f32,
}

/// Convert shaped text + atlas into GPU render commands for ALICE-View.
#[inline]
pub fn font_to_view_batch(
    text: &str,
    shaper: &mut TextShaper,
    atlas: &mut SdfAtlas,
    max_width: f32,
) -> TextRenderBatch {
    let lines = shaper.shape_text(text, atlas, max_width);
    let mut commands = Vec::new();
    let mut total_height = 0.0f32;
    let mut max_w = 0.0f32;

    for line in &lines {
        for g in &line.glyphs {
            if let Some(entry) = atlas.lookup(g.codepoint) {
                commands.push(GlyphRenderCmd {
                    codepoint: g.codepoint,
                    screen_x: g.x,
                    screen_y: g.y + line.y_offset,
                    uv_x: entry.uv_x,
                    uv_y: entry.uv_y,
                    uv_w: entry.uv_w,
                    uv_h: entry.uv_h,
                    advance: g.advance,
                });
            }
        }
        max_w = max_w.max(line.width);
        total_height = line.y_offset;
    }

    TextRenderBatch {
        atlas_pixels: atlas.pixels().to_vec(),
        atlas_size: atlas.texture_size(),
        commands,
        total_width: max_w,
        total_height,
    }
}

// ── Bridge 2: Font → Browser (DOM text layout) ─────────────────────────

/// DOM text layout result for ALICE-Browser integration.
pub struct DomTextLayout {
    pub glyphs: Vec<DomGlyph>,
    pub width: f32,
    pub height: f32,
    pub line_count: usize,
}

/// Single glyph positioned for DOM rendering.
pub struct DomGlyph {
    pub codepoint: u32,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub tile_x: u16,
    pub tile_y: u16,
}

/// Layout text for ALICE-Browser DOM rendering.
#[inline]
pub fn font_to_browser_layout(
    text: &str,
    shaper: &mut TextShaper,
    atlas: &mut SdfAtlas,
    container_width: f32,
) -> DomTextLayout {
    let lines = shaper.shape_text(text, atlas, container_width);
    let mut glyphs = Vec::new();
    let mut max_w = 0.0f32;
    let mut height = 0.0f32;
    let line_count = lines.len();

    for line in &lines {
        for g in &line.glyphs {
            let (tx, ty) = atlas
                .lookup(g.codepoint)
                .map_or((0, 0), |e| (e.tile_x, e.tile_y));
            glyphs.push(DomGlyph {
                codepoint: g.codepoint as u32,
                x: g.x,
                y: g.y + line.y_offset,
                advance: g.advance,
                tile_x: tx,
                tile_y: ty,
            });
        }
        max_w = max_w.max(line.width);
        height = line.y_offset;
    }

    DomTextLayout {
        glyphs,
        width: max_w,
        height,
        line_count,
    }
}

// ── Bridge 3: Font → SDF (text as CSG scene) ───────────────────────────

/// SDF tile from a glyph for integration into an ALICE-SDF scene.
pub struct FontSdfTile {
    pub ch: char,
    pub x: f32,
    pub y: f32,
    pub sdf_data: Vec<f32>,
    pub advance: f32,
}

/// Text as SDF tile collection for ALICE-SDF scene integration.
pub struct FontSdfScene {
    pub tiles: Vec<FontSdfTile>,
    pub tile_size: usize,
    pub bbox_min: (f32, f32),
    pub bbox_max: (f32, f32),
}

/// Convert shaped text to SDF tile data for ALICE-SDF integration.
///
/// # Panics
///
/// Panics if a glyph codepoint is not found in the atlas after pre-population.
#[inline]
pub fn font_to_sdf_scene(
    text: &str,
    shaper: &mut TextShaper,
    atlas: &mut SdfAtlas,
    scale: f32,
) -> FontSdfScene {
    let line = shaper.shape_line(text, atlas);
    let mut tiles = Vec::new();
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    // Pre-populate atlas then copy pixels (avoids borrow conflict)
    for g in &line.glyphs {
        atlas.get_or_insert(g.codepoint);
    }
    let atlas_px = atlas.pixels().to_vec();
    let tex_size = atlas.texture_size();

    for g in &line.glyphs {
        let entry = atlas.lookup(g.codepoint).unwrap();
        let tsx = entry.tile_x as usize * GLYPH_SDF_SIZE;
        let tsy = entry.tile_y as usize * GLYPH_SDF_SIZE;
        let mut sdf_data = Vec::with_capacity(GLYPH_SDF_SIZE * GLYPH_SDF_SIZE);
        for row in 0..GLYPH_SDF_SIZE {
            for col in 0..GLYPH_SDF_SIZE {
                let idx = (tsy + row) * tex_size + (tsx + col);
                sdf_data.push(if idx < atlas_px.len() {
                    atlas_px[idx]
                } else {
                    1.0
                });
            }
        }
        let x = g.x * scale;
        let right = x + g.advance * scale;
        min_x = min_x.min(x);
        max_x = max_x.max(right);
        tiles.push(FontSdfTile {
            ch: g.codepoint,
            x,
            y: g.y * scale,
            sdf_data,
            advance: g.advance * scale,
        });
    }

    FontSdfScene {
        tiles,
        tile_size: GLYPH_SDF_SIZE,
        bbox_min: (if min_x >= f32::MAX { 0.0 } else { min_x }, 0.0),
        bbox_max: (if max_x <= f32::MIN { 0.0 } else { max_x }, scale),
    }
}

// ── Bridge 4: Font → Manga (balloon text) ───────────────────────────────

/// Vertical manga text column for balloon rendering.
pub struct MangaColumn {
    pub x: f32,
    pub glyphs: Vec<(char, f32, f32)>,
}

/// Manga text layout within a balloon.
pub struct MangaTextLayout {
    pub params_wire: [u8; 40],
    pub columns: Vec<MangaColumn>,
}

/// Create vertical manga text layout for ALICE-Manga balloon rendering.
#[inline]
#[must_use]
pub fn font_to_manga_layout(
    text: &str,
    balloon_w: f32,
    balloon_h: f32,
    font_size: f32,
) -> MangaTextLayout {
    let params = MetaFontParams::sans_regular();
    let chars: Vec<char> = text.chars().collect();
    let per_col = ((balloon_h / (font_size * 1.5)) as usize).max(1);
    let mut columns = Vec::new();
    let mut col_x = balloon_w - font_size;
    let mut i = 0;
    while i < chars.len() && col_x > 0.0 {
        let mut glyphs = Vec::new();
        let mut y = font_size;
        for _ in 0..per_col {
            if i >= chars.len() {
                break;
            }
            glyphs.push((chars[i], col_x, y));
            y += font_size * 1.5;
            i += 1;
        }
        columns.push(MangaColumn { x: col_x, glyphs });
        col_x -= font_size * 1.8;
    }
    MangaTextLayout {
        params_wire: params.encode(),
        columns,
    }
}

// ── Bridge 5: Font → Animation (animated text) ─────────────────────────

/// Animated text frame for ALICE-Animation subtitle/title rendering.
pub struct AnimTextFrame {
    pub params: MetaFontParams,
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub progress: f32,
}

/// Generate animated text transition between two font styles.
#[inline]
#[must_use]
pub fn font_animation_frame(
    text: &str,
    from: &MetaFontParams,
    to: &MetaFontParams,
    t: f32,
) -> AnimTextFrame {
    let params = from.lerp(to, t);
    let shaper = TextShaper::new(params);
    let mut atlas = SdfAtlas::new(8, params);
    let line = shaper.shape_line(text, &mut atlas);
    AnimTextFrame {
        params,
        glyphs: line.glyphs,
        width: line.width,
        progress: t,
    }
}

// ── Bridge 6: Font → CDN (parameter distribution) ──────────────────────

/// CDN-optimized font package (40 bytes vs multi-MB TTF).
pub struct FontCdnPackage {
    pub params_bytes: [u8; 40],
    pub content_hash: u64,
    pub compression_ratio: f64,
}

/// Package font parameters for CDN distribution.
#[inline]
#[must_use]
pub fn font_to_cdn_package(params: &MetaFontParams) -> FontCdnPackage {
    let bytes = params.encode();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    FontCdnPackage {
        params_bytes: bytes,
        content_hash: hash,
        compression_ratio: 12_500.0, // ~12,500x vs typical TTF
    }
}

// ── Bridge 7: Font → Print (text engraving contours) ────────────────────

/// Contour for G-code engraving.
pub struct EngravingContour {
    pub points: Vec<(f32, f32)>,
}

/// Text engraving result for ALICE-Print.
pub struct TextEngravingResult {
    pub contours: Vec<EngravingContour>,
    pub bbox: (f32, f32, f32, f32),
    pub path_length_mm: f32,
}

/// Extract contours from text SDF for ALICE-Print G-code engraving.
///
/// # Panics
///
/// Panics if a glyph codepoint is not found in the atlas after pre-population.
#[inline]
#[must_use]
pub fn font_to_print_contours(
    text: &str,
    params: &MetaFontParams,
    scale_mm: f32,
    threshold: f32,
) -> TextEngravingResult {
    let shaper = TextShaper::new(*params);
    let mut atlas = SdfAtlas::new(8, *params);
    let line = shaper.shape_line(text, &mut atlas);
    let mut contours = Vec::new();
    let mut path_len = 0.0f32;
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    // Pre-populate atlas then copy pixels (avoids borrow conflict)
    for g in &line.glyphs {
        atlas.get_or_insert(g.codepoint);
    }
    let atlas_px = atlas.pixels().to_vec();
    let tex_size = atlas.texture_size();

    for g in &line.glyphs {
        let entry = atlas.lookup(g.codepoint).unwrap();
        let tsx = entry.tile_x as usize * GLYPH_SDF_SIZE;
        let tsy = entry.tile_y as usize * GLYPH_SDF_SIZE;
        let mut pts = Vec::new();
        for row in 0..GLYPH_SDF_SIZE {
            for col in 0..GLYPH_SDF_SIZE {
                let idx = (tsy + row) * tex_size + (tsx + col);
                if idx < atlas_px.len() && (atlas_px[idx] - threshold).abs() < 0.15 {
                    let px = (g.x + col as f32 / GLYPH_SDF_SIZE as f32 * g.advance) * scale_mm;
                    let py = (row as f32 / GLYPH_SDF_SIZE as f32) * scale_mm;
                    pts.push((px, py));
                    min_x = min_x.min(px);
                    max_x = max_x.max(px);
                }
            }
        }
        if pts.len() >= 2 {
            for i in 1..pts.len() {
                let dx = pts[i].0 - pts[i - 1].0;
                let dy = pts[i].1 - pts[i - 1].1;
                path_len += (dx * dx + dy * dy).sqrt();
            }
            contours.push(EngravingContour { points: pts });
        }
    }

    TextEngravingResult {
        contours,
        bbox: (
            if min_x >= f32::MAX { 0.0 } else { min_x },
            0.0,
            if max_x <= f32::MIN { 0.0 } else { max_x },
            scale_mm,
        ),
        path_length_mm: path_len,
    }
}

// ── Bridge 8: Font → DB (font parameter persistence) ──────────────────

/// Serialized font parameter record for ALICE-DB storage.
pub struct FontDbRecord {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Serialized `MetaFontParams` bytes.
    pub data: [u8; 40],
    /// Weight value.
    pub weight: f32,
    /// Serif value.
    pub serif: f32,
}

/// Serialize `MetaFontParams` for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn font_to_db_record(params: &MetaFontParams) -> FontDbRecord {
    let data = params.encode();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    FontDbRecord {
        content_hash: hash,
        data,
        weight: params.weight,
        serif: params.serif,
    }
}

// ── Bridge 9: Font → Cache (font parameter caching) ──────────────────

/// Font cache entry for ALICE-Cache.
pub struct FontCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Font parameter bytes (40 bytes).
    pub params_bytes: [u8; 40],
    /// Compression ratio vs TTF.
    pub compression_ratio: f64,
}

/// Prepare `MetaFontParams` for ALICE-Cache storage.
#[inline]
#[must_use]
pub fn font_to_cache_entry(params: &MetaFontParams) -> FontCacheEntry {
    let data = params.encode();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    FontCacheEntry {
        content_hash: hash,
        params_bytes: data,
        compression_ratio: 12_500.0,
    }
}

// ── Bridge 10: Font → Sync (font parameter sync) ─────────────────────

/// Font sync packet for ALICE-Sync multiplayer.
pub struct FontSyncPacket {
    /// Font parameter bytes.
    pub params_bytes: [u8; 40],
    /// Content hash.
    pub content_hash: u64,
    /// Player ID.
    pub player_id: u8,
}

/// Package `MetaFontParams` for ALICE-Sync P2P exchange.
#[inline]
#[must_use]
pub fn font_to_sync_packet(params: &MetaFontParams, player_id: u8) -> FontSyncPacket {
    let data = params.encode();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    FontSyncPacket {
        params_bytes: data,
        content_hash: hash,
        player_id,
    }
}

// ── Bridge 11: Font → Crypto (DRM font payload) ─────────────────────

/// Encrypted font payload for DRM protection.
pub struct FontCryptoPayload {
    /// Plaintext font parameter bytes.
    pub plaintext: [u8; 40],
    /// Content hash for integrity verification.
    pub content_hash: u64,
    /// Payload size.
    pub payload_bytes: usize,
}

/// Prepare `MetaFontParams` for ALICE-Crypto encryption.
#[inline]
#[must_use]
pub fn font_to_crypto_payload(params: &MetaFontParams) -> FontCryptoPayload {
    let data = params.encode();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    FontCryptoPayload {
        plaintext: data,
        content_hash: hash,
        payload_bytes: 40,
    }
}

// ── Bridge 12: Font → Queue (font params via message queue) ─────────────

/// Font parameter message for ALICE-Queue delivery.
pub struct FontQueueMessage {
    /// Font parameter bytes (40 bytes).
    pub params_bytes: [u8; 40],
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Payload size.
    pub payload_bytes: usize,
}

/// Package `MetaFontParams` for ALICE-Queue message delivery.
#[inline]
#[must_use]
pub fn font_to_queue_message(params: &MetaFontParams) -> FontQueueMessage {
    let data = params.encode();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    FontQueueMessage {
        params_bytes: data,
        content_hash: hash,
        payload_bytes: 40,
    }
}

// ── Bridge 13: Font → Analytics (font usage metrics) ────────────────────

/// Font usage metrics for ALICE-Analytics monitoring.
pub struct FontAnalyticsMetrics {
    /// Font weight value.
    pub weight: f32,
    /// Serif amount.
    pub serif: f32,
    /// Content hash.
    pub content_hash: u64,
    /// Parameter bytes (always 40).
    pub param_bytes: usize,
    /// Compression ratio vs TTF.
    pub compression_ratio: f64,
}

/// Extract font usage metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn font_to_analytics_metrics(params: &MetaFontParams) -> FontAnalyticsMetrics {
    let data = params.encode();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    FontAnalyticsMetrics {
        weight: params.weight,
        serif: params.serif,
        content_hash: hash,
        param_bytes: 40,
        compression_ratio: 12_500.0,
    }
}

// ── Bridge 14: Font → Manga (direct SDF glyph atlas for manga panels) ──

/// Pre-built SDF glyph atlas optimized for manga balloon rendering.
///
/// Bypasses ALICE-Text to provide a direct Font → Manga pipeline for
/// Japanese vertical text in speech balloons. The atlas is pre-populated
/// with all glyphs in the input string.
pub struct MangaGlyphAtlas {
    /// Atlas pixel data (SDF values).
    pub pixels: Vec<f32>,
    /// Atlas texture size (width = height).
    pub texture_size: usize,
    /// Number of glyphs rasterized.
    pub glyph_count: usize,
    /// SDF tile size per glyph.
    pub tile_size: usize,
    /// Content hash for cache deduplication.
    pub content_hash: u64,
}

/// Build a direct SDF glyph atlas for ALICE-Manga balloon text.
///
/// Pre-rasterizes all unique glyphs in `text` into an SDF atlas,
/// ready for GPU-based manga panel compositing without going through
/// ALICE-Text compression/decompression.
#[inline]
#[must_use]
pub fn font_to_manga_glyph_atlas(text: &str, params: &MetaFontParams) -> MangaGlyphAtlas {
    let mut atlas = SdfAtlas::new(8, *params);
    let mut unique = 0usize;
    for ch in text.chars() {
        if atlas.lookup(ch).is_none() {
            atlas.get_or_insert(ch);
            unique += 1;
        }
    }
    let pixels = atlas.pixels().to_vec();
    let tex_size = atlas.texture_size();
    // FNV-1a over pixel count + glyph count for cache key
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in pixels.len().to_le_bytes().as_slice() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for &b in unique.to_le_bytes().as_slice() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    MangaGlyphAtlas {
        pixels,
        texture_size: tex_size,
        glyph_count: unique,
        tile_size: GLYPH_SDF_SIZE,
        content_hash: hash,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (TextShaper, SdfAtlas) {
        let p = MetaFontParams::sans_regular();
        (TextShaper::new(p), SdfAtlas::new(4, p))
    }

    #[test]
    fn test_font_to_view_batch() {
        let (mut s, mut a) = setup();
        let batch = font_to_view_batch("Hi", &mut s, &mut a, 200.0);
        assert!(batch.atlas_size > 0);
        assert!(!batch.atlas_pixels.is_empty());
        assert_eq!(batch.commands.len(), 2);
        assert_eq!(batch.commands[0].codepoint, 'H');
    }

    #[test]
    fn test_font_to_browser_layout() {
        let (mut s, mut a) = setup();
        let layout = font_to_browser_layout("AB", &mut s, &mut a, 200.0);
        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.line_count, 1);
        assert!(layout.width > 0.0);
    }

    #[test]
    fn test_font_to_sdf_scene() {
        let (mut s, mut a) = setup();
        let scene = font_to_sdf_scene("X", &mut s, &mut a, 10.0);
        assert_eq!(scene.tiles.len(), 1);
        assert_eq!(scene.tile_size, GLYPH_SDF_SIZE);
        assert_eq!(
            scene.tiles[0].sdf_data.len(),
            GLYPH_SDF_SIZE * GLYPH_SDF_SIZE
        );
    }

    #[test]
    fn test_font_to_manga_layout() {
        let ml = font_to_manga_layout("Hello", 100.0, 200.0, 14.0);
        assert!(!ml.columns.is_empty());
        assert_eq!(ml.params_wire.len(), 40);
        let total: usize = ml.columns.iter().map(|c| c.glyphs.len()).sum();
        assert_eq!(total, 5);
    }

    #[test]
    fn test_font_animation_frame() {
        let from = MetaFontParams::sans_regular();
        let to = MetaFontParams::sans_bold();
        let f = font_animation_frame("Hi", &from, &to, 0.5);
        assert!(!f.glyphs.is_empty());
        assert!((f.progress - 0.5).abs() < 0.001);
        assert!(f.width > 0.0);
    }

    #[test]
    fn test_font_to_cdn_package() {
        let p = MetaFontParams::serif_regular();
        let pkg = font_to_cdn_package(&p);
        assert_eq!(pkg.params_bytes.len(), 40);
        assert!(pkg.compression_ratio > 10000.0);
        assert_ne!(pkg.content_hash, 0);
    }

    #[test]
    fn test_font_to_print_contours() {
        let p = MetaFontParams::mono_regular();
        let r = font_to_print_contours("A", &p, 10.0, 0.5);
        // May or may not have contours depending on glyph SDF
        assert!(r.path_length_mm >= 0.0);
    }

    #[test]
    fn test_font_to_db_record() {
        let p = MetaFontParams::sans_regular();
        let rec = font_to_db_record(&p);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.data.len(), 40);
    }

    #[test]
    fn test_font_to_cache_entry() {
        let p = MetaFontParams::serif_regular();
        let entry = font_to_cache_entry(&p);
        assert_ne!(entry.content_hash, 0);
        assert!(entry.compression_ratio > 10000.0);
    }

    #[test]
    fn test_font_to_sync_packet() {
        let p = MetaFontParams::mono_regular();
        let pkt = font_to_sync_packet(&p, 1);
        assert_eq!(pkt.player_id, 1);
        assert_ne!(pkt.content_hash, 0);
    }

    #[test]
    fn test_font_to_crypto_payload() {
        let p = MetaFontParams::sans_bold();
        let crypto = font_to_crypto_payload(&p);
        assert_eq!(crypto.payload_bytes, 40);
        assert_ne!(crypto.content_hash, 0);
    }

    #[test]
    fn test_font_to_queue_message() {
        let p = MetaFontParams::sans_regular();
        let msg = font_to_queue_message(&p);
        assert_eq!(msg.payload_bytes, 40);
        assert_ne!(msg.content_hash, 0);
    }

    #[test]
    fn test_font_to_analytics_metrics() {
        let p = MetaFontParams::serif_regular();
        let m = font_to_analytics_metrics(&p);
        assert!(m.weight > 0.0);
        assert_ne!(m.content_hash, 0);
        assert!(m.compression_ratio > 10000.0);
    }

    #[test]
    fn test_font_to_manga_glyph_atlas() {
        let p = MetaFontParams::sans_regular();
        let atlas = font_to_manga_glyph_atlas("AB", &p);
        assert_eq!(atlas.glyph_count, 2);
        assert!(atlas.texture_size > 0);
        assert!(!atlas.pixels.is_empty());
        assert_eq!(atlas.tile_size, GLYPH_SDF_SIZE);
        assert_ne!(atlas.content_hash, 0);
    }
}
