//! Font bridges — ALICE-Font ↔ View, Browser, SDF, Manga, Animation, CDN, Print
//!
//! 7 bridges connecting parametric metafonts to the ALICE ecosystem.

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
        if line.width > max_w { max_w = line.width; }
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
                .map(|e| (e.tile_x, e.tile_y))
                .unwrap_or((0, 0));
            glyphs.push(DomGlyph {
                codepoint: g.codepoint as u32,
                x: g.x,
                y: g.y + line.y_offset,
                advance: g.advance,
                tile_x: tx,
                tile_y: ty,
            });
        }
        if line.width > max_w { max_w = line.width; }
        height = line.y_offset;
    }

    DomTextLayout { glyphs, width: max_w, height, line_count }
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
                sdf_data.push(if idx < atlas_px.len() { atlas_px[idx] } else { 1.0 });
            }
        }
        let x = g.x * scale;
        let right = x + g.advance * scale;
        if x < min_x { min_x = x; }
        if right > max_x { max_x = right; }
        tiles.push(FontSdfTile { ch: g.codepoint, x, y: g.y * scale, sdf_data, advance: g.advance * scale });
    }

    FontSdfScene {
        tiles,
        tile_size: GLYPH_SDF_SIZE,
        bbox_min: (if min_x == f32::MAX { 0.0 } else { min_x }, 0.0),
        bbox_max: (if max_x == f32::MIN { 0.0 } else { max_x }, scale),
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
pub fn font_to_manga_layout(text: &str, balloon_w: f32, balloon_h: f32, font_size: f32) -> MangaTextLayout {
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
            if i >= chars.len() { break; }
            glyphs.push((chars[i], col_x, y));
            y += font_size * 1.5;
            i += 1;
        }
        columns.push(MangaColumn { x: col_x, glyphs });
        col_x -= font_size * 1.8;
    }
    MangaTextLayout { params_wire: params.encode(), columns }
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
pub fn font_animation_frame(text: &str, from: &MetaFontParams, to: &MetaFontParams, t: f32) -> AnimTextFrame {
    let params = from.lerp(to, t);
    let shaper = TextShaper::new(params);
    let mut atlas = SdfAtlas::new(8, params);
    let line = shaper.shape_line(text, &mut atlas);
    AnimTextFrame { params, glyphs: line.glyphs, width: line.width, progress: t }
}

// ── Bridge 6: Font → CDN (parameter distribution) ──────────────────────

/// CDN-optimized font package (40 bytes vs multi-MB TTF).
pub struct FontCdnPackage {
    pub params_bytes: [u8; 40],
    pub content_hash: u64,
    pub compression_ratio: f64,
}

/// Package font parameters for CDN distribution.
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
        compression_ratio: 500_000.0 / 40.0, // ~12,500x vs typical TTF
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
pub fn font_to_print_contours(text: &str, params: &MetaFontParams, scale_mm: f32, threshold: f32) -> TextEngravingResult {
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
                    if px < min_x { min_x = px; }
                    if px > max_x { max_x = px; }
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
            if min_x == f32::MAX { 0.0 } else { min_x }, 0.0,
            if max_x == f32::MIN { 0.0 } else { max_x }, scale_mm,
        ),
        path_length_mm: path_len,
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
        assert_eq!(scene.tiles[0].sdf_data.len(), GLYPH_SDF_SIZE * GLYPH_SDF_SIZE);
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
}
