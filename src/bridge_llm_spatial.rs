//! LLM spatial bridges — ALICE-LLM ↔ SDF, PointCloud, Render, Vision, Image
//!
//! 5 bridges connecting LLM inference to 3D/spatial/visual systems.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LLM → SDF (text-to-3D generation) ────────────────────────

/// SDF generation request from LLM text description.
pub struct LlmSdfRequest {
    /// Content hash over the SDF request.
    pub content_hash: u64,
    /// Prompt embedding dimension.
    pub embed_dim: u32,
    /// Target SDF grid resolution (per axis).
    pub grid_resolution: u32,
    /// Number of SDF primitives to compose.
    pub primitive_count: u32,
    /// Estimated voxel count (resolution^3).
    pub estimated_voxels: u64,
    /// Whether smooth blending is requested.
    pub smooth_blend: bool,
}

/// Build an SDF generation request from LLM text-to-3D output.
///
/// Voxel count: resolution^3 for the evaluation grid.
#[inline]
#[must_use]
pub fn llm_to_sdf_request(
    embed_dim: u32,
    grid_resolution: u32,
    primitive_count: u32,
    smooth_blend: bool,
) -> LlmSdfRequest {
    let mut buf = [0u8; 13];
    buf[0..4].copy_from_slice(&embed_dim.to_le_bytes());
    buf[4..8].copy_from_slice(&grid_resolution.to_le_bytes());
    buf[8..12].copy_from_slice(&primitive_count.to_le_bytes());
    buf[12] = smooth_blend as u8;
    let estimated_voxels = (grid_resolution as u64)
        .saturating_mul(grid_resolution as u64)
        .saturating_mul(grid_resolution as u64);
    LlmSdfRequest {
        content_hash: fnv1a(&buf),
        embed_dim,
        grid_resolution,
        primitive_count,
        estimated_voxels,
        smooth_blend,
    }
}

// ── Bridge 2: LLM → PointCloud (3D annotation) ─────────────────────────

/// Point cloud annotation request from LLM semantic analysis.
pub struct LlmPointCloudAnnotation {
    /// Content hash over the annotation request.
    pub content_hash: u64,
    /// Number of points in the cloud.
    pub point_count: u64,
    /// Number of semantic classes to assign.
    pub class_count: u32,
    /// Embedding dimension for per-point features.
    pub feature_dim: u32,
    /// Estimated annotation memory in bytes.
    pub memory_bytes: u64,
    /// Whether 3D bounding boxes are included.
    pub has_bbox: bool,
}

/// Build a point cloud annotation request from LLM semantic output.
///
/// Memory: point_count * (class_count bytes for labels + feature_dim * 4 for embeddings).
#[inline]
#[must_use]
pub fn llm_to_pointcloud_annotation(
    point_count: u64,
    class_count: u32,
    feature_dim: u32,
    has_bbox: bool,
) -> LlmPointCloudAnnotation {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&point_count.to_le_bytes());
    buf[8..12].copy_from_slice(&class_count.to_le_bytes());
    buf[12..16].copy_from_slice(&feature_dim.to_le_bytes());
    buf[16] = has_bbox as u8;
    let memory_bytes = point_count * (class_count as u64 + feature_dim as u64 * 4);
    LlmPointCloudAnnotation {
        content_hash: fnv1a(&buf),
        point_count,
        class_count,
        feature_dim,
        memory_bytes,
        has_bbox,
    }
}

// ── Bridge 3: LLM → Render (LLM-guided rendering) ──────────────────────

/// Render directive from LLM scene description.
pub struct LlmRenderDirective {
    /// Content hash over the render directive.
    pub content_hash: u64,
    /// Scene complexity score (0–100).
    pub complexity: u8,
    /// Target resolution width.
    pub width: u32,
    /// Target resolution height.
    pub height: u32,
    /// Samples per pixel for path tracing.
    pub spp: u32,
    /// Estimated render time in milliseconds.
    pub estimated_render_ms: u64,
}

/// Build a render directive from LLM scene analysis.
///
/// Render time estimate: width * height * spp * complexity * 0.001 ms.
#[inline]
#[must_use]
pub fn llm_to_render_directive(
    complexity: u8,
    width: u32,
    height: u32,
    spp: u32,
) -> LlmRenderDirective {
    let mut buf = [0u8; 13];
    buf[0] = complexity;
    buf[1..5].copy_from_slice(&width.to_le_bytes());
    buf[5..9].copy_from_slice(&height.to_le_bytes());
    buf[9..13].copy_from_slice(&spp.to_le_bytes());
    let pixels = width as u64 * height as u64;
    let estimated_render_ms = pixels * spp as u64 * complexity as u64 / 1000;
    LlmRenderDirective {
        content_hash: fnv1a(&buf),
        complexity,
        width,
        height,
        spp,
        estimated_render_ms,
    }
}

// ── Bridge 4: LLM → Vision (multimodal vision-language) ────────────────

