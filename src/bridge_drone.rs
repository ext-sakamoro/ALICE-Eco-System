//! Drone bridges — ALICE-Drone ↔ DB, Cache, Analytics, Physics, Navigation
//!
//! 5 bridges connecting drone telemetry and mission data (extracted as
//! primitives) to the ALICE ecosystem. No external crate types are imported;
//! all fields use primitive types derived from serialised drone state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Drone → DB (telemetry snapshot persistence) ─────────────────

/// Drone telemetry snapshot for ALICE-DB persistence.
pub struct DroneDbRecord {
    /// Content hash over drone_id, position, and timestamp.
    pub content_hash: u64,
    /// Opaque drone identifier hash.
    pub drone_id_hash: u64,
    /// X-coordinate in metres (world frame).
    pub pos_x: f64,
    /// Y-coordinate in metres (world frame).
    pub pos_y: f64,
    /// Z-coordinate in metres (world frame, up-positive).
    pub pos_z: f64,
    /// Altitude above mean sea level in metres.
    pub altitude: f64,
    /// Battery charge as a percentage (0.0–100.0).
    pub battery_pct: f64,
    /// Heading in degrees from north (0.0–360.0).
    pub heading: f64,
    /// Number of waypoints remaining in the current mission.
    pub waypoint_count: u32,
    /// Unix timestamp in nanoseconds when this snapshot was captured.
    pub captured_at_ns: u64,
}

/// Build a DB persistence record from extracted drone telemetry.
#[inline]
#[must_use]
pub fn drone_to_db_record(
    drone_id: &[u8],
    pos_x: f64,
    pos_y: f64,
    pos_z: f64,
    altitude: f64,
    battery_pct: f64,
    heading: f64,
    waypoint_count: u32,
    captured_at_ns: u64,
) -> DroneDbRecord {
    let drone_id_hash = fnv1a(drone_id);
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&drone_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&pos_x.to_le_bytes());
    buf[16..24].copy_from_slice(&pos_y.to_le_bytes());
    buf[24..32].copy_from_slice(&captured_at_ns.to_le_bytes());
    DroneDbRecord {
        content_hash: fnv1a(&buf),
        drone_id_hash,
        pos_x,
        pos_y,
        pos_z,
        altitude,
        battery_pct,
        heading,
        waypoint_count,
        captured_at_ns,
    }
}

// ── Bridge 2: Drone → Cache (live state caching) ──────────────────────────

/// Cached drone live-state entry for ALICE-Cache.
pub struct DroneCacheEntry {
    /// Content hash over drone_id_hash and captured_at_ns.
    pub content_hash: u64,
    /// Hashed drone identifier used as cache key.
    pub drone_id_hash: u64,
    /// X-coordinate.
    pub pos_x: f64,
    /// Y-coordinate.
    pub pos_y: f64,
    /// Z-coordinate.
    pub pos_z: f64,
    /// Battery percentage.
    pub battery_pct: f64,
    /// TTL in milliseconds (branchless: shorter when battery is low).
    pub ttl_ms: u32,
    /// Capture timestamp in nanoseconds.
    pub captured_at_ns: u64,
}

/// Build a cache entry for a drone's live state.
///
/// TTL is 500 ms by default; reduced to 100 ms when `battery_pct` falls
/// below 20.0 (low-battery drones require more frequent state updates).
#[inline]
#[must_use]
pub fn drone_to_cache_entry(
    drone_id: &[u8],
    pos_x: f64,
    pos_y: f64,
    pos_z: f64,
    battery_pct: f64,
    captured_at_ns: u64,
) -> DroneCacheEntry {
    let drone_id_hash = fnv1a(drone_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&drone_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&captured_at_ns.to_le_bytes());
    // Branchless TTL: 500 - low_battery * 400
    let low_battery = (battery_pct < 20.0) as u32;
    let ttl_ms = 500 - low_battery * 400;
    DroneCacheEntry {
        content_hash: fnv1a(&buf),
        drone_id_hash,
        pos_x,
        pos_y,
        pos_z,
        battery_pct,
        ttl_ms,
        captured_at_ns,
    }
}

