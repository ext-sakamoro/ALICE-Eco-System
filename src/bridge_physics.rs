//! Physics bridges — ALICE-Physics ↔ SDF, View, DB, Cache, Analytics
//!
//! 6 bridges connecting deterministic physics simulation to the ALICE ecosystem.

use alice_physics::{
    RigidBody,
    ManifoldConfig,
    SdfForceType,
    PhysicsConfig,
};

// ── Bridge 1: Physics ↔ SDF (collision detection config) ────────────────

/// SDF collider configuration derived from physics SDF node parameters.
///
/// Describes how a physics body should collide with an SDF surface,
/// including sample budget, scale, and penetration thresholds.
pub struct SdfColliderConfig {
    /// World-space position of the SDF origin.
    pub position: [f32; 3],
    /// Uniform scale factor applied to the SDF field.
    pub scale: f32,
    /// Index of the rigid body this collider is attached to
    /// (`usize::MAX` = static world geometry).
    pub body_index: usize,
    /// Minimum penetration depth (metres) before a contact is recorded.
    pub min_penetration: f32,
    /// Maximum sphere-cast radius used for broad-phase rejection.
    pub broad_phase_radius: f32,
    /// Content hash for deduplication in the collider registry.
    pub content_hash: u64,
}

/// Build an `SdfColliderConfig` from a rigid body and manifold settings.
///
/// Encodes body position, manifold thresholds, and scale into a descriptor
/// suitable for passing to ALICE-SDF's collider pipeline.
///
/// # Optimization notes
/// - Position components extracted via `Vec3Fix::to_f32()` — one call, no branches.
/// - Scale derived via `f32::max()` — clamps away from zero without branching.
/// - `broad_phase_radius` = `collision_radius * scale` is a single multiply.
#[inline]
pub fn physics_to_sdf_collider_config(
    body: &RigidBody,
    body_index: usize,
    manifold: &ManifoldConfig,
    collision_radius: f32,
) -> SdfColliderConfig {
    let (px, py, pz) = body.position.to_f32();

    // Scale: treat manifold sample_radius as the characteristic length.
    // Clamp away from zero with max() to keep inv_scale finite.
    let scale = manifold.sample_radius.max(1e-6_f32);

    // broad_phase_radius: sphere that contains the manifold sampling area.
    let broad_phase_radius = collision_radius * scale;

    // Hash: encode position grid cell + body_index + manifold params.
    let mut bytes = [0u8; 32];
    bytes[0..4].copy_from_slice(&(px as i32).to_le_bytes());
    bytes[4..8].copy_from_slice(&(py as i32).to_le_bytes());
    bytes[8..12].copy_from_slice(&(pz as i32).to_le_bytes());
    bytes[12..16].copy_from_slice(&(body_index as u32).to_le_bytes());
    bytes[16..20].copy_from_slice(&(manifold.samples_per_axis as u32).to_le_bytes());
    bytes[20..24].copy_from_slice(&(manifold.max_contacts as u32).to_le_bytes());
    bytes[24..28].copy_from_slice(&scale.to_bits().to_le_bytes());
    bytes[28..32].copy_from_slice(&manifold.min_depth.to_bits().to_le_bytes());
    let content_hash = crate::hash::fnv1a(&bytes);

    SdfColliderConfig {
        position: [px, py, pz],
        scale,
        body_index,
        min_penetration: manifold.min_depth,
        broad_phase_radius,
        content_hash,
    }
}

// ── Bridge 2: Physics → SDF (force field influence descriptor) ───────────

/// Description of how an SDF force field influences SDF geometry.
///
/// Passed to ALICE-SDF to annotate regions of the signed distance field
/// that are actively deformed or modified by physics force fields
/// (e.g., wind bending a surface, a containment field warping a mesh).
pub struct PhysicsForceFieldDesc {
    /// SDF collider index that this force field is bound to.
    pub sdf_index: usize,
    /// Force type tag: 0=Attract, 1=Repel, 2=Contain, 3=SurfaceFlow, 4=SdfVortex.
    pub force_type_tag: u8,
    /// Scalar strength of the force (in simulation units).
    pub strength: f32,
    /// Maximum effective distance (metres) from the SDF surface.
    pub influence_distance: f32,
    /// Flow or vortex axis direction (normalised), or zero for scalar fields.
    pub axis: [f32; 3],
    /// Whether the force field is currently active.
    pub enabled: bool,
    /// Content hash for deduplication / change detection.
    pub content_hash: u64,
}

