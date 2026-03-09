//! Render bridges — ALICE-Render ↔ Cache, DB, Analytics, SDF, CDN
//!
//! 5 bridges connecting render pipeline outputs (extracted as primitives)
//! to the ALICE ecosystem. No external crate types are imported; all fields
//! use primitive types derived from serialised render frame data.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Render → Cache (frame buffer caching) ───────────────────────

/// Render frame buffer cache entry for ALICE-Cache.
pub struct RenderCacheEntry {
    /// Content hash over frame_id and pixel_count.
    pub content_hash: u64,
    /// Opaque frame identifier hash.
    pub frame_id_hash: u64,
    /// Render output width in pixels.
    pub width: u32,
    /// Render output height in pixels.
    pub height: u32,
    /// Total pixel count (width × height).
    pub pixel_count: u64,
    /// Frames per second achieved in this render pass.
    pub fps: f32,
    /// TTL in milliseconds (branchless: shorter for high-res frames).
    pub ttl_ms: u32,
    /// Unix timestamp in nanoseconds when the frame was rendered.
    pub rendered_at_ns: u64,
}

/// Build a cache entry for a rendered frame buffer.
///
/// TTL is 200 ms by default; reduced to 33 ms (≈1 frame at 30 fps) when
/// `pixel_count` exceeds `hires_threshold` (large frames should not be
/// stale for long).
#[inline]
#[must_use]
pub fn render_to_cache_entry(
    frame_id: &[u8],
    width: u32,
    height: u32,
    fps: f32,
    hires_threshold: u64,
    rendered_at_ns: u64,
) -> RenderCacheEntry {
    let pixel_count = u64::from(width) * u64::from(height);
    let frame_id_hash = fnv1a(frame_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&frame_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&pixel_count.to_le_bytes());
    // Branchless TTL: 200 - hires * 167
    let hires = (pixel_count > hires_threshold) as u32;
    let ttl_ms = 200 - hires * 167;
    RenderCacheEntry {
        content_hash: fnv1a(&buf),
        frame_id_hash,
        width,
        height,
        pixel_count,
        fps,
        ttl_ms,
        rendered_at_ns,
    }
}

// ── Bridge 2: Render → DB (frame statistics persistence) ──────────────────

/// Render frame statistics record for ALICE-DB persistence.
pub struct RenderDbRecord {
    /// Content hash over frame_id_hash and triangle_count.
    pub content_hash: u64,
    /// Hashed frame identifier.
    pub frame_id_hash: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Total triangle count submitted to the render pipeline.
    pub triangle_count: u64,
    /// Total ray count (for ray-tracing passes; 0 if rasterisation only).
    pub ray_count: u64,
    /// Frames per second.
    pub fps: f32,
    /// Render pass duration in microseconds.
    pub duration_us: u64,
    /// Unix timestamp in nanoseconds.
    pub rendered_at_ns: u64,
}

/// Build a DB persistence record from render frame statistics.
#[inline]
#[must_use]
pub fn render_to_db_record(
    frame_id: &[u8],
    width: u32,
    height: u32,
    triangle_count: u64,
    ray_count: u64,
    fps: f32,
    duration_us: u64,
    rendered_at_ns: u64,
) -> RenderDbRecord {
    let frame_id_hash = fnv1a(frame_id);
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&frame_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&triangle_count.to_le_bytes());
    buf[16..24].copy_from_slice(&ray_count.to_le_bytes());
    RenderDbRecord {
        content_hash: fnv1a(&buf),
        frame_id_hash,
        width,
        height,
        triangle_count,
        ray_count,
        fps,
        duration_us,
        rendered_at_ns,
    }
}

// ── Bridge 3: Render → Analytics (performance metrics ingestion) ───────────