// ── Bridge 3: Drone → Analytics (flight metrics ingestion) ────────────────

/// Drone flight metrics event for ALICE-Analytics ingestion.
pub struct DroneAnalyticsEvent {
    /// Content hash over drone_id_hash and captured_at_ns.
    pub content_hash: u64,
    /// Hashed drone identifier.
    pub drone_id_hash: u64,
    /// Altitude at capture time.
    pub altitude: f64,
    /// Battery percentage.
    pub battery_pct: f64,
    /// Heading in degrees.
    pub heading: f64,
    /// Horizontal speed in metres per second.
    pub horizontal_speed_mps: f64,
    /// Number of waypoints remaining.
    pub waypoint_count: u32,
    /// Flight duration in seconds since mission start.
    pub flight_duration_secs: u64,
}

/// Build an analytics ingestion event from drone flight telemetry.
#[inline]
#[must_use]
pub fn drone_to_analytics_event(
    drone_id: &[u8],
    altitude: f64,
    battery_pct: f64,
    heading: f64,
    horizontal_speed_mps: f64,
    waypoint_count: u32,
    flight_duration_secs: u64,
    captured_at_ns: u64,
) -> DroneAnalyticsEvent {
    let drone_id_hash = fnv1a(drone_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&drone_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&captured_at_ns.to_le_bytes());
    DroneAnalyticsEvent {
        content_hash: fnv1a(&buf),
        drone_id_hash,
        altitude,
        battery_pct,
        heading,
        horizontal_speed_mps,
        waypoint_count,
        flight_duration_secs,
    }
}

// ── Bridge 4: Drone → Physics (rigid-body state extraction) ───────────────

/// Drone rigid-body state for ALICE-Physics simulation input.
pub struct DronePhysicsState {
    /// Content hash over drone_id_hash and position.
    pub content_hash: u64,
    /// Hashed drone identifier.
    pub drone_id_hash: u64,
    /// X-coordinate (metres).
    pub pos_x: f64,
    /// Y-coordinate (metres).
    pub pos_y: f64,
    /// Z-coordinate (metres).
    pub pos_z: f64,
    /// Velocity along X-axis (m/s).
    pub vel_x: f64,
    /// Velocity along Y-axis (m/s).
    pub vel_y: f64,
    /// Velocity along Z-axis (m/s).
    pub vel_z: f64,
    /// Mass of the drone in kilograms.
    pub mass_kg: f64,
    /// Heading in degrees (used to orient the drag model).
    pub heading: f64,
}

/// Build a physics rigid-body state from drone telemetry.
#[inline]
#[must_use]
pub fn drone_to_physics_state(
    drone_id: &[u8],
    pos_x: f64,
    pos_y: f64,
    pos_z: f64,
    vel_x: f64,
    vel_y: f64,
    vel_z: f64,
    mass_kg: f64,
    heading: f64,
) -> DronePhysicsState {
    let drone_id_hash = fnv1a(drone_id);
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&drone_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&pos_x.to_le_bytes());
    buf[16..24].copy_from_slice(&pos_y.to_le_bytes());
    buf[24..32].copy_from_slice(&pos_z.to_le_bytes());
    DronePhysicsState {
        content_hash: fnv1a(&buf),
        drone_id_hash,
        pos_x,
        pos_y,
        pos_z,
        vel_x,
        vel_y,
        vel_z,
        mass_kg,
        heading,
    }
}

// ── Bridge 5: Drone → Navigation (waypoint mission state) ─────────────────

