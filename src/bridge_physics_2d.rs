//! 2D Physics bridges — ALICE-Physics (2D) ↔ View, DB, Cache, Analytics, Edge
//!
//! 5 bridges connecting the deterministic 2D physics subsystem to the ALICE ecosystem.

use alice_physics::{BodyType2D, Fix128, PhysicsConfig2D, PhysicsWorld2D, RigidBody2D, Vec2Fix};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: 2D scene → View descriptor ───────────────────────────────

/// View descriptor for rendering a 2D physics scene.
///
/// Converts Fix128 positions and angles to f32 for GPU upload.
pub struct Physics2DViewDescriptor {
    /// Content hash for frame deduplication.
    pub content_hash: u64,
    /// Body positions as f32 (x, y) pairs.
    pub positions: Vec<[f32; 2]>,
    /// Body orientation angles in radians.
    pub angles: Vec<f32>,
    /// Body linear velocities (x, y).
    pub velocities: Vec<[f32; 2]>,
    /// Number of active (non-sleeping) bodies.
    pub active_count: usize,
    /// Number of sleeping bodies.
    pub sleeping_count: usize,
}

/// Build a `Physics2DViewDescriptor` from a 2D physics world.
///
/// Single pass over bodies extracts position, angle, velocity and classifies
/// active/sleeping branchlessly via speed-squared threshold.
#[inline]
#[must_use]
pub fn physics2d_world_to_view(world: &PhysicsWorld2D) -> Physics2DViewDescriptor {
    let cap = world.bodies.len();
    let mut positions = Vec::with_capacity(cap);
    let mut angles = Vec::with_capacity(cap);
    let mut velocities = Vec::with_capacity(cap);
    let mut active_count: usize = 0;
    let mut sleeping_count: usize = 0;

    for body in &world.bodies {
        let px = body.position.x.to_f32();
        let py = body.position.y.to_f32();
        let vx = body.velocity.x.to_f32();
        let vy = body.velocity.y.to_f32();
        let a = body.angle.to_f32();

        positions.push([px, py]);
        angles.push(a);
        velocities.push([vx, vy]);

        // Branchless active/sleeping classification.
        let speed_sq = vx * vx + vy * vy;
        let is_sleeping = (speed_sq < 1e-6_f32) as usize;
        sleeping_count += is_sleeping;
        active_count += 1 - is_sleeping;
    }

    // Hash: body count + first position.
    let first_pos: [f32; 2] = positions.first().copied().unwrap_or([0.0, 0.0]);
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&(cap as u64).to_le_bytes());
    bytes[8..12].copy_from_slice(&first_pos[0].to_bits().to_le_bytes());
    bytes[12..16].copy_from_slice(&first_pos[1].to_bits().to_le_bytes());
    let content_hash = fnv1a(&bytes);

    Physics2DViewDescriptor {
        content_hash,
        positions,
        angles,
        velocities,
        active_count,
        sleeping_count,
    }
}

// ── Bridge 2: 2D body state → DB record ────────────────────────────────

/// Persistence record for a 2D rigid body in ALICE-DB.
///
/// Stores the fixed-point high-words for deterministic checkpointing
/// alongside f32 approximations for queries.
pub struct Physics2DDbRecord {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Body count in the world at checkpoint.
    pub body_count: usize,
    /// Joint count in the world at checkpoint.
    pub joint_count: usize,
    /// Gravity vector (x, y) in m/s^2.
    pub gravity: [f32; 2],
    /// Solver substeps.
    pub substeps: u32,
    /// Solver iterations per substep.
    pub iterations: u32,
    /// Determinism checksum: XOR-fold of body position X high-words.
    pub determinism_checksum: u64,
}

/// Build a `Physics2DDbRecord` from a 2D physics world.
///
/// Determinism checksum XOR-folds all body position X high-words for
/// rollback verification.
#[inline]
#[must_use]
pub fn physics2d_world_to_db(world: &PhysicsWorld2D) -> Physics2DDbRecord {
    let body_count = world.bodies.len();
    let joint_count = world.joints.len();
    let gx = world.config.gravity.x.to_f32();
    let gy = world.config.gravity.y.to_f32();

    let determinism_checksum = world
        .bodies
        .iter()
        .fold(0u64, |acc, b| acc ^ (b.position.x.hi as u64));

    let mut bytes = [0u8; 32];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(joint_count as u64).to_le_bytes());
    bytes[16..24].copy_from_slice(&determinism_checksum.to_le_bytes());
    bytes[24..28].copy_from_slice(&gx.to_bits().to_le_bytes());
    bytes[28..32].copy_from_slice(&gy.to_bits().to_le_bytes());
    let content_hash = fnv1a(&bytes);

    Physics2DDbRecord {
        content_hash,
        body_count,
        joint_count,
        gravity: [gx, gy],
        substeps: world.config.substeps as u32,
        iterations: world.config.iterations as u32,
        determinism_checksum,
    }
}

