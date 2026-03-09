//! VR bridges — ALICE-VR ↔ DB, Cache, Analytics, Render, Physics
//!
//! 5 bridges connecting the VR runtime layer to the ALICE ecosystem.
//! Covers session-data persistence in DB, pose caching, comfort metrics
//! in Analytics, stereo rendering descriptors, and physics interaction.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: VR → DB (session log) ──────────────────────────────────────

/// VR session log entry for ALICE-DB persistence.
///
/// Written at session start and end so the database layer can track
/// headset usage time, comfort metrics, and per-user session history.
pub struct VrDbSessionLog {
    /// FNV-1a hash of session_id bytes.
    pub content_hash: u64,
    /// Opaque 64-bit session identifier.
    pub session_id: u64,
    /// Session duration in seconds.
    pub duration_secs: u32,
    /// HMD model identifier: 0=unknown, 1=Quest3, 2=PSVR2, 3=Index, 4=Pico4.
    pub hmd_model: u8,
    /// Interpupillary distance in millimetres (scaled * 10, e.g. 632 = 63.2 mm).
    pub ipd_mm_x10: u16,
    /// Display refresh rate in Hz.
    pub refresh_rate: u16,
    /// True when the session ended cleanly (no crash or forced quit).
    pub clean_exit: bool,
}

/// Build a VR session log entry for ALICE-DB.
#[inline]
#[must_use]
pub fn vr_to_db_session_log(
    session_id: u64,
    duration_secs: u32,
    hmd_model: u8,
    ipd_mm_x10: u16,
    refresh_rate: u16,
    clean_exit: bool,
) -> VrDbSessionLog {
    let content_hash = fnv1a(&session_id.to_le_bytes());
    VrDbSessionLog {
        content_hash,
        session_id,
        duration_secs,
        hmd_model: hmd_model.min(4),
        ipd_mm_x10,
        refresh_rate,
        clean_exit,
    }
}

// ── Bridge 2: VR → Cache (pose cache) ────────────────────────────────────

/// Head pose cache entry for ALICE-Cache.
///
/// Caches the most recent HMD pose (position + orientation) keyed by
/// session_id so rendering and physics modules can read the latest pose
/// without querying the tracking runtime on every frame.
/// Fast-moving sessions (frame_time_ms < 12) receive a shorter TTL.
pub struct VrCachePoseEntry {
    /// FNV-1a hash of session_id bytes — cache key.
    pub content_hash: u64,
    /// Session identifier.
    pub session_id: u64,
    /// HMD position X in metres (fixed-point * 1000, i.e. millimetres).
    pub hmd_position_x_mm: i32,
    /// HMD position Y in metres (fixed-point * 1000).
    pub hmd_position_y_mm: i32,
    /// HMD position Z in metres (fixed-point * 1000).
    pub hmd_position_z_mm: i32,
    /// Orientation quaternion W component (fixed-point * 10000).
    pub rotation_w_x10k: i32,
    /// Orientation quaternion X component (fixed-point * 10000).
    pub rotation_x_x10k: i32,
    /// Orientation quaternion Y component (fixed-point * 10000).
    pub rotation_y_x10k: i32,
    /// Orientation quaternion Z component (fixed-point * 10000).
    pub rotation_z_x10k: i32,
    /// Cache TTL in seconds: 1 for fast motion (frame_time_ms < 12), else 5.
    pub ttl_secs: u32,
}

