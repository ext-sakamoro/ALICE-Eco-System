//! AR bridges — ALICE-AR ↔ DB, Cache, Analytics, Render, SLAM
//!
//! 5 bridges connecting augmented reality session processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: AR → DB (session storage) ──────────────────────────────────

/// AR session storage record for ALICE-DB persistence.
pub struct ArDbRecord {
    /// Content hash over the session metadata.
    pub content_hash: u64,
    /// Number of spatial anchors in the session.
    pub anchor_count: u32,
    /// Total mesh vertex count for the session.
    pub mesh_vertex_count: u64,
    /// Hash of the session identifier.
    pub session_hash: u64,
    /// Session duration in milliseconds.
    pub duration_ms: u64,
    /// Number of detected planes.
    pub plane_count: u32,
}

/// Serialize AR session metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn ar_to_db_record(
    anchor_count: u32,
    mesh_vertex_count: u64,
    session_hash: u64,
    duration_ms: u64,
    plane_count: u32,
) -> ArDbRecord {
    let mut buf = [0u8; 32];
    buf[0..4].copy_from_slice(&anchor_count.to_le_bytes());
    buf[4..12].copy_from_slice(&mesh_vertex_count.to_le_bytes());
    buf[12..20].copy_from_slice(&session_hash.to_le_bytes());
    buf[20..28].copy_from_slice(&duration_ms.to_le_bytes());
    buf[28..32].copy_from_slice(&plane_count.to_le_bytes());
    ArDbRecord {
        content_hash: fnv1a(&buf),
        anchor_count,
        mesh_vertex_count,
        session_hash,
        duration_ms,
        plane_count,
    }
}

// ── Bridge 2: AR → Cache (anchor cache) ──────────────────────────────────

/// AR anchor cache entry for ALICE-Cache.
pub struct ArCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of cached spatial anchors.
    pub anchor_count: u32,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Compressed mesh data size in bytes.
    pub mesh_bytes: u64,
    /// Hash of the session this entry belongs to.
    pub session_hash: u64,
}

/// Build an AR anchor cache entry for ALICE-Cache.
///
/// Sessions with anchors (anchor_count > 0) get a longer TTL (600 s) since
/// re-computing spatial anchors is expensive; empty sessions get 60 s.
#[inline]
#[must_use]
pub fn ar_to_cache_entry(anchor_count: u32, mesh_bytes: u64, session_hash: u64) -> ArCacheEntry {
    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&anchor_count.to_le_bytes());
    buf[4..12].copy_from_slice(&mesh_bytes.to_le_bytes());
    buf[12..20].copy_from_slice(&session_hash.to_le_bytes());
    let has_anchors = (anchor_count > 0) as u32;
    let ttl_secs = 60 + has_anchors * 540;
    ArCacheEntry {
        content_hash: fnv1a(&buf),
        anchor_count,
        ttl_secs,
        mesh_bytes,
        session_hash,
    }
}

// ── Bridge 3: AR → Analytics (tracking event) ────────────────────────────

/// AR tracking analytics event for ALICE-Analytics.
pub struct ArAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of active spatial anchors.
    pub anchor_count: u32,
    /// Number of tracking loss events.
    pub tracking_loss_count: u32,
    /// Rendered frames per second.
    pub fps: u32,
    /// Number of detected planes.
    pub plane_count: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an AR tracking analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn ar_to_analytics_event(
    anchor_count: u32,
    tracking_loss_count: u32,
    fps: u32,
    plane_count: u32,
    timestamp_ms: u64,
) -> ArAnalyticsEvent {
    let mut buf = [0u8; 24];
    buf[0..4].copy_from_slice(&anchor_count.to_le_bytes());
    buf[4..8].copy_from_slice(&tracking_loss_count.to_le_bytes());
    buf[8..12].copy_from_slice(&fps.to_le_bytes());
    buf[12..16].copy_from_slice(&plane_count.to_le_bytes());
    buf[16..24].copy_from_slice(&timestamp_ms.to_le_bytes());
    ArAnalyticsEvent {
        content_hash: fnv1a(&buf),
        anchor_count,
        tracking_loss_count,
        fps,
        plane_count,
        timestamp_ms,
    }
}

// ── Bridge 4: AR → Render (frame descriptor) ─────────────────────────────