/// Render performance metrics event for ALICE-Analytics ingestion.
pub struct RenderAnalyticsEvent {
    /// Content hash over frame_id_hash and duration_us.
    pub content_hash: u64,
    /// Hashed frame identifier.
    pub frame_id_hash: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Triangle count.
    pub triangle_count: u64,
    /// Ray count.
    pub ray_count: u64,
    /// Frames per second.
    pub fps: f32,
    /// Render duration in microseconds.
    pub duration_us: u64,
    /// Triangles rendered per microsecond (throughput metric).
    pub triangles_per_us: f64,
}

/// Build an analytics event from render frame performance data.
#[inline]
#[must_use]
pub fn render_to_analytics_event(
    frame_id: &[u8],
    width: u32,
    height: u32,
    triangle_count: u64,
    ray_count: u64,
    fps: f32,
    duration_us: u64,
) -> RenderAnalyticsEvent {
    let frame_id_hash = fnv1a(frame_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&frame_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&duration_us.to_le_bytes());
    let triangles_per_us = triangle_count as f64 / duration_us.max(1) as f64;
    RenderAnalyticsEvent {
        content_hash: fnv1a(&buf),
        frame_id_hash,
        width,
        height,
        triangle_count,
        ray_count,
        fps,
        duration_us,
        triangles_per_us,
    }
}

// ── Bridge 4: Render → SDF (geometry feed for SDF evaluation) ─────────────

/// SDF geometry feed extracted from the render scene for ALICE-SDF.
pub struct RenderSdfFeed {
    /// Content hash over scene_id and triangle_count.
    pub content_hash: u64,
    /// Opaque scene identifier hash.
    pub scene_id_hash: u64,
    /// Triangle count in the scene.
    pub triangle_count: u64,
    /// Axis-aligned bounding box minimum X.
    pub aabb_min_x: f64,
    /// Axis-aligned bounding box minimum Y.
    pub aabb_min_y: f64,
    /// Axis-aligned bounding box minimum Z.
    pub aabb_min_z: f64,
    /// Axis-aligned bounding box maximum X.
    pub aabb_max_x: f64,
    /// Axis-aligned bounding box maximum Y.
    pub aabb_max_y: f64,
    /// Axis-aligned bounding box maximum Z.
    pub aabb_max_z: f64,
    /// Desired SDF voxel resolution (cells per unit length).
    pub voxel_resolution: u32,
}

/// Build an SDF geometry feed from render scene data.
#[inline]
#[must_use]
pub fn render_to_sdf_feed(
    scene_id: &[u8],
    triangle_count: u64,
    aabb_min_x: f64,
    aabb_min_y: f64,
    aabb_min_z: f64,
    aabb_max_x: f64,
    aabb_max_y: f64,
    aabb_max_z: f64,
    voxel_resolution: u32,
) -> RenderSdfFeed {
    let scene_id_hash = fnv1a(scene_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&scene_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&triangle_count.to_le_bytes());
    RenderSdfFeed {
        content_hash: fnv1a(&buf),
        scene_id_hash,
        triangle_count,
        aabb_min_x,
        aabb_min_y,
        aabb_min_z,
        aabb_max_x,
        aabb_max_y,
        aabb_max_z,
        voxel_resolution,
    }
}

// ── Bridge 5: Render → CDN (rendered asset delivery descriptor) ────────────

/// Rendered asset delivery descriptor for ALICE-CDN.
pub struct RenderCdnDescriptor {
    /// Content hash over asset_id and pixel_count.
    pub content_hash: u64,
    /// Opaque asset identifier hash.
    pub asset_id_hash: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Total pixel count.
    pub pixel_count: u64,
    /// Encoded asset size in bytes.
    pub encoded_bytes: u64,
    /// MIME type tag (0 = image/png, 1 = image/webp, 2 = video/mp4).
    pub mime_type_tag: u8,
    /// Content-addressed cache key (re-uses content_hash as hex representation).
    pub cache_key: u64,
    /// Time-to-live for CDN edge nodes in seconds.
    pub cdn_ttl_secs: u32,
}

