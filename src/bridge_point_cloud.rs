//! PointCloud bridges — ALICE-PointCloud ↔ DB, Cache, Analytics, Render, SDF
//!
//! 5 bridges connecting point cloud processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: PointCloud → DB (cloud storage) ────────────────────────────

/// Point cloud storage record for ALICE-DB persistence.
pub struct PointCloudDbRecord {
    /// Content hash over the cloud metadata.
    pub content_hash: u64,
    /// Total number of points in the cloud.
    pub point_count: u64,
    /// Number of dimensions per point (e.g. 3 for XYZ, 6 for XYZRGB).
    pub dim: u8,
    /// Hash of the axis-aligned bounding box descriptor.
    pub bounding_box_hash: u64,
    /// Hash of the data acquisition source.
    pub source_hash: u64,
    /// Precision descriptor (0=f32, 1=f64).
    pub precision: u8,
}

/// Serialize point cloud metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn point_cloud_to_db_record(
    point_count: u64,
    dim: u8,
    bounding_box_hash: u64,
    source_hash: u64,
    precision: u8,
) -> PointCloudDbRecord {
    let mut buf = [0u8; 26];
    buf[0..8].copy_from_slice(&point_count.to_le_bytes());
    buf[8] = dim;
    buf[9..17].copy_from_slice(&bounding_box_hash.to_le_bytes());
    buf[17..25].copy_from_slice(&source_hash.to_le_bytes());
    buf[25] = precision;
    PointCloudDbRecord {
        content_hash: fnv1a(&buf),
        point_count,
        dim,
        bounding_box_hash,
        source_hash,
        precision,
    }
}

// ── Bridge 2: PointCloud → Cache (LOD cache) ─────────────────────────────

/// Point cloud LOD cache entry for ALICE-Cache.
pub struct PointCloudCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of points at this LOD level.
    pub point_count: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Compressed cloud size in bytes.
    pub compressed_bytes: u64,
    /// Level-of-detail level (0 = full resolution).
    pub lod_level: u8,
}

/// Build a point cloud LOD cache entry for ALICE-Cache.
///
/// Full-resolution clouds (lod_level == 0) get a longer TTL (600 s) because
/// they are expensive to reconstruct; reduced LOD levels get 120 s.
#[inline]
#[must_use]
pub fn point_cloud_to_cache_entry(
    point_count: u64,
    compressed_bytes: u64,
    lod_level: u8,
) -> PointCloudCacheEntry {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&point_count.to_le_bytes());
    buf[8..16].copy_from_slice(&compressed_bytes.to_le_bytes());
    buf[16] = lod_level;
    let is_full_res = (lod_level == 0) as u32;
    let ttl_secs = 120 + is_full_res * 480;
    PointCloudCacheEntry {
        content_hash: fnv1a(&buf),
        point_count,
        ttl_secs,
        compressed_bytes,
        lod_level,
    }
}

// ── Bridge 3: PointCloud → Analytics (processing event) ──────────────────

/// Point cloud processing analytics event for ALICE-Analytics.
pub struct PointCloudAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of points processed.
    pub point_count: u64,
    /// Processing time in microseconds.
    pub processing_time_us: u64,
    /// Point density scaled by 1000 (points per cubic metre).
    pub density_x1000: u32,
    /// Number of outlier points removed.
    pub outlier_count: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a point cloud processing analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn point_cloud_to_analytics_event(
    point_count: u64,
    processing_time_us: u64,
    density_x1000: u32,
    outlier_count: u32,
    timestamp_ms: u64,
) -> PointCloudAnalyticsEvent {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&point_count.to_le_bytes());
    buf[8..16].copy_from_slice(&processing_time_us.to_le_bytes());
    buf[16..20].copy_from_slice(&density_x1000.to_le_bytes());
    buf[20..24].copy_from_slice(&outlier_count.to_le_bytes());
    buf[24..32].copy_from_slice(&timestamp_ms.to_le_bytes());
    PointCloudAnalyticsEvent {
        content_hash: fnv1a(&buf),
        point_count,
        processing_time_us,
        density_x1000,
        outlier_count,
        timestamp_ms,
    }
}

// ── Bridge 4: PointCloud → Render (splat frame) ──────────────────────────

/// Gaussian splat render frame for ALICE-Render.
pub struct PointCloudRenderFrame {
    /// Content hash over the render frame descriptor.
    pub content_hash: u64,
    /// Number of splats submitted.
    pub point_count: u64,
    /// Splat size scaled by 1000 (metres).
    pub splat_size_x1000: u32,
    /// Render time in microseconds.
    pub render_time_us: u64,
    /// Total frame payload size in bytes.
    pub frame_bytes: u64,
}

