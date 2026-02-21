//! Text bridges — ALICE-Text ↔ Font, Manga, DB, Browser, Queue, Analytics, SDF
//!
//! 7 bridges connecting exception-based text compression to the ALICE ecosystem.

use alice_text::{compress_tuned, decompress_tuned, CompressionMode};

#[inline(always)]
fn fnv1a_text(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Text → Font (compressed text → MetaFont rendering) ────────

/// Compressed text ready for ALICE-Font rendering.
pub struct TextFontPayload {
    /// Decompressed text for rendering.
    pub text: String,
    /// Character count.
    pub char_count: usize,
    /// Original compressed size.
    pub compressed_bytes: usize,
    /// Compression ratio achieved.
    pub compression_ratio: f32,
}

/// Decompress ALICE-Text payload for ALICE-Font rendering.
#[inline]
pub fn text_to_font_payload(compressed: &[u8]) -> Option<TextFontPayload> {
    let text = decompress_tuned(compressed).ok()?;
    let char_count = text.chars().count();
    Some(TextFontPayload {
        text,
        char_count,
        compressed_bytes: compressed.len(),
        compression_ratio: if compressed.is_empty() { 0.0 } else { (char_count as f32) / compressed.len() as f32 },
    })
}

// ── Bridge 2: Text → Manga (dialogue compression → page text) ───────────

/// Compressed manga dialogue for ALICE-Manga balloon text.
pub struct TextMangaDialogue {
    /// Compressed dialogue bytes.
    pub compressed: Vec<u8>,
    /// Original text length.
    pub original_len: usize,
    /// Compressed size.
    pub compressed_len: usize,
    /// Compression ratio.
    pub compression_ratio: f32,
    /// Number of dialogue lines.
    pub line_count: usize,
}

/// Compress manga dialogue text for ALICE-Manga page embedding.
#[inline]
pub fn text_to_manga_dialogue(dialogue: &str) -> TextMangaDialogue {
    let compressed = compress_tuned(dialogue, CompressionMode::Balanced).unwrap_or_else(|_| dialogue.as_bytes().to_vec());
    let line_count = dialogue.lines().count();
    TextMangaDialogue {
        compressed_len: compressed.len(),
        original_len: dialogue.len(),
        compression_ratio: if compressed.is_empty() { 0.0 } else { dialogue.len() as f32 / compressed.len() as f32 },
        line_count,
        compressed,
    }
}

// ── Bridge 3: Text → DB (columnar log → DB storage) ────────────────────

/// Compressed log record for ALICE-DB persistence.
pub struct TextDbLogRecord {
    /// Compressed payload bytes.
    pub compressed: Vec<u8>,
    /// Original log size.
    pub original_bytes: usize,
    /// Compressed size.
    pub compressed_bytes: usize,
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Number of log entries.
    pub entry_count: usize,
}

/// Compress log batch for ALICE-DB storage.
#[inline]
pub fn text_to_db_log_batch(logs: &[&str]) -> TextDbLogRecord {
    let combined = logs.join("\n");
    let compressed = compress_tuned(&combined, CompressionMode::Balanced).unwrap_or_else(|_| combined.as_bytes().to_vec());
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &compressed {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    TextDbLogRecord {
        original_bytes: combined.len(),
        compressed_bytes: compressed.len(),
        content_hash: hash,
        entry_count: logs.len(),
        compressed,
    }
}

// ── Bridge 4: Text → Browser (exception-compressed → DOM content) ───────

/// Compressed DOM text content for ALICE-Browser.
pub struct TextBrowserContent {
    /// Compressed DOM text.
    pub compressed: Vec<u8>,
    /// Original size.
    pub original_bytes: usize,
    /// Compressed size.
    pub compressed_bytes: usize,
    /// Bandwidth savings percentage.
    pub bandwidth_saving_pct: f32,
}

/// Compress browser DOM text content via ALICE-Text.
#[inline]
pub fn text_to_browser_content(dom_text: &str) -> TextBrowserContent {
    let compressed = compress_tuned(dom_text, CompressionMode::Balanced).unwrap_or_else(|_| dom_text.as_bytes().to_vec());
    let saving = if dom_text.is_empty() { 0.0 } else { (1.0 - compressed.len() as f32 / dom_text.len() as f32) * 100.0 };
    TextBrowserContent {
        original_bytes: dom_text.len(),
        compressed_bytes: compressed.len(),
        bandwidth_saving_pct: saving.max(0.0),
        compressed,
    }
}

// ── Bridge 5: Text → Queue (compressed text messages) ────────────────────

/// Compressed text message for ALICE-Queue delivery.
pub struct TextQueueMessage {
    /// Compressed payload bytes.
    pub compressed: Vec<u8>,
    /// Original text length.
    pub original_bytes: usize,
    /// Compressed size.
    pub compressed_bytes: usize,
    /// Content hash for deduplication.
    pub content_hash: u64,
}

/// Compress text for ALICE-Queue message delivery.
#[inline]
pub fn text_to_queue_message(text: &str) -> TextQueueMessage {
    let compressed = compress_tuned(text, CompressionMode::Fast).unwrap_or_else(|_| text.as_bytes().to_vec());
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &compressed {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    TextQueueMessage {
        original_bytes: text.len(),
        compressed_bytes: compressed.len(),
        content_hash: hash,
        compressed,
    }
}

// ── Bridge 6: Text → Analytics (compression metrics) ─────────────────────

/// Text compression metrics for ALICE-Analytics monitoring.
pub struct TextAnalyticsMetrics {
    /// Original size in bytes.
    pub original_bytes: usize,
    /// Compressed size in bytes.
    pub compressed_bytes: usize,
    /// Compression ratio.
    pub compression_ratio: f32,
    /// Bandwidth savings percentage.
    pub bandwidth_saving_pct: f32,
}

/// Extract compression metrics for ALICE-Analytics.
#[inline]
pub fn text_to_analytics_metrics(text: &str) -> TextAnalyticsMetrics {
    let compressed = compress_tuned(text, CompressionMode::Balanced).unwrap_or_else(|_| text.as_bytes().to_vec());
    let ratio = if compressed.is_empty() { 0.0 } else { text.len() as f32 / compressed.len() as f32 };
    let saving = if text.is_empty() { 0.0 } else { (1.0 - compressed.len() as f32 / text.len() as f32) * 100.0 };
    TextAnalyticsMetrics {
        original_bytes: text.len(),
        compressed_bytes: compressed.len(),
        compression_ratio: ratio,
        bandwidth_saving_pct: saving.max(0.0),
    }
}

// ── Bridge 7: Text ↔ SDF (3D text rendering via SDF geometry) ────────────

/// Request to render text as 3D geometry using SDF extrusion.
///
/// Encodes text and font identity as content hashes so the SDF pipeline can
/// cache generated geometry keyed by content without re-extruding identical strings.
pub struct TextSdf3dRequest {
    /// FNV-1a hash of the compressed text content (geometry cache key).
    pub content_hash: u64,
    /// FNV-1a hash of the raw text bytes (dedup / dirty-flag key).
    pub text_hash: u64,
    /// FNV-1a hash of the font name bytes (selects SDF outline generator).
    pub font_hash: u64,
    /// Extrusion depth in millimetres (z-axis thickness of the 3D glyph).
    pub extrude_depth_mm: f32,
    /// Bevel radius in millimetres applied to the extruded edge.
    pub bevel_radius_mm: f32,
    /// Number of characters in the source text.
    pub char_count: usize,
    /// Voxel grid resolution for SDF evaluation (higher = finer detail).
    pub sdf_resolution: u32,
}

/// Build a 3D text SDF extrusion request from a text string and font descriptor.
///
/// Compresses the text with ALICE-Text to derive a stable content hash, then
/// combines it with the font and geometry parameters into a request struct
/// that the SDF pipeline can use to drive glyph extrusion.
#[inline]
pub fn text_to_sdf_3d_request(
    text: &str,
    font_name: &str,
    extrude_depth_mm: f32,
    bevel_radius_mm: f32,
    resolution: u32,
) -> TextSdf3dRequest {
    let compressed = compress_tuned(text, CompressionMode::Balanced)
        .unwrap_or_else(|_| text.as_bytes().to_vec());
    let content_hash = fnv1a_text(&compressed);
    let text_hash = fnv1a_text(text.as_bytes());
    let font_hash = fnv1a_text(font_name.as_bytes());
    let char_count = text.chars().count();

    TextSdf3dRequest {
        content_hash,
        text_hash,
        font_hash,
        extrude_depth_mm,
        bevel_radius_mm,
        char_count,
        sdf_resolution: resolution.max(8),
    }
}

/// Mesh result produced by the SDF pipeline after evaluating a 3D text request.
///
/// Returned from the SDF layer back to the Text layer so callers can query
/// mesh statistics (vertex / face counts, bounding box) without holding a
/// reference to SDF-internal data structures.
pub struct SdfTextMeshResult {
    /// FNV-1a hash matching the originating TextSdf3dRequest content_hash.
    pub content_hash: u64,
    /// FNV-1a hash of the source text bytes.
    pub text_hash: u64,
    /// Total vertex count in the generated mesh.
    pub vertex_count: usize,
    /// Total triangle face count in the generated mesh.
    pub face_count: usize,
    /// Axis-aligned bounding box extents in millimetres (x, y, z).
    pub bounding_box_mm: [f32; 3],
}

/// Build an SDF text mesh result record from mesh statistics.
///
/// `vertices` and `faces` are element counts (not byte sizes).
/// `bbox_x/y/z` are the bounding box extents in millimetres.
#[inline]
pub fn sdf_to_text_mesh_result(
    text: &str,
    vertices: usize,
    faces: usize,
    bbox_x: f32,
    bbox_y: f32,
    bbox_z: f32,
) -> SdfTextMeshResult {
    let compressed = compress_tuned(text, CompressionMode::Balanced)
        .unwrap_or_else(|_| text.as_bytes().to_vec());
    let content_hash = fnv1a_text(&compressed);
    let text_hash = fnv1a_text(text.as_bytes());

    SdfTextMeshResult {
        content_hash,
        text_hash,
        vertex_count: vertices,
        face_count: faces,
        bounding_box_mm: [bbox_x, bbox_y, bbox_z],
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_to_manga_dialogue() {
        let dialogue = "Hello world!\nHow are you?\nFine, thanks.";
        let result = text_to_manga_dialogue(dialogue);
        assert_eq!(result.line_count, 3);
        assert!(result.compressed_len > 0);
        assert!(result.original_len > 0);
    }

    #[test]
    fn test_text_to_db_log_batch() {
        let logs = vec!["2024-01-01 INFO startup", "2024-01-01 WARN timeout", "2024-01-01 ERROR crash"];
        let result = text_to_db_log_batch(&logs);
        assert_eq!(result.entry_count, 3);
        assert_ne!(result.content_hash, 0);
        assert!(result.compressed_bytes > 0);
    }

    #[test]
    fn test_text_to_browser_content() {
        let content = "The quick brown fox jumps over the lazy dog. ".repeat(100);
        let result = text_to_browser_content(&content);
        assert!(result.compressed_bytes > 0);
        assert!(result.original_bytes > 0);
    }

    #[test]
    fn test_text_to_font_roundtrip() {
        let original = "Hello, ALICE!";
        let compressed = compress_tuned(original, CompressionMode::Balanced).unwrap();
        let payload = text_to_font_payload(&compressed);
        assert!(payload.is_some());
        let p = payload.unwrap();
        assert_eq!(p.text, original);
        assert_eq!(p.char_count, 13);
    }

    #[test]
    fn test_text_to_queue_message() {
        let msg = text_to_queue_message("Hello ALICE queue!");
        assert!(msg.compressed_bytes > 0);
        assert_ne!(msg.content_hash, 0);
        assert!(msg.original_bytes > 0);
    }

    #[test]
    fn test_text_to_analytics_metrics() {
        let content = "The quick brown fox jumps over the lazy dog. ".repeat(50);
        let m = text_to_analytics_metrics(&content);
        assert!(m.original_bytes > 0);
        assert!(m.compressed_bytes > 0);
        assert!(m.compression_ratio > 0.0);
    }

    // ── Bridge 7 tests ────────────────────────────────────────────────────

    #[test]
    fn test_text_to_sdf_3d_request_basic() {
        let req = text_to_sdf_3d_request("Hello", "Noto Sans", 5.0, 0.5, 64);
        assert_ne!(req.content_hash, 0);
        assert_ne!(req.text_hash, 0);
        assert_ne!(req.font_hash, 0);
        assert_eq!(req.char_count, 5);
        assert!((req.extrude_depth_mm - 5.0).abs() < f32::EPSILON);
        assert!((req.bevel_radius_mm - 0.5).abs() < f32::EPSILON);
        assert_eq!(req.sdf_resolution, 64);
    }

    #[test]
    fn test_text_to_sdf_3d_request_resolution_floor() {
        // Resolution below 8 should be clamped to 8.
        let req = text_to_sdf_3d_request("A", "Mono", 2.0, 0.1, 0);
        assert_eq!(req.sdf_resolution, 8, "resolution below 8 must be clamped to 8");
    }

    #[test]
    fn test_text_to_sdf_3d_request_different_fonts_differ() {
        let req_a = text_to_sdf_3d_request("ABC", "Noto Sans", 3.0, 0.2, 32);
        let req_b = text_to_sdf_3d_request("ABC", "Noto Serif", 3.0, 0.2, 32);
        assert_ne!(req_a.font_hash, req_b.font_hash);
        // Content hash includes only text (not font), so text hash must be equal.
        assert_eq!(req_a.text_hash, req_b.text_hash);
    }

    #[test]
    fn test_text_to_sdf_3d_request_different_texts_differ() {
        let req_a = text_to_sdf_3d_request("Hello", "Noto Sans", 3.0, 0.2, 32);
        let req_b = text_to_sdf_3d_request("World", "Noto Sans", 3.0, 0.2, 32);
        assert_ne!(req_a.content_hash, req_b.content_hash);
        assert_ne!(req_a.text_hash, req_b.text_hash);
        // Font hash must be identical for the same font name.
        assert_eq!(req_a.font_hash, req_b.font_hash);
    }

    #[test]
    fn test_sdf_to_text_mesh_result_basic() {
        let result = sdf_to_text_mesh_result("ALICE", 1024, 512, 30.0, 10.0, 5.0);
        assert_ne!(result.content_hash, 0);
        assert_ne!(result.text_hash, 0);
        assert_eq!(result.vertex_count, 1024);
        assert_eq!(result.face_count, 512);
        assert!((result.bounding_box_mm[0] - 30.0).abs() < f32::EPSILON);
        assert!((result.bounding_box_mm[1] - 10.0).abs() < f32::EPSILON);
        assert!((result.bounding_box_mm[2] - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sdf_to_text_mesh_result_hash_matches_request() {
        // The content_hash of the mesh result must match the request derived from
        // the same text string so the caller can correlate request ↔ result.
        let text = "ALICE 3D";
        let req = text_to_sdf_3d_request(text, "Gothic", 4.0, 0.3, 128);
        let result = sdf_to_text_mesh_result(text, 2048, 1024, 50.0, 15.0, 4.0);
        assert_eq!(req.content_hash, result.content_hash,
            "request and result must share the same content_hash for the same text");
        assert_eq!(req.text_hash, result.text_hash);
    }
}
