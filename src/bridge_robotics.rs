//! Robotics bridges — ALICE-Robotics ↔ DB, Cache, Analytics, Edge, Kinematics
//!
//! 5 bridges connecting robot task and joint data (extracted as primitives)
//! to the ALICE ecosystem. No external crate types are imported; all fields
//! use primitive types derived from serialised robotics state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Robotics → DB (robot snapshot persistence) ──────────────────

/// Robot snapshot for ALICE-DB persistence.
pub struct RoboticsDbRecord {
    /// Content hash over robot_hash, task_count, and uptime_ms.
    pub content_hash: u64,
    /// Opaque robot identifier hash.
    pub robot_hash: u64,
    /// Number of joints on this robot.
    pub joint_count: u8,
    /// Total tasks executed since power-on.
    pub task_count: u64,
    /// Robot uptime in milliseconds since last boot.
    pub uptime_ms: u64,
    /// Hash of the currently loaded firmware image.
    pub firmware_hash: u64,
}

/// Build a DB persistence record from extracted robotics state.
#[inline]
#[must_use]
pub fn robotics_to_db_record(
    robot_id: &[u8],
    joint_count: u8,
    task_count: u64,
    uptime_ms: u64,
    firmware_id: &[u8],
) -> RoboticsDbRecord {
    let robot_hash = fnv1a(robot_id);
    let firmware_hash = fnv1a(firmware_id);
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&robot_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&task_count.to_le_bytes());
    buf[16..24].copy_from_slice(&uptime_ms.to_le_bytes());
    RoboticsDbRecord {
        content_hash: fnv1a(&buf),
        robot_hash,
        joint_count,
        task_count,
        uptime_ms,
        firmware_hash,
    }
}

// ── Bridge 2: Robotics → Cache (live robot state caching) ─────────────────

/// Cached robot live-state entry for ALICE-Cache.
pub struct RoboticsCacheEntry {
    /// Content hash over robot_hash and task_count.
    pub content_hash: u64,
    /// Hashed robot identifier used as cache key.
    pub robot_hash: u64,
    /// TTL in seconds for this cache entry.
    pub ttl_secs: u32,
    /// Serialised state size in bytes.
    pub state_bytes: u64,
    /// Number of joints included in the state snapshot.
    pub joint_count: u8,
}

/// Build a cache entry for a robot's live state.
///
/// TTL is 10 s by default; reduced to 2 s when `joint_count` exceeds 12
/// to keep high-DOF robot states fresh for real-time control loops.
#[inline]
#[must_use]
pub fn robotics_to_cache_entry(
    robot_id: &[u8],
    task_count: u64,
    state_bytes: u64,
    joint_count: u8,
) -> RoboticsCacheEntry {
    let robot_hash = fnv1a(robot_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&robot_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&task_count.to_le_bytes());
    let high_dof = (joint_count > 12) as u32;
    let ttl_secs = 10 - high_dof * 8;
    RoboticsCacheEntry {
        content_hash: fnv1a(&buf),
        robot_hash,
        ttl_secs,
        state_bytes,
        joint_count,
    }
}

// ── Bridge 3: Robotics → Analytics (task metrics ingestion) ───────────────

/// Robot task metrics event for ALICE-Analytics ingestion.
pub struct RoboticsAnalyticsEvent {
    /// Content hash over robot_hash and timestamp_ms.
    pub content_hash: u64,
    /// Total tasks completed since power-on.
    pub task_count: u64,
    /// Number of errors encountered since power-on.
    pub error_count: u32,
    /// Average task cycle time in microseconds.
    pub cycle_time_us: u64,
    /// Positioning accuracy in micrometres.
    pub accuracy_um: u32,
    /// Unix timestamp in milliseconds when the event was recorded.
    pub timestamp_ms: u64,
}