/// Build a CDN delivery descriptor for a rendered asset.
///
/// `cdn_ttl_secs` defaults to 86400 (1 day); halved when `pixel_count`
/// exceeds `hires_threshold` (large assets may need faster invalidation).
#[inline]
#[must_use]
pub fn render_to_cdn_descriptor(
    asset_id: &[u8],
    width: u32,
    height: u32,
    encoded_bytes: u64,
    mime_type_tag: u8,
    hires_threshold: u64,
) -> RenderCdnDescriptor {
    let pixel_count = u64::from(width) * u64::from(height);
    let asset_id_hash = fnv1a(asset_id);
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&asset_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&pixel_count.to_le_bytes());
    buf[16] = mime_type_tag;
    let content_hash = fnv1a(&buf);
    // Branchless CDN TTL: 86400 - hires * 43200
    let hires = (pixel_count > hires_threshold) as u32;
    let cdn_ttl_secs = 86400 - hires * 43200;
    RenderCdnDescriptor {
        content_hash,
        asset_id_hash,
        width,
        height,
        pixel_count,
        encoded_bytes,
        mime_type_tag,
        cache_key: content_hash,
        cdn_ttl_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_lowres_long_ttl() {
        // 1920×1080 = 2_073_600 pixels; threshold 4M → low-res
        let e = render_to_cache_entry(b"f1", 1920, 1080, 30.0, 4_000_000, 0);
        assert_eq!(e.ttl_ms, 200);
        assert_eq!(e.pixel_count, 1920 * 1080);
    }

    #[test]
    fn cache_entry_hires_short_ttl() {
        // 3840×2160 = 8_294_400 pixels; threshold 4M → hi-res
        let e = render_to_cache_entry(b"f2", 3840, 2160, 60.0, 4_000_000, 0);
        assert_eq!(e.ttl_ms, 33);
    }

    #[test]
    fn cache_entry_hash_nonzero() {
        let e = render_to_cache_entry(b"frame-x", 800, 600, 24.0, 1_000_000, 5_000);
        assert_ne!(e.content_hash, 0);
        assert_ne!(e.frame_id_hash, 0);
    }

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_fields_preserved() {
        let rec = render_to_db_record(b"fr", 1280, 720, 500_000, 0, 60.0, 16_000, 9_999);
        assert_eq!(rec.width, 1280);
        assert_eq!(rec.height, 720);
        assert_eq!(rec.triangle_count, 500_000);
        assert_eq!(rec.ray_count, 0);
        assert_ne!(rec.content_hash, 0);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_triangles_per_us() {
        // 1_000_000 triangles / 1_000 us = 1000.0 tri/us
        let ev = render_to_analytics_event(b"a", 1920, 1080, 1_000_000, 0, 60.0, 1_000);
        assert!((ev.triangles_per_us - 1000.0).abs() < 1e-9);
        assert_ne!(ev.content_hash, 0);
    }

    // ── SDF feed tests ────────────────────────────────────────────────────

    #[test]
    fn sdf_feed_hash_and_fields() {
        let feed = render_to_sdf_feed(
            b"scene1", 200_000, -10.0, -10.0, -10.0, 10.0, 10.0, 10.0, 64,
        );
        assert_ne!(feed.content_hash, 0);
        assert_eq!(feed.voxel_resolution, 64);
        assert!((feed.aabb_max_x - 10.0).abs() < f64::EPSILON);
    }

    // ── CDN descriptor tests ──────────────────────────────────────────────

    #[test]
    fn cdn_descriptor_lowres_full_ttl() {
        let d = render_to_cdn_descriptor(b"img1", 800, 600, 120_000, 0, 4_000_000);
        assert_eq!(d.cdn_ttl_secs, 86400);
        assert_eq!(d.mime_type_tag, 0);
    }

    #[test]
    fn cdn_descriptor_hires_half_ttl() {
        let d = render_to_cdn_descriptor(b"img2", 3840, 2160, 5_000_000, 1, 4_000_000);
        assert_eq!(d.cdn_ttl_secs, 43200);
        // cache_key == content_hash
        assert_eq!(d.cache_key, d.content_hash);
    }
}