/// Build a head pose cache entry for ALICE-Cache.
///
/// Position and orientation are passed as floating-point and converted to
/// fixed-point integers for lossless hashing and compact storage.
/// TTL is computed branchlessly: fast motion → 1 s, normal → 5 s.
#[inline]
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn vr_to_cache_pose_entry(
    session_id: u64,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    rot_w: f32,
    rot_x: f32,
    rot_y: f32,
    rot_z: f32,
    frame_time_ms: f32,
) -> VrCachePoseEntry {
    let content_hash = fnv1a(&session_id.to_le_bytes());
    // ブランチレス TTL: 高速動作 (frame_time_ms < 12) → 1s, 通常 → 5s
    let fast = (frame_time_ms < 12.0) as u32;
    let ttl_secs = 5 - fast * 4;
    VrCachePoseEntry {
        content_hash,
        session_id,
        hmd_position_x_mm: (pos_x * 1000.0) as i32,
        hmd_position_y_mm: (pos_y * 1000.0) as i32,
        hmd_position_z_mm: (pos_z * 1000.0) as i32,
        rotation_w_x10k: (rot_w * 10_000.0) as i32,
        rotation_x_x10k: (rot_x * 10_000.0) as i32,
        rotation_y_x10k: (rot_y * 10_000.0) as i32,
        rotation_z_x10k: (rot_z * 10_000.0) as i32,
        ttl_secs,
    }
}

// ── Bridge 3: VR → Analytics (comfort metrics) ───────────────────────────

/// Comfort metrics event for ALICE-Analytics.
///
/// Emitted periodically during a VR session so the analytics layer can
/// track reprojection rates, motion-to-photon latency, and comfort scores
/// for device-quality monitoring and per-user comfort trend analysis.
pub struct VrAnalyticsComfortEvent {
    /// FNV-1a hash of session_id and frame_index bytes.
    pub content_hash: u64,
    /// Session identifier.
    pub session_id: u64,
    /// Frame counter within the session.
    pub frame_index: u64,
    /// Frame render time in microseconds.
    pub frame_time_us: u32,
    /// Motion-to-photon latency in microseconds.
    pub m2p_latency_us: u32,
    /// Number of reprojected frames in the last second.
    pub reprojection_count: u16,
    /// Comfort score 0–100 (100=no discomfort indicators).
    pub comfort_score: u8,
    /// True when asynchronous spacewarp / reprojection was active this frame.
    pub asw_active: bool,
}

/// Build a comfort metrics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn vr_to_analytics_comfort_event(
    session_id: u64,
    frame_index: u64,
    frame_time_us: u32,
    m2p_latency_us: u32,
    reprojection_count: u16,
    comfort_score: u8,
    asw_active: bool,
) -> VrAnalyticsComfortEvent {
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&session_id.to_le_bytes());
    key[8..16].copy_from_slice(&frame_index.to_le_bytes());
    VrAnalyticsComfortEvent {
        content_hash: fnv1a(&key),
        session_id,
        frame_index,
        frame_time_us,
        m2p_latency_us,
        reprojection_count,
        comfort_score: comfort_score.min(100),
        asw_active,
    }
}

// ── Bridge 4: VR → Render (stereo rendering descriptor) ──────────────────

/// Stereo rendering descriptor for ALICE-Render.
///
/// Packages per-eye projection parameters and foveation hints so the
/// render pipeline can configure stereo projection matrices and
/// fixed-foveated rendering regions without re-querying the HMD runtime.
pub struct VrRenderStereoDesc {
    /// FNV-1a hash of session_id, eye, and fov bytes.
    pub content_hash: u64,
    /// Session identifier.
    pub session_id: u64,
    /// Eye index: 0=left, 1=right.
    pub eye: u8,
    /// Render target width in pixels.
    pub render_width: u16,
    /// Render target height in pixels.
    pub render_height: u16,
    /// Horizontal field of view in tenths of a degree (e.g. 1100 = 110.0°).
    pub fov_h_deci_deg: u16,
    /// Vertical field of view in tenths of a degree.
    pub fov_v_deci_deg: u16,
    /// IPD in millimetres (scaled * 10).
    pub ipd_mm_x10: u16,
    /// Fixed-foveated rendering level: 0=off, 1=low, 2=medium, 3=high.
    pub ffr_level: u8,
}