/// Build a `PhysicsForceFieldDesc` from an `SdfForceType` and its binding.
///
/// # Optimization notes
/// - Tag selection is a single integer assignment per match arm — no branches after dispatch.
/// - All f32 fields are extracted once inside each arm; no secondary calls.
/// - No heap allocation; all fields are scalars or small fixed-size arrays.
#[inline]
pub fn physics_to_force_field_desc(
    sdf_index: usize,
    force_type: &SdfForceType,
    enabled: bool,
) -> PhysicsForceFieldDesc {
    let (tag, strength, influence_distance, axis): (u8, f32, f32, [f32; 3]) = match force_type {
        SdfForceType::Attract { strength, max_force } => {
            let s = strength.to_f32();
            let mf = max_force.to_f32();
            // influence_distance: max_force / strength gives the effective pull range.
            // Reciprocal multiply: pre-guard avoids divide-by-zero branchlessly via max().
            let inv_s = 1.0_f32 / s.max(1e-10_f32);
            let range = mf * inv_s;
            (0, s, range, [0.0; 3])
        }
        SdfForceType::Repel { strength, range } => {
            (1, strength.to_f32(), range.to_f32(), [0.0; 3])
        }
        SdfForceType::Contain { strength, damping } => {
            // damping stored in influence_distance slot for downstream consumers.
            (2, strength.to_f32(), damping.to_f32(), [0.0; 3])
        }
        SdfForceType::SurfaceFlow { flow_direction, strength, influence_distance } => {
            let (dx, dy, dz) = flow_direction.to_f32();
            (3, strength.to_f32(), influence_distance.to_f32(), [dx, dy, dz])
        }
        SdfForceType::SdfVortex { axis, strength, influence_distance } => {
            let (ax, ay, az) = axis.to_f32();
            (4, strength.to_f32(), influence_distance.to_f32(), [ax, ay, az])
        }
    };

    // Hash: encode sdf_index + tag + strength + influence + axis + enabled.
    let mut bytes = [0u8; 32];
    bytes[0..4].copy_from_slice(&(sdf_index as u32).to_le_bytes());
    bytes[4] = tag;
    bytes[5..9].copy_from_slice(&strength.to_bits().to_le_bytes());
    bytes[9..13].copy_from_slice(&influence_distance.to_bits().to_le_bytes());
    bytes[13..17].copy_from_slice(&axis[0].to_bits().to_le_bytes());
    bytes[17..21].copy_from_slice(&axis[1].to_bits().to_le_bytes());
    bytes[21..25].copy_from_slice(&axis[2].to_bits().to_le_bytes());
    bytes[25] = enabled as u8;
    let content_hash = crate::hash::fnv1a(&bytes);

    PhysicsForceFieldDesc {
        sdf_index,
        force_type_tag: tag,
        strength,
        influence_distance,
        axis,
        enabled,
        content_hash,
    }
}

// ── Bridge 3: Physics → View (simulation state snapshot for rendering) ───

/// Per-body state snapshot for ALICE-View rendering.
///
/// Converts Fix128 deterministic state to f32 for GPU upload.
/// Suitable for instanced rendering: one entry per rigid body.
pub struct PhysicsViewSnapshot {
    /// Body positions in world space as f32 (x, y, z).
    pub positions: Vec<[f32; 3]>,
    /// Body orientations as unit quaternions (x, y, z, w).
    pub orientations: Vec<[f32; 4]>,
    /// Body linear velocities (used for motion blur / LOD).
    pub velocities: Vec<[f32; 3]>,
    /// Number of active (non-sleeping) bodies.
    pub active_count: usize,
    /// Number of sleeping bodies.
    pub sleeping_count: usize,
    /// Simulation frame sequence number for render interpolation.
    pub frame_seq: u64,
    /// Content hash of the snapshot for frame deduplication.
    pub content_hash: u64,
}