/// Vision-language input descriptor for multimodal LLM.
pub struct LlmVisionInput {
    /// Content hash over the vision input.
    pub content_hash: u64,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Number of image patches (for ViT-style encoding).
    pub patch_count: u32,
    /// Patch embedding dimension.
    pub patch_dim: u32,
    /// Total visual tokens injected into LLM context.
    pub visual_tokens: u32,
}

/// Build a vision input descriptor from image metadata.
///
/// Patch count: (width/patch_size) * (height/patch_size) where patch_size=14 (ViT standard).
#[inline]
#[must_use]
pub fn llm_to_vision_input(
    width: u32,
    height: u32,
    patch_dim: u32,
) -> LlmVisionInput {
    let patch_size = 14u32;
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&width.to_le_bytes());
    buf[4..8].copy_from_slice(&height.to_le_bytes());
    buf[8..12].copy_from_slice(&patch_dim.to_le_bytes());
    let patches_w = (width + patch_size - 1) / patch_size;
    let patches_h = (height + patch_size - 1) / patch_size;
    let patch_count = patches_w * patches_h;
    let visual_tokens = patch_count + 1; // +1 for CLS token
    LlmVisionInput {
        content_hash: fnv1a(&buf),
        width,
        height,
        patch_count,
        patch_dim,
        visual_tokens,
    }
}

// ── Bridge 5: LLM → Image (image understanding) ────────────────────────

/// Image captioning/analysis result from LLM.
pub struct LlmImageAnalysis {
    /// Content hash over the analysis result.
    pub content_hash: u64,
    /// Image identifier hash.
    pub image_hash: u64,
    /// Caption token count.
    pub caption_tokens: u32,
    /// Number of detected objects/regions.
    pub object_count: u32,
    /// Analysis confidence (0.0–1.0).
    pub confidence: f32,
    /// Whether OCR text was extracted.
    pub has_ocr: bool,
}

/// Build an image analysis result from LLM multimodal output.
#[inline]
#[must_use]
pub fn llm_to_image_analysis(
    image_hash: u64,
    caption_tokens: u32,
    object_count: u32,
    confidence: f32,
    has_ocr: bool,
) -> LlmImageAnalysis {
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&image_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&caption_tokens.to_le_bytes());
    buf[12..16].copy_from_slice(&object_count.to_le_bytes());
    buf[16..20].copy_from_slice(&confidence.to_bits().to_le_bytes());
    buf[20] = has_ocr as u8;
    LlmImageAnalysis {
        content_hash: fnv1a(&buf),
        image_hash,
        caption_tokens,
        object_count,
        confidence,
        has_ocr,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdf_request_voxels() {
        let s = llm_to_sdf_request(2048, 256, 12, true);
        assert_ne!(s.content_hash, 0);
        assert_eq!(s.estimated_voxels, 256 * 256 * 256); // 16M
        assert!(s.smooth_blend);
    }

    #[test]
    fn test_sdf_request_determinism() {
        let a = llm_to_sdf_request(768, 128, 5, false);
        let b = llm_to_sdf_request(768, 128, 5, false);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_pointcloud_annotation_memory() {
        // 1M points, 20 classes, 256-dim features
        let p = llm_to_pointcloud_annotation(1_000_000, 20, 256, true);
        assert_ne!(p.content_hash, 0);
        // 1M * (20 + 256*4) = 1M * 1044 = 1,044,000,000
        assert_eq!(p.memory_bytes, 1_044_000_000);
        assert!(p.has_bbox);
    }

    #[test]
    fn test_render_directive_time() {
        let r = llm_to_render_directive(50, 1920, 1080, 64);
        assert_ne!(r.content_hash, 0);
        // 1920*1080 * 64 * 50 / 1000 = 6,635,520
        assert_eq!(r.estimated_render_ms, 6_635_520);
    }

    #[test]
    fn test_vision_input_patches() {
        // 224x224 image, patch_size=14 → 16x16=256 patches + 1 CLS = 257
        let v = llm_to_vision_input(224, 224, 768);
        assert_ne!(v.content_hash, 0);
        assert_eq!(v.patch_count, 256);
        assert_eq!(v.visual_tokens, 257);
    }

    #[test]
    fn test_vision_input_non_square() {
        // 336x224 → 24*16=384 patches + 1 = 385
        let v = llm_to_vision_input(336, 224, 1024);
        assert_eq!(v.patch_count, 384);
        assert_eq!(v.visual_tokens, 385);
    }

    #[test]
    fn test_image_analysis_with_ocr() {
        let a = llm_to_image_analysis(0xdead, 64, 5, 0.92, true);
        assert_ne!(a.content_hash, 0);
        assert!(a.has_ocr);
        assert!((a.confidence - 0.92).abs() < 0.01);
    }

    #[test]
    fn test_image_analysis_no_objects() {
        let a = llm_to_image_analysis(0xbeef, 32, 0, 0.85, false);
        assert_eq!(a.object_count, 0);
        assert!(!a.has_ocr);
    }
}