/// Build a Gaussian splat render frame for ALICE-Render.
#[inline]
#[must_use]
pub fn point_cloud_to_render_frame(
    point_count: u64,
    splat_size_x1000: u32,
    render_time_us: u64,
    frame_bytes: u64,
) -> PointCloudRenderFrame {
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&point_count.to_le_bytes());
    buf[8..12].copy_from_slice(&splat_size_x1000.to_le_bytes());
    buf[12..20].copy_from_slice(&render_time_us.to_le_bytes());
    buf[20..28].copy_from_slice(&frame_bytes.to_le_bytes());
    PointCloudRenderFrame {
        content_hash: fnv1a(&buf),
        point_count,
        splat_size_x1000,
        render_time_us,
        frame_bytes,
    }
}

// ── Bridge 5: PointCloud → SDF (voxel conversion) ────────────────────────

/// SDF voxel conversion descriptor for ALICE-SDF.
pub struct PointCloudSdfConversion {
    /// Content hash over the conversion descriptor.
    pub content_hash: u64,
    /// Number of input points.
    pub point_count: u64,
    /// Number of output voxels.
    pub voxel_count: u64,
    /// Voxel grid resolution (cells per axis).
    pub resolution: u32,
    /// Conversion time in microseconds.
    pub conversion_time_us: u64,
}

/// Build a point cloud to SDF voxel conversion descriptor.
#[inline]
#[must_use]
pub fn point_cloud_to_sdf_conversion(
    point_count: u64,
    voxel_count: u64,
    resolution: u32,
    conversion_time_us: u64,
) -> PointCloudSdfConversion {
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&point_count.to_le_bytes());
    buf[8..16].copy_from_slice(&voxel_count.to_le_bytes());
    buf[16..20].copy_from_slice(&resolution.to_le_bytes());
    buf[20..28].copy_from_slice(&conversion_time_us.to_le_bytes());
    PointCloudSdfConversion {
        content_hash: fnv1a(&buf),
        point_count,
        voxel_count,
        resolution,
        conversion_time_us,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_cloud_to_db_record_hash_nonzero() {
        let rec = point_cloud_to_db_record(1_000_000, 3, 0xaabb, 0xccdd, 0);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_point_cloud_to_db_record_fields() {
        let rec = point_cloud_to_db_record(500_000, 6, 0x1234, 0x5678, 1);
        assert_eq!(rec.point_count, 500_000);
        assert_eq!(rec.dim, 6);
        assert_eq!(rec.bounding_box_hash, 0x1234);
        assert_eq!(rec.source_hash, 0x5678);
        assert_eq!(rec.precision, 1);
    }

    #[test]
    fn test_point_cloud_to_db_record_deterministic() {
        let a = point_cloud_to_db_record(1, 2, 3, 4, 0);
        let b = point_cloud_to_db_record(1, 2, 3, 4, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_point_cloud_to_cache_entry_full_res_ttl() {
        let entry = point_cloud_to_cache_entry(100_000, 4_096_000, 0);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 600);
        assert_eq!(entry.lod_level, 0);
    }

    #[test]
    fn test_point_cloud_to_cache_entry_reduced_lod_ttl() {
        let entry = point_cloud_to_cache_entry(10_000, 409_600, 2);
        assert_eq!(entry.ttl_secs, 120);
        assert_eq!(entry.lod_level, 2);
    }

    #[test]
    fn test_point_cloud_to_analytics_event() {
        let ev = point_cloud_to_analytics_event(200_000, 15_000, 5_000, 200, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.point_count, 200_000);
        assert_eq!(ev.outlier_count, 200);
    }

    #[test]
    fn test_point_cloud_to_render_frame() {
        let frame = point_cloud_to_render_frame(50_000, 10, 4_000, 1_048_576);
        assert_ne!(frame.content_hash, 0);
        assert_eq!(frame.splat_size_x1000, 10);
        assert_eq!(frame.render_time_us, 4_000);
    }

    #[test]
    fn test_point_cloud_to_sdf_conversion() {
        let conv = point_cloud_to_sdf_conversion(1_000_000, 8_000_000, 200, 50_000);
        assert_ne!(conv.content_hash, 0);
        assert_eq!(conv.voxel_count, 8_000_000);
        assert_eq!(conv.resolution, 200);
        assert_eq!(conv.conversion_time_us, 50_000);
    }
}