/// Build a `PhysicsViewSnapshot` from a slice of rigid bodies.
///
/// # Optimization notes
/// - Single pass over bodies: position, orientation, velocity extracted together.
/// - Active/sleeping counts incremented branchlessly via `bool as usize` addition.
/// - First-body position folded into the hash after the loop — no speculative work.
#[inline]
pub fn physics_to_view_snapshot(bodies: &[RigidBody], frame_seq: u64) -> PhysicsViewSnapshot {
    let cap = bodies.len();
    let mut positions = Vec::with_capacity(cap);
    let mut orientations = Vec::with_capacity(cap);
    let mut velocities = Vec::with_capacity(cap);
    let mut active_count: usize = 0;
    let mut sleeping_count: usize = 0;

    for body in bodies {
        let (px, py, pz) = body.position.to_f32();
        let (vx, vy, vz) = body.velocity.to_f32();
        // QuatFix fields: x, y, z, w as Fix128.
        let qx = body.rotation.x.to_f32();
        let qy = body.rotation.y.to_f32();
        let qz = body.rotation.z.to_f32();
        let qw = body.rotation.w.to_f32();

        positions.push([px, py, pz]);
        orientations.push([qx, qy, qz, qw]);
        velocities.push([vx, vy, vz]);

        // Branchless classification: speed² threshold for sleeping detection.
        let speed_sq = vx * vx + vy * vy + vz * vz;
        let is_sleeping = (speed_sq < 1e-6_f32) as usize;
        sleeping_count += is_sleeping;
        active_count += 1 - is_sleeping;
    }

    // Hash: body count + frame_seq + first body position.
    let first_pos: [f32; 3] = positions.get(0).copied().unwrap_or([0.0, 0.0, 0.0]);
    let mut hash_bytes = [0u8; 24];
    hash_bytes[0..8].copy_from_slice(&(cap as u64).to_le_bytes());
    hash_bytes[8..16].copy_from_slice(&frame_seq.to_le_bytes());
    hash_bytes[16..20].copy_from_slice(&first_pos[0].to_bits().to_le_bytes());
    hash_bytes[20..24].copy_from_slice(&first_pos[1].to_bits().to_le_bytes());
    let content_hash = crate::hash::fnv1a(&hash_bytes);

    PhysicsViewSnapshot {
        positions,
        orientations,
        velocities,
        active_count,
        sleeping_count,
        frame_seq,
        content_hash,
    }
}

// ── Bridge 4: Physics → DB (simulation state persistence record) ─────────

/// Simulation state persistence record for ALICE-DB.
///
/// Captures the key invariants of a physics world at a checkpoint:
/// body count, constraint count, determinism checksum, and timing.
/// Designed for rollback netcode and replay system storage.
pub struct PhysicsDbRecord {
    /// Number of rigid bodies in the world at checkpoint.
    pub body_count: usize,
    /// Number of distance constraints active.
    pub constraint_count: usize,
    /// Substep duration in seconds for this simulation.
    ///
    /// Derived as `(1/60) / substeps` assuming a 60 Hz outer tick.
    pub timestep_secs: f32,
    /// Gravity vector (x, y, z) in m/s².
    pub gravity: [f32; 3],
    /// Solver iteration count (XPBD constraint iterations per substep).
    pub solver_iterations: u32,
    /// Determinism checksum: XOR-fold of all body position high words.
    pub determinism_checksum: u64,
    /// Content hash of the record for deduplication in ALICE-DB.
    pub content_hash: u64,
}