/// Build a stereo rendering descriptor for ALICE-Render.
#[inline]
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn vr_to_render_stereo_desc(
    session_id: u64,
    eye: u8,
    render_width: u16,
    render_height: u16,
    fov_h_deci_deg: u16,
    fov_v_deci_deg: u16,
    ipd_mm_x10: u16,
    ffr_level: u8,
) -> VrRenderStereoDesc {
    let eye_clamped = eye.min(1);
    let ffr = ffr_level.min(3);
    let mut key = [0u8; 13];
    key[0..8].copy_from_slice(&session_id.to_le_bytes());
    key[8] = eye_clamped;
    key[9..11].copy_from_slice(&fov_h_deci_deg.to_le_bytes());
    key[11..13].copy_from_slice(&fov_v_deci_deg.to_le_bytes());
    VrRenderStereoDesc {
        content_hash: fnv1a(&key),
        session_id,
        eye: eye_clamped,
        render_width,
        render_height,
        fov_h_deci_deg,
        fov_v_deci_deg,
        ipd_mm_x10,
        ffr_level: ffr,
    }
}

// ── Bridge 5: VR → Physics (interaction descriptor) ──────────────────────

/// Physics interaction descriptor for ALICE-Physics.
///
/// Describes a controller or hand-tracking interaction event so the
/// physics engine can apply forces and torques to scene objects in
/// response to VR input gestures and collisions.
pub struct VrPhysicsInteraction {
    /// FNV-1a hash of session_id, controller_id, and event_type bytes.
    pub content_hash: u64,
    /// Session identifier.
    pub session_id: u64,
    /// Controller index: 0=left hand, 1=right hand.
    pub controller_id: u8,
    /// Interaction event type: 0=grab, 1=release, 2=throw, 3=poke, 4=pinch.
    pub event_type: u8,
    /// Linear velocity X at interaction point (fixed-point mm/s).
    pub velocity_x_mm_s: i32,
    /// Linear velocity Y at interaction point (fixed-point mm/s).
    pub velocity_y_mm_s: i32,
    /// Linear velocity Z at interaction point (fixed-point mm/s).
    pub velocity_z_mm_s: i32,
    /// Applied force magnitude in millinewtons.
    pub force_mn: u32,
    /// True when haptic feedback was triggered for this interaction.
    pub haptic_triggered: bool,
}