/// Drone navigation mission state for ALICE-Navigation path planning.
pub struct DroneNavigationState {
    /// Content hash over drone_id_hash, waypoint_count, and pos fields.
    pub content_hash: u64,
    /// Hashed drone identifier.
    pub drone_id_hash: u64,
    /// Current X-coordinate.
    pub pos_x: f64,
    /// Current Y-coordinate.
    pub pos_y: f64,
    /// Current Z-coordinate.
    pub pos_z: f64,
    /// Current heading in degrees.
    pub heading: f64,
    /// Number of waypoints remaining in the mission.
    pub waypoint_count: u32,
    /// Battery percentage (used for range planning).
    pub battery_pct: f64,
    /// Estimated range remaining in metres (battery_pct × range_per_pct).
    pub range_remaining_m: f64,
}

/// Build a navigation mission state from drone telemetry.
///
/// `range_per_pct` is the metres of range corresponding to 1% battery.
#[inline]
#[must_use]
pub fn drone_to_navigation_state(
    drone_id: &[u8],
    pos_x: f64,
    pos_y: f64,
    pos_z: f64,
    heading: f64,
    waypoint_count: u32,
    battery_pct: f64,
    range_per_pct: f64,
) -> DroneNavigationState {
    let drone_id_hash = fnv1a(drone_id);
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&drone_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&pos_x.to_le_bytes());
    buf[16..24].copy_from_slice(&pos_y.to_le_bytes());
    buf[24..28].copy_from_slice(&waypoint_count.to_le_bytes());
    let range_remaining_m = battery_pct * range_per_pct;
    DroneNavigationState {
        content_hash: fnv1a(&buf),
        drone_id_hash,
        pos_x,
        pos_y,
        pos_z,
        heading,
        waypoint_count,
        battery_pct,
        range_remaining_m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = drone_to_db_record(b"drone-01", 10.0, 20.0, 30.0, 100.0, 85.0, 90.0, 5, 1_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.drone_id_hash, 0);
    }

    #[test]
    fn db_record_fields_preserved() {
        let rec = drone_to_db_record(b"d", 1.1, 2.2, 3.3, 50.0, 72.5, 180.0, 3, 9_999);
        assert!((rec.pos_x - 1.1).abs() < f64::EPSILON);
        assert!((rec.pos_y - 2.2).abs() < f64::EPSILON);
        assert!((rec.pos_z - 3.3).abs() < f64::EPSILON);
        assert_eq!(rec.waypoint_count, 3);
        assert_eq!(rec.captured_at_ns, 9_999);
    }

    #[test]
    fn db_record_hash_deterministic() {
        let a = drone_to_db_record(b"dx", 0.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0, 0);
        let b = drone_to_db_record(b"dx", 0.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_high_battery_long_ttl() {
        let entry = drone_to_cache_entry(b"d1", 0.0, 0.0, 0.0, 80.0, 1_000);
        assert_eq!(entry.ttl_ms, 500);
    }

    #[test]
    fn cache_entry_low_battery_short_ttl() {
        let entry = drone_to_cache_entry(b"d2", 0.0, 0.0, 0.0, 15.0, 2_000);
        assert_eq!(entry.ttl_ms, 100);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_hash_nonzero() {
        let ev = drone_to_analytics_event(b"drone-42", 120.0, 60.0, 45.0, 5.5, 8, 300, 5_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.waypoint_count, 8);
    }

    // ── Navigation state tests ────────────────────────────────────────────

    #[test]
    fn navigation_range_remaining_computed() {
        // 80% battery × 10 m/pct = 800 m
        let nav = drone_to_navigation_state(b"dn", 0.0, 0.0, 50.0, 270.0, 4, 80.0, 10.0);
        assert!((nav.range_remaining_m - 800.0).abs() < 1e-9);
        assert_ne!(nav.content_hash, 0);
    }

    // ── Physics state tests ───────────────────────────────────────────────

    #[test]
    fn physics_state_fields_and_hash() {
        let ps = drone_to_physics_state(b"dp", 5.0, 10.0, 20.0, 1.0, 0.5, -0.1, 1.5, 90.0);
        assert!((ps.mass_kg - 1.5).abs() < f64::EPSILON);
        assert!((ps.vel_z - (-0.1)).abs() < f64::EPSILON);
        assert_ne!(ps.content_hash, 0);
    }
}