// ── Bridge 3: 2D world → Cache entry ───────────────────────────────────

/// Cache entry for a 2D physics world state snapshot.
///
/// Enables memoisation of expensive 2D simulation checkpoints.
pub struct Physics2DCacheEntry {
    /// Cache key derived from world state.
    pub content_hash: u64,
    /// Number of bodies in the cached state.
    pub body_count: usize,
    /// Estimated memory cost of storing the full state (bytes).
    pub state_size_bytes: usize,
    /// Time-to-live in seconds.
    pub ttl_secs: u32,
    /// Eviction priority (higher = keep longer).
    pub eviction_priority: u32,
}

/// Build a `Physics2DCacheEntry` from a 2D physics world and frame index.
///
/// Branchless TTL: worlds with high body counts (>100) get shorter TTL
/// to avoid stale large cache entries.
#[inline]
#[must_use]
pub fn physics2d_world_to_cache(world: &PhysicsWorld2D, frame_index: u64) -> Physics2DCacheEntry {
    let body_count = world.bodies.len();

    // Each 2D body ~ 64 bytes (2x Vec2Fix @16B + angle + inv_mass + shape tag).
    let state_size_bytes = body_count << 6;

    // Branchless TTL: large worlds (>100 bodies) get 10s, small get 60s.
    let is_large = (body_count > 100) as u32;
    let ttl_secs = 60 - is_large * 50;

    let eviction_priority = frame_index.min(u32::MAX as u64) as u32;

    // State checksum: XOR-fold position Y high-words.
    let state_checksum = world
        .bodies
        .iter()
        .fold(0u64, |acc, b| acc ^ (b.position.y.hi as u64));

    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&frame_index.to_le_bytes());
    bytes[16..24].copy_from_slice(&state_checksum.to_le_bytes());
    let content_hash = fnv1a(&bytes);

    Physics2DCacheEntry {
        content_hash,
        body_count,
        state_size_bytes,
        ttl_secs,
        eviction_priority,
    }
}

// ── Bridge 4: 2D world stats → Analytics event ─────────────────────────

/// Analytics event for 2D physics simulation metrics.
///
/// Captures per-step throughput and body-population data for monitoring.
pub struct Physics2DAnalyticsEvent {
    /// Content hash for time-series deduplication.
    pub content_hash: u64,
    /// Total body count.
    pub body_count: usize,
    /// Number of active (moving) bodies.
    pub active_bodies: usize,
    /// Number of sleeping bodies.
    pub sleeping_bodies: usize,
    /// Number of dynamic bodies.
    pub dynamic_count: usize,
    /// Number of static bodies.
    pub static_count: usize,
    /// Number of kinematic bodies.
    pub kinematic_count: usize,
    /// Kinetic energy proxy: sum of speed-squared over active bodies.
    pub total_kinetic_energy_proxy: f32,
    /// Maximum speed-squared observed.
    pub max_speed_sq: f32,
    /// Simulation frequency in Hz (60 * substeps).
    pub sim_frequency_hz: f32,
}

/// Build `Physics2DAnalyticsEvent` from a 2D physics world.
///
/// Single pass classifies bodies by type and activity state. Body type
/// counters use match on enum (not `as u8` cast).
#[inline]
#[must_use]
pub fn physics2d_world_to_analytics(world: &PhysicsWorld2D) -> Physics2DAnalyticsEvent {
    let body_count = world.bodies.len();
    let mut active_bodies: usize = 0;
    let mut dynamic_count: usize = 0;
    let mut static_count: usize = 0;
    let mut kinematic_count: usize = 0;
    let mut total_ke_proxy = 0.0f32;
    let mut max_speed_sq = 0.0f32;

    for body in &world.bodies {
        let vx = body.velocity.x.to_f32();
        let vy = body.velocity.y.to_f32();
        let speed_sq = vx * vx + vy * vy;

        let is_active = (speed_sq >= 1e-6_f32) as usize;
        active_bodies += is_active;
        total_ke_proxy += speed_sq * (is_active as f32);
        max_speed_sq = max_speed_sq.max(speed_sq);

        match body.body_type {
            BodyType2D::Dynamic => dynamic_count += 1,
            BodyType2D::Static => static_count += 1,
            BodyType2D::Kinematic => kinematic_count += 1,
        }
    }

    let sleeping_bodies = body_count - active_bodies;
    let sim_frequency_hz = 60.0_f32 * world.config.substeps as f32;

    let mut bytes = [0u8; 32];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(active_bodies as u64).to_le_bytes());
    bytes[16..20].copy_from_slice(&total_ke_proxy.to_bits().to_le_bytes());
    bytes[20..24].copy_from_slice(&max_speed_sq.to_bits().to_le_bytes());
    bytes[24..28].copy_from_slice(&(dynamic_count as u32).to_le_bytes());
    bytes[28..32].copy_from_slice(&(static_count as u32).to_le_bytes());
    let content_hash = fnv1a(&bytes);

    Physics2DAnalyticsEvent {
        content_hash,
        body_count,
        active_bodies,
        sleeping_bodies,
        dynamic_count,
        static_count,
        kinematic_count,
        total_kinetic_energy_proxy: total_ke_proxy,
        max_speed_sq,
        sim_frequency_hz,
    }
}