/// Build a `PhysicsDbRecord` from a body slice and `PhysicsConfig`.
///
/// # Optimization notes
/// - Determinism checksum: single XOR-fold pass over `body.position.y.hi`
///   (the most significant 64 bits of the vertical coordinate).
/// - `timestep_secs` = `RCP_60 * inv_substeps` — two reciprocal multiplies,
///   no division in the hot path.
#[inline]
pub fn physics_to_db_record(
    bodies: &[RigidBody],
    constraint_count: usize,
    config: &PhysicsConfig,
) -> PhysicsDbRecord {
    // Determinism checksum: XOR all body position Y high-words.
    let determinism_checksum = bodies
        .iter()
        .fold(0u64, |acc, b| acc ^ (b.position.y.hi as u64));

    let (gx, gy, gz) = config.gravity.to_f32();

    // Substep dt = (1/60) / substeps — reciprocal multiply avoids two divisions.
    // RCP_60 is computed once; then multiplied by inv_substeps.
    const RCP_60: f32 = 1.0 / 60.0;
    let substeps = config.substeps as f32;
    let inv_substeps = 1.0_f32 / substeps.max(1.0_f32);
    let timestep_secs = RCP_60 * inv_substeps;

    // Hash: body_count + constraint_count + checksum + gravity + timestep.
    let mut bytes = [0u8; 40];
    bytes[0..8].copy_from_slice(&(bodies.len() as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(constraint_count as u64).to_le_bytes());
    bytes[16..24].copy_from_slice(&determinism_checksum.to_le_bytes());
    bytes[24..28].copy_from_slice(&gx.to_bits().to_le_bytes());
    bytes[28..32].copy_from_slice(&gy.to_bits().to_le_bytes());
    bytes[32..36].copy_from_slice(&gz.to_bits().to_le_bytes());
    bytes[36..40].copy_from_slice(&timestep_secs.to_bits().to_le_bytes());
    let content_hash = crate::hash::fnv1a(&bytes);

    PhysicsDbRecord {
        body_count: bodies.len(),
        constraint_count,
        timestep_secs,
        gravity: [gx, gy, gz],
        solver_iterations: config.iterations as u32,
        determinism_checksum,
        content_hash,
    }
}

// ── Bridge 5: Physics → Cache (physics state cache entry) ────────────────

/// Physics state cache entry for ALICE-Cache.
///
/// Enables memoisation of expensive simulation checkpoints (e.g., the
/// result of 1000 substeps for a known initial state). Used by the
/// replay and rollback systems to skip redundant re-simulation.
pub struct PhysicsCacheEntry {
    /// Cache key: hash of the state that produced this snapshot.
    pub content_hash: u64,
    /// Number of bodies in the cached state.
    pub body_count: usize,
    /// Simulation frame index of the cached snapshot.
    pub frame_index: u64,
    /// Estimated memory cost of storing the full state (bytes).
    pub state_size_bytes: usize,
    /// Priority weight for LRU eviction (higher = keep longer).
    ///
    /// Derived from `frame_index` saturated to `u32::MAX`; recent frames
    /// score higher and are evicted last.
    pub eviction_priority: u32,
}

/// Build a `PhysicsCacheEntry` from body slice and frame index.
///
/// # Optimization notes
/// - `state_size_bytes` = `body_count << 7` (× 128, shift avoids multiply).
/// - `eviction_priority`: saturating cast via `min(frame_index, u32::MAX as u64) as u32`.
/// - State checksum XOR-folds X and Z high-words; complements the DB record's Y fold.
#[inline]
pub fn physics_to_cache_entry(bodies: &[RigidBody], frame_index: u64) -> PhysicsCacheEntry {
    let body_count = bodies.len();

    // State size: each RigidBody ≈ 128 bytes (4× Vec3Fix @24B + QuatFix + scalars).
    let state_size_bytes = body_count << 7;

    // Eviction priority: saturate frame_index into u32; recent = higher priority.
    let eviction_priority = frame_index.min(u32::MAX as u64) as u32;

    // Checksum: XOR-fold position X+Z high-words for hash entropy.
    let state_checksum = bodies.iter().fold(0u64, |acc, b| {
        acc ^ (b.position.x.hi as u64).wrapping_add(b.position.z.hi as u64)
    });

    // Hash: body_count + frame_index + state_checksum.
    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&frame_index.to_le_bytes());
    bytes[16..24].copy_from_slice(&state_checksum.to_le_bytes());
    let content_hash = crate::hash::fnv1a(&bytes);

    PhysicsCacheEntry {
        content_hash,
        body_count,
        frame_index,
        state_size_bytes,
        eviction_priority,
    }
}

// ── Bridge 6: Physics → Analytics (simulation performance metrics) ────────

/// Simulation performance metrics for ALICE-Analytics.
///
/// Captures per-step throughput and body-population data to feed the
/// ALICE-Analytics monitoring pipeline. Used to detect simulation budget
/// overruns, constraint explosion, and sleeping island health.
pub struct PhysicsAnalyticsMetrics {
    /// Total body count in the world.
    pub body_count: usize,
    /// Number of bodies currently active (speed² ≥ threshold).
    pub active_bodies: usize,
    /// Number of sleeping bodies.
    pub sleeping_bodies: usize,
    /// Estimated constraint solve cost: `active_bodies × solver_iterations`.
    pub constraint_solve_ops: u64,
    /// Kinetic energy proxy: Σ|velocity|² over active bodies (no sqrt).
    pub total_kinetic_energy_proxy: f32,
    /// Maximum speed² observed across all bodies (for outlier detection).
    pub max_speed_sq: f32,
    /// Simulation frequency in Hz (1 / substep_dt, at 60 Hz outer tick).
    pub sim_frequency_hz: f32,
    /// Content hash for time-series deduplication.
    pub content_hash: u64,
}