/// Build a physics interaction descriptor for ALICE-Physics.
///
/// Velocities are passed as floating-point m/s and converted to mm/s
/// fixed-point integers for deterministic hashing.
#[inline]
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn vr_to_physics_interaction(
    session_id: u64,
    controller_id: u8,
    event_type: u8,
    vel_x_m_s: f32,
    vel_y_m_s: f32,
    vel_z_m_s: f32,
    force_mn: u32,
    haptic_triggered: bool,
) -> VrPhysicsInteraction {
    let ctrl = controller_id.min(1);
    let evt = event_type.min(4);
    let mut key = [0u8; 10];
    key[0..8].copy_from_slice(&session_id.to_le_bytes());
    key[8] = ctrl;
    key[9] = evt;
    VrPhysicsInteraction {
        content_hash: fnv1a(&key),
        session_id,
        controller_id: ctrl,
        event_type: evt,
        velocity_x_mm_s: (vel_x_m_s * 1000.0) as i32,
        velocity_y_mm_s: (vel_y_m_s * 1000.0) as i32,
        velocity_z_mm_s: (vel_z_m_s * 1000.0) as i32,
        force_mn,
        haptic_triggered,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION_A: u64 = 0xDEAD_BEEF_CAFE_1234;
    const SESSION_B: u64 = 0x0000_0000_0000_0001;

    // Bridge 1 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_vr_to_db_session_log_basic() {
        let log = vr_to_db_session_log(SESSION_A, 3600, 1, 632, 120, true);
        assert_ne!(log.content_hash, 0);
        assert_eq!(log.session_id, SESSION_A);
        assert_eq!(log.hmd_model, 1);
        assert_eq!(log.ipd_mm_x10, 632);
        assert_eq!(log.refresh_rate, 120);
        assert!(log.clean_exit);
    }

    #[test]
    fn test_vr_to_db_session_log_model_clamped() {
        let log = vr_to_db_session_log(SESSION_B, 60, 99, 640, 90, false);
        // hmd_model が 4 に丸められること
        assert_eq!(log.hmd_model, 4);
        assert!(!log.clean_exit);
    }

    // Bridge 2 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_vr_to_cache_pose_entry_normal_ttl() {
        // frame_time_ms = 13.9 >= 12 → ttl = 5
        let entry = vr_to_cache_pose_entry(SESSION_A, 0.5, 1.7, -0.3, 1.0, 0.0, 0.0, 0.0, 13.9);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 5);
        // 位置の固定小数点変換チェック
        assert_eq!(entry.hmd_position_x_mm, 500);
        assert_eq!(entry.hmd_position_y_mm, 1700);
    }

    #[test]
    fn test_vr_to_cache_pose_entry_fast_motion_ttl() {
        // frame_time_ms = 8.3 < 12 → ttl = 1
        let entry = vr_to_cache_pose_entry(SESSION_B, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 8.3);
        assert_eq!(entry.ttl_secs, 1);
    }

    // Bridge 3 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_vr_to_analytics_comfort_event_basic() {
        let ev = vr_to_analytics_comfort_event(SESSION_A, 1000, 11_111, 20_000, 3, 92, false);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.frame_index, 1000);
        assert_eq!(ev.comfort_score, 92);
        assert_eq!(ev.reprojection_count, 3);
        assert!(!ev.asw_active);
    }

    #[test]
    fn test_vr_to_analytics_comfort_event_score_clamped() {
        let ev = vr_to_analytics_comfort_event(SESSION_B, 0, 0, 0, 0, 200, true);
        // comfort_score が 100 に丸められること
        assert_eq!(ev.comfort_score, 100);
        assert!(ev.asw_active);
    }

    // Bridge 4 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_vr_to_render_stereo_desc_left_eye() {
        let desc = vr_to_render_stereo_desc(SESSION_A, 0, 2064, 2096, 1100, 1100, 632, 2);
        assert_ne!(desc.content_hash, 0);
        assert_eq!(desc.eye, 0);
        assert_eq!(desc.render_width, 2064);
        assert_eq!(desc.ffr_level, 2);
        assert_eq!(desc.ipd_mm_x10, 632);
    }

    #[test]
    fn test_vr_to_render_stereo_desc_fields_clamped() {
        let desc = vr_to_render_stereo_desc(SESSION_B, 99, 1920, 1080, 900, 900, 640, 99);
        // eye が 1 に、ffr_level が 3 に丸められること
        assert_eq!(desc.eye, 1);
        assert_eq!(desc.ffr_level, 3);
    }

    // Bridge 5 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_vr_to_physics_interaction_grab() {
        let intr = vr_to_physics_interaction(SESSION_A, 1, 0, 0.5, -1.2, 0.0, 500, true);
        assert_ne!(intr.content_hash, 0);
        assert_eq!(intr.controller_id, 1);
        assert_eq!(intr.event_type, 0);
        assert_eq!(intr.velocity_x_mm_s, 500);
        assert_eq!(intr.velocity_y_mm_s, -1200);
        assert!(intr.haptic_triggered);
    }

    #[test]
    fn test_vr_to_physics_interaction_hash_determinism() {
        let i1 = vr_to_physics_interaction(SESSION_B, 0, 2, 1.0, 0.0, 0.0, 100, false);
        let i2 = vr_to_physics_interaction(SESSION_B, 0, 2, 1.0, 0.0, 0.0, 100, false);
        assert_eq!(i1.content_hash, i2.content_hash);
        assert_eq!(i1.event_type, 2);
    }
}