/// AR render frame descriptor for ALICE-Render.
pub struct ArRenderFrame {
    /// Content hash over the frame descriptor.
    pub content_hash: u64,
    /// Number of mesh vertices submitted this frame.
    pub vertex_count: u64,
    /// Number of draw calls issued.
    pub draw_calls: u32,
    /// Time spent rendering in microseconds.
    pub render_time_us: u64,
    /// Total frame payload size in bytes.
    pub frame_bytes: u64,
}

/// Build an AR render frame descriptor for ALICE-Render.
#[inline]
#[must_use]
pub fn ar_to_render_frame(
    vertex_count: u64,
    draw_calls: u32,
    render_time_us: u64,
    frame_bytes: u64,
) -> ArRenderFrame {
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&vertex_count.to_le_bytes());
    buf[8..12].copy_from_slice(&draw_calls.to_le_bytes());
    buf[12..20].copy_from_slice(&render_time_us.to_le_bytes());
    buf[20..28].copy_from_slice(&frame_bytes.to_le_bytes());
    ArRenderFrame {
        content_hash: fnv1a(&buf),
        vertex_count,
        draw_calls,
        render_time_us,
        frame_bytes,
    }
}

// ── Bridge 5: AR → SLAM (map link) ───────────────────────────────────────

/// SLAM map link for ALICE-SLAM integration.
pub struct ArSlamLink {
    /// Content hash over the SLAM state.
    pub content_hash: u64,
    /// Number of keyframes in the SLAM map.
    pub keyframe_count: u32,
    /// Number of 3-D map points.
    pub map_point_count: u64,
    /// Number of relocalization events.
    pub relocalization_count: u32,
    /// Accumulated drift in millimetres.
    pub drift_mm: u32,
}

/// Build an AR SLAM map link for ALICE-SLAM.
#[inline]
#[must_use]
pub fn ar_to_slam_link(
    keyframe_count: u32,
    map_point_count: u64,
    relocalization_count: u32,
    drift_mm: u32,
) -> ArSlamLink {
    let mut buf = [0u8; 24];
    buf[0..4].copy_from_slice(&keyframe_count.to_le_bytes());
    buf[4..12].copy_from_slice(&map_point_count.to_le_bytes());
    buf[12..16].copy_from_slice(&relocalization_count.to_le_bytes());
    buf[16..20].copy_from_slice(&drift_mm.to_le_bytes());
    ArSlamLink {
        content_hash: fnv1a(&buf),
        keyframe_count,
        map_point_count,
        relocalization_count,
        drift_mm,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ar_to_db_record_hash_nonzero() {
        let rec = ar_to_db_record(10, 50_000, 0xabcd, 60_000, 4);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_ar_to_db_record_fields() {
        let rec = ar_to_db_record(5, 12_000, 0x1234, 30_000, 2);
        assert_eq!(rec.anchor_count, 5);
        assert_eq!(rec.mesh_vertex_count, 12_000);
        assert_eq!(rec.session_hash, 0x1234);
        assert_eq!(rec.duration_ms, 30_000);
        assert_eq!(rec.plane_count, 2);
    }

    #[test]
    fn test_ar_to_db_record_deterministic() {
        let a = ar_to_db_record(1, 2, 3, 4, 5);
        let b = ar_to_db_record(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_ar_to_cache_entry_no_anchors_ttl() {
        let entry = ar_to_cache_entry(0, 0, 0xffff);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_ar_to_cache_entry_with_anchors_ttl() {
        let entry = ar_to_cache_entry(3, 8_192, 0x5555);
        assert_eq!(entry.ttl_secs, 600);
        assert_eq!(entry.anchor_count, 3);
    }

    #[test]
    fn test_ar_to_analytics_event() {
        let ev = ar_to_analytics_event(8, 1, 60, 3, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.fps, 60);
        assert_eq!(ev.tracking_loss_count, 1);
    }

    #[test]
    fn test_ar_to_render_frame() {
        let frame = ar_to_render_frame(10_000, 25, 8_000, 2_097_152);
        assert_ne!(frame.content_hash, 0);
        assert_eq!(frame.vertex_count, 10_000);
        assert_eq!(frame.draw_calls, 25);
        assert_eq!(frame.render_time_us, 8_000);
    }

    #[test]
    fn test_ar_to_slam_link() {
        let link = ar_to_slam_link(120, 5_000, 2, 3);
        assert_ne!(link.content_hash, 0);
        assert_eq!(link.keyframe_count, 120);
        assert_eq!(link.map_point_count, 5_000);
        assert_eq!(link.drift_mm, 3);
    }
}