/// Build an analytics ingestion event from robot task metrics.
#[inline]
#[must_use]
pub fn robotics_to_analytics_event(
    robot_id: &[u8],
    task_count: u64,
    error_count: u32,
    cycle_time_us: u64,
    accuracy_um: u32,
    timestamp_ms: u64,
) -> RoboticsAnalyticsEvent {
    let robot_hash = fnv1a(robot_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&robot_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    RoboticsAnalyticsEvent {
        content_hash: fnv1a(&buf),
        task_count,
        error_count,
        cycle_time_us,
        accuracy_um,
        timestamp_ms,
    }
}

// ── Bridge 4: Robotics → Edge (on-device telemetry) ───────────────────────

/// Robot on-device telemetry for ALICE-Edge transmission.
pub struct RoboticsEdgeTelemetry {
    /// Content hash over robot_hash and torque_sum_x1000.
    pub content_hash: u64,
    /// Hashed robot identifier.
    pub robot_hash: u64,
    /// Number of joints sampled in this telemetry packet.
    pub joint_count: u8,
    /// Sum of all joint torques multiplied by 1000 (Nm × 1000).
    pub torque_sum_x1000: u64,
    /// Average joint temperature multiplied by 10 (°C × 10).
    pub temperature_c_x10: u16,
}

/// Build an edge telemetry packet from robot joint sensor data.
#[inline]
#[must_use]
pub fn robotics_to_edge_telemetry(
    robot_id: &[u8],
    joint_count: u8,
    torque_sum_x1000: u64,
    temperature_c_x10: u16,
) -> RoboticsEdgeTelemetry {
    let robot_hash = fnv1a(robot_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&robot_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&torque_sum_x1000.to_le_bytes());
    RoboticsEdgeTelemetry {
        content_hash: fnv1a(&buf),
        robot_hash,
        joint_count,
        torque_sum_x1000,
        temperature_c_x10,
    }
}

// ── Bridge 5: Robotics → Kinematics (joint configuration link) ────────────

/// Robot kinematics configuration link for ALICE-Kinematics.
pub struct RoboticsKinematicsLink {
    /// Content hash over joint_count, dof, and workspace_volume_mm3.
    pub content_hash: u64,
    /// Number of joints in the kinematic chain.
    pub joint_count: u8,
    /// Degrees of freedom of the robot arm.
    pub dof: u8,
    /// Reachable workspace volume in cubic millimetres.
    pub workspace_volume_mm3: u64,
    /// Maximum payload in grams.
    pub payload_g: u32,
}

/// Build a kinematics configuration link from robot structural parameters.
#[inline]
#[must_use]
pub fn robotics_to_kinematics_link(
    joint_count: u8,
    dof: u8,
    workspace_volume_mm3: u64,
    payload_g: u32,
) -> RoboticsKinematicsLink {
    let mut buf = [0u8; 10];
    buf[0] = joint_count;
    buf[1] = dof;
    buf[2..10].copy_from_slice(&workspace_volume_mm3.to_le_bytes());
    RoboticsKinematicsLink {
        content_hash: fnv1a(&buf),
        joint_count,
        dof,
        workspace_volume_mm3,
        payload_g,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = robotics_to_db_record(b"robot-01", 6, 1_000, 3_600_000, b"fw-v1.2");
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.robot_hash, 0);
        assert_ne!(rec.firmware_hash, 0);
    }

    #[test]
    fn db_record_fields_preserved() {
        let rec = robotics_to_db_record(b"r", 7, 500, 900_000, b"fw");
        assert_eq!(rec.joint_count, 7);
        assert_eq!(rec.task_count, 500);
        assert_eq!(rec.uptime_ms, 900_000);
    }

    #[test]
    fn db_record_hash_deterministic() {
        let a = robotics_to_db_record(b"rx", 4, 100, 60_000, b"fw-x");
        let b = robotics_to_db_record(b"rx", 4, 100, 60_000, b"fw-x");
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_low_dof_long_ttl() {
        let entry = robotics_to_cache_entry(b"r1", 10, 2_048, 6);
        assert_eq!(entry.ttl_secs, 10);
    }

    #[test]
    fn cache_entry_high_dof_short_ttl() {
        let entry = robotics_to_cache_entry(b"r2", 20, 4_096, 14);
        assert_eq!(entry.ttl_secs, 2);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_fields_and_hash() {
        let ev = robotics_to_analytics_event(b"rbt-99", 5_000, 3, 50_000, 5, 9_000_000);
        assert_eq!(ev.task_count, 5_000);
        assert_eq!(ev.error_count, 3);
        assert_eq!(ev.cycle_time_us, 50_000);
        assert_eq!(ev.accuracy_um, 5);
        assert_ne!(ev.content_hash, 0);
    }

    // ── Edge telemetry tests ──────────────────────────────────────────────

    #[test]
    fn edge_telemetry_fields_and_hash() {
        let tel = robotics_to_edge_telemetry(b"re", 6, 12_345_678, 350);
        assert_eq!(tel.joint_count, 6);
        assert_eq!(tel.torque_sum_x1000, 12_345_678);
        assert_eq!(tel.temperature_c_x10, 350);
        assert_ne!(tel.content_hash, 0);
    }

    // ── Kinematics link tests ─────────────────────────────────────────────

    #[test]
    fn kinematics_link_fields_and_hash() {
        let link = robotics_to_kinematics_link(6, 6, 500_000_000, 5_000);
        assert_eq!(link.joint_count, 6);
        assert_eq!(link.dof, 6);
        assert_eq!(link.workspace_volume_mm3, 500_000_000);
        assert_eq!(link.payload_g, 5_000);
        assert_ne!(link.content_hash, 0);
    }

    #[test]
    fn kinematics_link_different_dof_different_hash() {
        let a = robotics_to_kinematics_link(6, 6, 500_000_000, 5_000);
        let b = robotics_to_kinematics_link(6, 7, 500_000_000, 5_000);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