/// Build `PhysicsAnalyticsMetrics` from a body slice and `PhysicsConfig`.
///
/// # Optimization notes
/// - Single pass accumulates KE proxy, active count, and max speed together.
/// - Max speed uses branchless `f32::max()` — no if/else.
/// - `sim_frequency_hz` = `60.0 * substeps` — one multiply, zero divisions.
/// - `constraint_solve_ops` uses `u64` widening multiply to prevent overflow.
#[inline]
pub fn physics_to_analytics_metrics(
    bodies: &[RigidBody],
    config: &PhysicsConfig,
) -> PhysicsAnalyticsMetrics {
    let body_count = bodies.len();
    let mut active_bodies: usize = 0;
    let mut total_ke_proxy = 0.0f32;
    let mut max_speed_sq = 0.0f32;

    for body in bodies {
        let (vx, vy, vz) = body.velocity.to_f32();
        let speed_sq = vx * vx + vy * vy + vz * vz;

        // Branchless active/sleeping classification.
        let is_active = (speed_sq >= 1e-6_f32) as usize;
        active_bodies += is_active;
        total_ke_proxy += speed_sq * (is_active as f32);
        max_speed_sq = max_speed_sq.max(speed_sq);
    }

    let sleeping_bodies = body_count - active_bodies;

    // constraint_solve_ops: active_bodies × iterations (u64 widening avoids overflow).
    let constraint_solve_ops =
        (active_bodies as u64).wrapping_mul(config.iterations as u64);

    // sim_frequency_hz = 60 Hz outer tick × substeps per tick — pure multiply.
    let sim_frequency_hz = 60.0_f32 * config.substeps as f32;

    // Hash: body_count + active + ke_proxy bits + max_speed bits + solve_ops.
    let mut bytes = [0u8; 32];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(active_bodies as u64).to_le_bytes());
    bytes[16..20].copy_from_slice(&total_ke_proxy.to_bits().to_le_bytes());
    bytes[20..24].copy_from_slice(&max_speed_sq.to_bits().to_le_bytes());
    bytes[24..32].copy_from_slice(&constraint_solve_ops.to_le_bytes());
    let content_hash = crate::hash::fnv1a(&bytes);

    PhysicsAnalyticsMetrics {
        body_count,
        active_bodies,
        sleeping_bodies,
        constraint_solve_ops,
        total_kinetic_energy_proxy: total_ke_proxy,
        max_speed_sq,
        sim_frequency_hz,
        content_hash,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_physics::{PhysicsConfig, RigidBody, Fix128, Vec3Fix};
    use alice_physics::{ManifoldConfig, SdfForceType};

    /// Construct a default `PhysicsConfig` for testing.
    fn test_config() -> PhysicsConfig {
        PhysicsConfig::default()
    }

    /// Build N dynamic bodies at predictable positions for testing.
    ///
    /// Body `i` is placed at `(i, 2i, 0)` and given velocity `(i, 0, 0)`.
    /// Body 0 therefore has zero velocity (sleeping), bodies 1..N are active.
    fn make_bodies(n: usize) -> Vec<RigidBody> {
        (0..n)
            .map(|i| {
                let mut body = RigidBody::new_dynamic(
                    Vec3Fix::from_int(i as i64, (i * 2) as i64, 0),
                    Fix128::ONE,
                );
                body.velocity = Vec3Fix::from_int(i as i64, 0, 0);
                body
            })
            .collect()
    }

    // ── Test 1: Physics ↔ SDF collider config ───────────────────────────

    #[test]
    fn test_physics_to_sdf_collider_config() {
        let body = RigidBody::new_dynamic(Vec3Fix::from_int(3, 7, -2), Fix128::ONE);
        let manifold = ManifoldConfig {
            samples_per_axis: 5,
            sample_radius: 0.5,
            max_contacts: 4,
            min_depth: 0.001,
        };

        let result = physics_to_sdf_collider_config(&body, 42, &manifold, 1.0);

        // Position round-trips through Fix128 → f32 faithfully.
        assert!((result.position[0] - 3.0).abs() < 0.01, "x position mismatch");
        assert!((result.position[1] - 7.0).abs() < 0.01, "y position mismatch");

        // Scale equals sample_radius (clamped).
        assert!(result.scale > 0.0, "scale must be positive");
        assert!((result.scale - 0.5).abs() < 1e-5, "scale should equal sample_radius");

        // broad_phase_radius = collision_radius(1.0) × scale(0.5) = 0.5.
        assert!((result.broad_phase_radius - 0.5).abs() < 1e-5, "broad_phase_radius mismatch");

        // Body index and min_penetration are preserved verbatim.
        assert_eq!(result.body_index, 42);
        assert!((result.min_penetration - 0.001).abs() < 1e-6);

        // Hash is non-zero and deterministic.
        assert_ne!(result.content_hash, 0);
        let result2 = physics_to_sdf_collider_config(&body, 42, &manifold, 1.0);
        assert_eq!(result.content_hash, result2.content_hash, "hash must be deterministic");
    }

    // ── Test 2: Physics → SDF force field descriptor ────────────────────

    #[test]
    fn test_physics_to_force_field_desc() {
        // Attract variant.
        let attract = SdfForceType::Attract {
            strength: Fix128::from_int(5),
            max_force: Fix128::from_int(50),
        };
        let desc = physics_to_force_field_desc(2, &attract, true);
        assert_eq!(desc.force_type_tag, 0, "Attract should have tag 0");
        assert!((desc.strength - 5.0).abs() < 0.01, "strength mismatch");
        assert!(desc.enabled);
        assert_ne!(desc.content_hash, 0);

        // Repel variant.
        let repel = SdfForceType::Repel {
            strength: Fix128::from_int(3),
            range: Fix128::from_int(10),
        };
        let desc_repel = physics_to_force_field_desc(0, &repel, true);
        assert_eq!(desc_repel.force_type_tag, 1, "Repel should have tag 1");
        assert!((desc_repel.influence_distance - 10.0).abs() < 0.01, "range mismatch");

        // SurfaceFlow variant: axis should carry the direction.
        let flow = SdfForceType::SurfaceFlow {
            flow_direction: Vec3Fix::UNIT_Z,
            strength: Fix128::from_ratio(1, 2),
            influence_distance: Fix128::from_int(2),
        };
        let desc_flow = physics_to_force_field_desc(1, &flow, false);
        assert_eq!(desc_flow.force_type_tag, 3, "SurfaceFlow should have tag 3");
        assert!(!desc_flow.enabled);
        assert!((desc_flow.axis[2] - 1.0).abs() < 0.01, "axis Z should be ~1.0");

        // Different variants produce different hashes.
        assert_ne!(desc.content_hash, desc_repel.content_hash);
    }

    // ── Test 3: Physics → View snapshot ─────────────────────────────────

    #[test]
    fn test_physics_to_view_snapshot() {
        let bodies = make_bodies(4);
        let snap = physics_to_view_snapshot(&bodies, 99);

        assert_eq!(snap.positions.len(), 4);
        assert_eq!(snap.orientations.len(), 4);
        assert_eq!(snap.velocities.len(), 4);
        assert_eq!(snap.frame_seq, 99);

        // Active + sleeping must equal body count.
        assert_eq!(
            snap.active_count + snap.sleeping_count, 4,
            "active + sleeping must equal body count"
        );

        // Body 0 velocity=(0,0,0) → sleeping; bodies 1-3 → active.
        assert!(snap.sleeping_count >= 1, "body 0 should be sleeping");
        assert!(snap.active_count >= 3, "bodies 1-3 should be active");

        // Positions reflect from_int values.
        assert!((snap.positions[0][0] - 0.0).abs() < 0.01);
        assert!((snap.positions[1][0] - 1.0).abs() < 0.01);

        // Identity quaternion: w=1, x=y=z=0.
        let [qx, qy, qz, qw] = snap.orientations[0];
        assert!((qw - 1.0).abs() < 0.01, "identity quaternion w should be ~1.0");
        assert!(qx.abs() < 0.01);
        assert!(qy.abs() < 0.01);
        assert!(qz.abs() < 0.01);

        // Hash is non-zero and deterministic.
        assert_ne!(snap.content_hash, 0);
        let snap2 = physics_to_view_snapshot(&bodies, 99);
        assert_eq!(snap.content_hash, snap2.content_hash);
    }

    // ── Test 4: Physics → DB record ──────────────────────────────────────

    #[test]
    fn test_physics_to_db_record() {
        let bodies = make_bodies(8);
        let config = test_config();
        let rec = physics_to_db_record(&bodies, 12, &config);

        assert_eq!(rec.body_count, 8);
        assert_eq!(rec.constraint_count, 12);

        // timestep_secs = (1/60) / substeps — must be small and positive.
        assert!(
            rec.timestep_secs > 0.0 && rec.timestep_secs < 1.0,
            "timestep should be sub-second, got {}",
            rec.timestep_secs
        );

        // Default gravity is downward → Y should be negative.
        assert!(rec.gravity[1] < 0.0, "gravity Y should be negative");

        // Solver iterations match config.iterations.
        assert_eq!(rec.solver_iterations, config.iterations as u32);

        // Determinism checksum is stable across identical calls.
        let rec2 = physics_to_db_record(&bodies, 12, &config);
        assert_eq!(
            rec.determinism_checksum, rec2.determinism_checksum,
            "checksum must be deterministic"
        );
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.content_hash, rec2.content_hash, "hash must be deterministic");
    }

    // ── Test 5: Physics → Cache entry ────────────────────────────────────

    #[test]
    fn test_physics_to_cache_entry() {
        let bodies = make_bodies(16);
        let entry = physics_to_cache_entry(&bodies, 1000);

        assert_eq!(entry.body_count, 16);
        assert_eq!(entry.frame_index, 1000);

        // state_size_bytes = 16 × 128 = 2048.
        assert_eq!(entry.state_size_bytes, 2048, "state size should be body_count * 128");

        // frame_index 1000 fits in u32 → passes through unchanged.
        assert_eq!(entry.eviction_priority, 1000u32);

        // Hash is non-zero and deterministic.
        assert_ne!(entry.content_hash, 0);
        let entry2 = physics_to_cache_entry(&bodies, 1000);
        assert_eq!(entry.content_hash, entry2.content_hash, "hash must be deterministic");

        // Different frame_index → different hash.
        let entry3 = physics_to_cache_entry(&bodies, 1001);
        assert_ne!(entry.content_hash, entry3.content_hash, "different frame should differ");

        // u64::MAX saturates eviction_priority to u32::MAX.
        let big_entry = physics_to_cache_entry(&bodies, u64::MAX);
        assert_eq!(big_entry.eviction_priority, u32::MAX, "should saturate at u32::MAX");
    }

    // ── Test 6: Physics → Analytics metrics ──────────────────────────────

    #[test]
    fn test_physics_to_analytics_metrics() {
        let bodies = make_bodies(6);
        let config = test_config();
        let metrics = physics_to_analytics_metrics(&bodies, &config);

        assert_eq!(metrics.body_count, 6);

        // Active + sleeping must equal total.
        assert_eq!(
            metrics.active_bodies + metrics.sleeping_bodies, 6,
            "active + sleeping must equal body_count"
        );

        // Body 0 velocity=0 → sleeping; bodies 1-5 → active.
        assert!(metrics.sleeping_bodies >= 1, "at least body 0 should be sleeping");
        assert!(metrics.active_bodies >= 5, "bodies 1-5 should be active");

        // KE proxy is positive (sum of v² for active bodies).
        assert!(
            metrics.total_kinetic_energy_proxy > 0.0,
            "total KE proxy should be positive"
        );

        // max_speed_sq: fastest body is body 5 with v=(5,0,0) → 25.0.
        assert!(
            (metrics.max_speed_sq - 25.0).abs() < 0.1,
            "max_speed_sq should be ~25.0, got {}",
            metrics.max_speed_sq
        );

        // sim_frequency_hz = 60 × substeps (default substeps=8 → 480 Hz).
        let expected_hz = 60.0_f32 * config.substeps as f32;
        assert!(
            (metrics.sim_frequency_hz - expected_hz).abs() < 0.1,
            "sim_frequency_hz should be ~{}, got {}",
            expected_hz,
            metrics.sim_frequency_hz
        );

        // constraint_solve_ops = active_bodies × iterations.
        assert_eq!(
            metrics.constraint_solve_ops,
            metrics.active_bodies as u64 * config.iterations as u64
        );

        // Hash is non-zero and deterministic.
        assert_ne!(metrics.content_hash, 0);
        let metrics2 = physics_to_analytics_metrics(&bodies, &config);
        assert_eq!(metrics.content_hash, metrics2.content_hash, "hash must be deterministic");
    }
}