// ── Bridge 5: 2D body → Edge snapshot ──────────────────────────────────

/// Per-body edge snapshot for ALICE-Edge telemetry.
///
/// Lightweight struct suitable for IoT/sensor pipeline ingestion.
pub struct Physics2DEdgeSnapshot {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Body position (x, y) in f32.
    pub position: [f32; 2],
    /// Body velocity (x, y) in f32.
    pub velocity: [f32; 2],
    /// Body angle in radians.
    pub angle: f32,
    /// Speed (magnitude of velocity).
    pub speed: f32,
    /// Body type tag: 0=Dynamic, 1=Static, 2=Kinematic.
    pub body_type_tag: u8,
}

/// Build a `Physics2DEdgeSnapshot` from a single 2D rigid body.
///
/// Body type tag mapped via match for deterministic encoding.
#[inline]
#[must_use]
pub fn physics2d_body_to_edge(body: &RigidBody2D, body_index: usize) -> Physics2DEdgeSnapshot {
    let px = body.position.x.to_f32();
    let py = body.position.y.to_f32();
    let vx = body.velocity.x.to_f32();
    let vy = body.velocity.y.to_f32();
    let angle = body.angle.to_f32();
    let speed = (vx * vx + vy * vy).sqrt();

    let body_type_tag = match body.body_type {
        BodyType2D::Dynamic => 0,
        BodyType2D::Static => 1,
        BodyType2D::Kinematic => 2,
    };

    let mut bytes = [0u8; 24];
    bytes[0..4].copy_from_slice(&px.to_bits().to_le_bytes());
    bytes[4..8].copy_from_slice(&py.to_bits().to_le_bytes());
    bytes[8..12].copy_from_slice(&vx.to_bits().to_le_bytes());
    bytes[12..16].copy_from_slice(&vy.to_bits().to_le_bytes());
    bytes[16..20].copy_from_slice(&(body_index as u32).to_le_bytes());
    bytes[20] = body_type_tag;
    let content_hash = fnv1a(&bytes);

    Physics2DEdgeSnapshot {
        content_hash,
        position: [px, py],
        velocity: [vx, vy],
        angle,
        speed,
        body_type_tag,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_physics::{Fix128, PhysicsConfig2D, Shape2D, Vec2Fix};

    fn make_world(n: usize) -> PhysicsWorld2D {
        let config = PhysicsConfig2D::default();
        let mut world = PhysicsWorld2D::new(config);
        for i in 0..n {
            let mut body = RigidBody2D::new_dynamic(
                Vec2Fix::from_int(i as i64, (i * 2) as i64),
                Fix128::ONE,
                Shape2D::Circle {
                    radius: Fix128::ONE,
                },
            );
            body.velocity = Vec2Fix::from_int(i as i64, 0);
            world.add_body(body);
        }
        world
    }

    // ── Test 1: View descriptor ────────────────────────────────────────

    #[test]
    fn test_physics2d_world_to_view() {
        let world = make_world(4);
        let view = physics2d_world_to_view(&world);

        assert_eq!(view.positions.len(), 4);
        assert_eq!(view.angles.len(), 4);
        assert_eq!(view.velocities.len(), 4);
        assert_eq!(view.active_count + view.sleeping_count, 4);
        // Body 0 has velocity=(0,0) → sleeping.
        assert!(view.sleeping_count >= 1);
        assert!(view.active_count >= 3);
        assert_ne!(view.content_hash, 0);
    }

    // ── Test 2: View descriptor hash determinism ───────────────────────

    #[test]
    fn test_view_hash_determinism() {
        let world = make_world(4);
        let v1 = physics2d_world_to_view(&world);
        let v2 = physics2d_world_to_view(&world);
        assert_eq!(v1.content_hash, v2.content_hash);
    }

    // ── Test 3: DB record ──────────────────────────────────────────────

    #[test]
    fn test_physics2d_world_to_db() {
        let world = make_world(6);
        let rec = physics2d_world_to_db(&world);

        assert_eq!(rec.body_count, 6);
        assert_eq!(rec.joint_count, 0);
        assert!(rec.gravity[1] < 0.0, "gravity Y should be negative");
        assert_eq!(rec.substeps, 4);
        assert_eq!(rec.iterations, 8);
        assert_ne!(rec.content_hash, 0);
    }

    // ── Test 4: DB record hash determinism ─────────────────────────────

    #[test]
    fn test_db_hash_determinism() {
        let world = make_world(6);
        let r1 = physics2d_world_to_db(&world);
        let r2 = physics2d_world_to_db(&world);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.determinism_checksum, r2.determinism_checksum);
    }

    // ── Test 5: Cache entry small world ────────────────────────────────

    #[test]
    fn test_physics2d_cache_small_world() {
        let world = make_world(8);
        let entry = physics2d_world_to_cache(&world, 500);

        assert_eq!(entry.body_count, 8);
        assert_eq!(entry.state_size_bytes, 8 << 6);
        assert_eq!(entry.ttl_secs, 60, "small world should get 60s TTL");
        assert_eq!(entry.eviction_priority, 500);
        assert_ne!(entry.content_hash, 0);
    }

    // ── Test 6: Cache entry large world (branchless TTL) ───────────────

    #[test]
    fn test_physics2d_cache_large_world() {
        let config = PhysicsConfig2D::default();
        let mut world = PhysicsWorld2D::new(config);
        for i in 0..150 {
            let body = RigidBody2D::new_dynamic(
                Vec2Fix::from_int(i, 0),
                Fix128::ONE,
                Shape2D::Circle {
                    radius: Fix128::ONE,
                },
            );
            world.add_body(body);
        }
        let entry = physics2d_world_to_cache(&world, 1000);
        assert_eq!(entry.ttl_secs, 10, "large world should get 10s TTL");
    }

    // ── Test 7: Analytics event ────────────────────────────────────────

    #[test]
    fn test_physics2d_world_to_analytics() {
        let world = make_world(5);
        let evt = physics2d_world_to_analytics(&world);

        assert_eq!(evt.body_count, 5);
        assert_eq!(evt.active_bodies + evt.sleeping_bodies, 5);
        assert!(evt.sleeping_bodies >= 1);
        assert_eq!(evt.dynamic_count, 5);
        assert_eq!(evt.static_count, 0);
        assert_eq!(evt.kinematic_count, 0);
        assert!(evt.total_kinetic_energy_proxy > 0.0);
        // Fastest body is body 4 with v=(4,0) → speed_sq=16.
        assert!((evt.max_speed_sq - 16.0).abs() < 0.1);
        let expected_hz = 60.0 * 4.0; // default substeps=4
        assert!((evt.sim_frequency_hz - expected_hz).abs() < 0.1);
        assert_ne!(evt.content_hash, 0);
    }

    // ── Test 8: Edge snapshot ──────────────────────────────────────────

    #[test]
    fn test_physics2d_body_to_edge() {
        let body = RigidBody2D::new_dynamic(
            Vec2Fix::from_int(3, 7),
            Fix128::ONE,
            Shape2D::Circle {
                radius: Fix128::ONE,
            },
        );
        let snap = physics2d_body_to_edge(&body, 42);

        assert!((snap.position[0] - 3.0).abs() < 0.01);
        assert!((snap.position[1] - 7.0).abs() < 0.01);
        assert_eq!(snap.body_type_tag, 0); // Dynamic
        assert_ne!(snap.content_hash, 0);
    }

    // ── Test 9: Edge snapshot hash determinism ─────────────────────────

    #[test]
    fn test_edge_hash_determinism() {
        let body = RigidBody2D::new_dynamic(
            Vec2Fix::from_int(1, 2),
            Fix128::ONE,
            Shape2D::Circle {
                radius: Fix128::ONE,
            },
        );
        let s1 = physics2d_body_to_edge(&body, 0);
        let s2 = physics2d_body_to_edge(&body, 0);
        assert_eq!(s1.content_hash, s2.content_hash);
    }

    // ── Test 10: Static body type tag ──────────────────────────────────

    #[test]
    fn test_static_body_type_tag() {
        let body = RigidBody2D::new_static(
            Vec2Fix::from_int(0, 0),
            Shape2D::Edge {
                start: Vec2Fix::from_int(-10, 0),
                end: Vec2Fix::from_int(10, 0),
            },
        );
        let snap = physics2d_body_to_edge(&body, 0);
        assert_eq!(snap.body_type_tag, 1); // Static
    }
}
