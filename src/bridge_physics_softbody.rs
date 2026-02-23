//! Soft-body physics bridges — ALICE-Physics (Cloth, Fluid, Rope, Deformable) ↔ Analytics, DB, Cache, Edge, View
//!
//! 6 bridges connecting soft-body simulation subsystems to the ALICE ecosystem.

use alice_physics::{Cloth, DeformableBody, Fix128, Fluid, Rope, Vec3Fix};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Cloth state → Analytics event ────────────────────────────

/// Analytics event for cloth simulation metrics.
///
/// Captures particle count, triangle count, wind influence, and solver
/// configuration for monitoring cloth simulation budget.
pub struct ClothAnalyticsEvent {
    /// Content hash for time-series deduplication.
    pub content_hash: u64,
    /// Number of cloth particles.
    pub particle_count: usize,
    /// Number of triangles in the mesh.
    pub triangle_count: usize,
    /// Number of pinned particles.
    pub pinned_count: usize,
    /// Wind magnitude (scalar).
    pub wind_magnitude: f32,
    /// Whether self-collision is enabled.
    pub self_collision_enabled: bool,
    /// Solver iterations per substep.
    pub iterations: u32,
    /// Number of substeps per step.
    pub substeps: u32,
}

/// Build a `ClothAnalyticsEvent` from a `Cloth` instance.
///
/// Wind magnitude computed as length of wind vector via component squares + sqrt.
#[inline]
#[must_use]
pub fn cloth_to_analytics(cloth: &Cloth) -> ClothAnalyticsEvent {
    let particle_count = cloth.positions.len();
    let triangle_count = cloth.triangles.len();
    let pinned_count = cloth.pinned.len();
    let (wx, wy, wz) = cloth.wind.to_f32();
    let wind_magnitude = (wx * wx + wy * wy + wz * wz).sqrt();

    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(particle_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(triangle_count as u64).to_le_bytes());
    bytes[16..20].copy_from_slice(&wind_magnitude.to_bits().to_le_bytes());
    bytes[20..24].copy_from_slice(&(pinned_count as u32).to_le_bytes());
    let content_hash = fnv1a(&bytes);

    ClothAnalyticsEvent {
        content_hash,
        particle_count,
        triangle_count,
        pinned_count,
        wind_magnitude,
        self_collision_enabled: cloth.config.self_collision,
        iterations: cloth.config.iterations as u32,
        substeps: cloth.config.substeps as u32,
    }
}

// ── Bridge 2: Fluid state → Analytics event ────────────────────────────

/// Analytics event for fluid simulation metrics.
///
/// Captures particle count, density statistics, and solver parameters
/// for monitoring fluid simulation performance.
pub struct FluidAnalyticsEvent {
    /// Content hash for time-series deduplication.
    pub content_hash: u64,
    /// Number of fluid particles.
    pub particle_count: usize,
    /// Average density across all particles (f32 approximation).
    pub avg_density: f32,
    /// Maximum density observed.
    pub max_density: f32,
    /// Rest density target.
    pub rest_density: f32,
    /// Kernel radius.
    pub kernel_radius: f32,
    /// Solver iterations.
    pub iterations: u32,
    /// Number of substeps.
    pub substeps: u32,
}

/// Build a `FluidAnalyticsEvent` from a `Fluid` instance.
///
/// Single pass over densities computes avg and max. Uses reciprocal
/// multiply for averaging to avoid division in the hot path.
#[inline]
#[must_use]
pub fn fluid_to_analytics(fluid: &Fluid) -> FluidAnalyticsEvent {
    let particle_count = fluid.positions.len();
    let mut sum_density = 0.0f32;
    let mut max_density = 0.0f32;

    for d in &fluid.densities {
        let df = d.to_f32();
        sum_density += df;
        max_density = max_density.max(df);
    }

    // Reciprocal multiply for average — avoids division.
    let inv_count = 1.0_f32 / (particle_count.max(1) as f32);
    let avg_density = sum_density * inv_count;

    let rest_density = fluid.config.rest_density.to_f32();
    let kernel_radius = fluid.config.kernel_radius.to_f32();

    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(particle_count as u64).to_le_bytes());
    bytes[8..12].copy_from_slice(&avg_density.to_bits().to_le_bytes());
    bytes[12..16].copy_from_slice(&max_density.to_bits().to_le_bytes());
    bytes[16..20].copy_from_slice(&rest_density.to_bits().to_le_bytes());
    bytes[20..24].copy_from_slice(&kernel_radius.to_bits().to_le_bytes());
    let content_hash = fnv1a(&bytes);

    FluidAnalyticsEvent {
        content_hash,
        particle_count,
        avg_density,
        max_density,
        rest_density,
        kernel_radius,
        iterations: fluid.config.iterations as u32,
        substeps: fluid.config.substeps as u32,
    }
}

// ── Bridge 3: Rope state → DB record ───────────────────────────────────

/// Persistence record for a rope simulation in ALICE-DB.
///
/// Captures rope topology, length, and pin configuration for
/// checkpoint/rollback storage.
pub struct RopeDbRecord {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Number of particles.
    pub particle_count: usize,
    /// Number of segments.
    pub segment_count: usize,
    /// Number of pin constraints.
    pub pin_count: usize,
    /// Total rest length.
    pub total_length: f32,
    /// Current length (sum of segment distances).
    pub current_length: f32,
    /// Length stretch ratio (current / total).
    pub stretch_ratio: f32,
    /// Solver iterations.
    pub iterations: u32,
}

/// Build a `RopeDbRecord` from a `Rope` instance.
///
/// Stretch ratio computed via reciprocal multiply against total length.
#[inline]
#[must_use]
pub fn rope_to_db(rope: &Rope) -> RopeDbRecord {
    let particle_count = rope.particle_count();
    let segment_count = rope.segment_count();
    let pin_count = rope.pins.len();
    let total_length = rope.total_length.to_f32();
    let current_length = rope.current_length().to_f32();

    // Reciprocal multiply for stretch ratio.
    let inv_total = 1.0_f32 / total_length.max(1e-10_f32);
    let stretch_ratio = current_length * inv_total;

    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(particle_count as u64).to_le_bytes());
    bytes[8..12].copy_from_slice(&total_length.to_bits().to_le_bytes());
    bytes[12..16].copy_from_slice(&current_length.to_bits().to_le_bytes());
    bytes[16..20].copy_from_slice(&(pin_count as u32).to_le_bytes());
    bytes[20..24].copy_from_slice(&(segment_count as u32).to_le_bytes());
    let content_hash = fnv1a(&bytes);

    RopeDbRecord {
        content_hash,
        particle_count,
        segment_count,
        pin_count,
        total_length,
        current_length,
        stretch_ratio,
        iterations: rope.config.iterations as u32,
    }
}

// ── Bridge 4: Cloth mesh → Cache entry ─────────────────────────────────

/// Cache entry for a cloth mesh state snapshot.
///
/// Enables memoisation of cloth vertex positions for frame interpolation.
pub struct ClothCacheEntry {
    /// Cache key derived from cloth state.
    pub content_hash: u64,
    /// Number of particles cached.
    pub particle_count: usize,
    /// Number of triangles.
    pub triangle_count: usize,
    /// Estimated memory cost (bytes).
    pub state_size_bytes: usize,
    /// Time-to-live in seconds.
    pub ttl_secs: u32,
}

/// Build a `ClothCacheEntry` from a `Cloth` instance.
///
/// Branchless TTL: cloth with self-collision enabled gets shorter TTL
/// (5s) because state changes more rapidly; without self-collision gets 30s.
#[inline]
#[must_use]
pub fn cloth_to_cache(cloth: &Cloth) -> ClothCacheEntry {
    let particle_count = cloth.positions.len();
    let triangle_count = cloth.triangles.len();

    // Each particle: Vec3Fix position (24B) + velocity (24B) = 48B
    let state_size_bytes = particle_count * 48;

    // Branchless TTL: self-collision → 5s, no self-collision → 30s.
    let has_self_coll = cloth.config.self_collision as u32;
    let ttl_secs = 30 - has_self_coll * 25;

    // Hash: particle count + triangle count + first position XOR.
    let first_hash = cloth
        .positions
        .first()
        .map(|p| p.x.hi as u64)
        .unwrap_or(0);
    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(particle_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(triangle_count as u64).to_le_bytes());
    bytes[16..24].copy_from_slice(&first_hash.to_le_bytes());
    let content_hash = fnv1a(&bytes);

    ClothCacheEntry {
        content_hash,
        particle_count,
        triangle_count,
        state_size_bytes,
        ttl_secs,
    }
}

// ── Bridge 5: Fluid particle count → Edge event ────────────────────────

/// Edge telemetry event for fluid simulation.
///
/// Lightweight struct suitable for IoT/sensor pipeline ingestion.
pub struct FluidEdgeEvent {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Number of fluid particles.
    pub particle_count: usize,
    /// Average density as f32.
    pub avg_density: f32,
    /// Gravity magnitude.
    pub gravity_magnitude: f32,
    /// Time-to-live in seconds.
    pub ttl_secs: u32,
}

/// Build a `FluidEdgeEvent` from a `Fluid` instance.
///
/// Branchless TTL: high particle counts (>10000) get shorter TTL (5s)
/// to avoid stale large entries; small counts get 30s.
#[inline]
#[must_use]
pub fn fluid_to_edge(fluid: &Fluid) -> FluidEdgeEvent {
    let particle_count = fluid.positions.len();

    let mut sum_density = 0.0f32;
    for d in &fluid.densities {
        sum_density += d.to_f32();
    }
    let inv_count = 1.0_f32 / (particle_count.max(1) as f32);
    let avg_density = sum_density * inv_count;

    let (gx, gy, gz) = fluid.config.gravity.to_f32();
    let gravity_magnitude = (gx * gx + gy * gy + gz * gz).sqrt();

    // Branchless TTL: large particle count → 5s, small → 30s.
    let is_large = (particle_count > 10000) as u32;
    let ttl_secs = 30 - is_large * 25;

    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&(particle_count as u64).to_le_bytes());
    bytes[8..12].copy_from_slice(&avg_density.to_bits().to_le_bytes());
    bytes[12..16].copy_from_slice(&gravity_magnitude.to_bits().to_le_bytes());
    let content_hash = fnv1a(&bytes);

    FluidEdgeEvent {
        content_hash,
        particle_count,
        avg_density,
        gravity_magnitude,
        ttl_secs,
    }
}

// ── Bridge 6: Deformable body → View descriptor ────────────────────────

/// View descriptor for rendering a deformable body.
///
/// Converts Fix128 positions to f32 for GPU upload together with
/// surface triangle topology.
pub struct DeformableViewDescriptor {
    /// Content hash for frame deduplication.
    pub content_hash: u64,
    /// Vertex positions as f32 (x, y, z).
    pub positions: Vec<[f32; 3]>,
    /// Surface triangle indices.
    pub surface_triangles: Vec<[usize; 3]>,
    /// Number of tetrahedra (internal mesh complexity indicator).
    pub tetrahedra_count: usize,
    /// Center of mass (f32 approximation).
    pub center_of_mass: [f32; 3],
}

/// Build a `DeformableViewDescriptor` from a `DeformableBody`.
///
/// Single pass converts positions to f32. Center of mass computed
/// via the deformable body's own method.
#[inline]
#[must_use]
pub fn deformable_to_view(body: &DeformableBody) -> DeformableViewDescriptor {
    let positions: Vec<[f32; 3]> = body
        .positions
        .iter()
        .map(|p| {
            let (x, y, z) = p.to_f32();
            [x, y, z]
        })
        .collect();

    let com = body.center_of_mass();
    let (cx, cy, cz) = com.to_f32();

    let tetrahedra_count = body.tetrahedra.len();

    // Hash: particle count + tet count + center of mass.
    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(body.positions.len() as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(tetrahedra_count as u64).to_le_bytes());
    bytes[16..20].copy_from_slice(&cx.to_bits().to_le_bytes());
    bytes[20..24].copy_from_slice(&cy.to_bits().to_le_bytes());
    let content_hash = fnv1a(&bytes);

    DeformableViewDescriptor {
        content_hash,
        positions,
        surface_triangles: body.surface_triangles.clone(),
        tetrahedra_count,
        center_of_mass: [cx, cy, cz],
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_physics::{Cloth, DeformableBody, Fix128, Fluid, FluidConfig, Rope, Vec3Fix};

    // ── Test 1: Cloth → Analytics basic ────────────────────────────────

    #[test]
    fn test_cloth_to_analytics() {
        let cloth = Cloth::new_grid(
            Vec3Fix::ZERO,
            Fix128::from_int(2),
            Fix128::from_int(2),
            4,
            4,
            Fix128::ONE,
        );
        let evt = cloth_to_analytics(&cloth);

        assert_eq!(evt.particle_count, 16); // 4x4 grid
        assert_eq!(evt.triangle_count, 18); // (4-1)*(4-1)*2 = 18
        assert_eq!(evt.pinned_count, 0);
        assert!(!evt.self_collision_enabled);
        assert_ne!(evt.content_hash, 0);
    }

    // ── Test 2: Cloth analytics hash determinism ───────────────────────

    #[test]
    fn test_cloth_analytics_hash_determinism() {
        let cloth = Cloth::new_grid(
            Vec3Fix::ZERO,
            Fix128::from_int(2),
            Fix128::from_int(2),
            3,
            3,
            Fix128::ONE,
        );
        let e1 = cloth_to_analytics(&cloth);
        let e2 = cloth_to_analytics(&cloth);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Test 3: Fluid → Analytics basic ────────────────────────────────

    #[test]
    fn test_fluid_to_analytics() {
        let config = FluidConfig::default();
        let positions = vec![Vec3Fix::ZERO; 100];
        let fluid = Fluid::new(positions, config);
        let evt = fluid_to_analytics(&fluid);

        assert_eq!(evt.particle_count, 100);
        assert_eq!(evt.rest_density, 1000.0); // default rest_density
        assert_ne!(evt.content_hash, 0);
    }

    // ── Test 4: Rope → DB basic ────────────────────────────────────────

    #[test]
    fn test_rope_to_db() {
        let rope = Rope::new(
            Vec3Fix::ZERO,
            Vec3Fix::from_int(10, 0, 0),
            10,
            Fix128::ONE,
        );
        let rec = rope_to_db(&rope);

        assert_eq!(rec.particle_count, 11); // 10 segments + 1
        assert_eq!(rec.segment_count, 10);
        assert_eq!(rec.pin_count, 0);
        assert!((rec.total_length - 10.0).abs() < 0.1);
        // At rest, current_length should equal total_length.
        assert!((rec.stretch_ratio - 1.0).abs() < 0.01);
        assert_ne!(rec.content_hash, 0);
    }

    // ── Test 5: Rope DB hash determinism ───────────────────────────────

    #[test]
    fn test_rope_db_hash_determinism() {
        let rope = Rope::new(
            Vec3Fix::ZERO,
            Vec3Fix::from_int(5, 0, 0),
            5,
            Fix128::ONE,
        );
        let r1 = rope_to_db(&rope);
        let r2 = rope_to_db(&rope);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Test 6: Cloth → Cache with self-collision (short TTL) ──────────

    #[test]
    fn test_cloth_cache_self_collision_ttl() {
        let mut cloth = Cloth::new_grid(
            Vec3Fix::ZERO,
            Fix128::from_int(2),
            Fix128::from_int(2),
            4,
            4,
            Fix128::ONE,
        );
        cloth.config.self_collision = true;
        let entry = cloth_to_cache(&cloth);
        assert_eq!(entry.ttl_secs, 5, "self-collision cloth should get 5s TTL");
        assert_ne!(entry.content_hash, 0);
    }

    // ── Test 7: Cloth → Cache without self-collision (long TTL) ────────

    #[test]
    fn test_cloth_cache_no_self_collision_ttl() {
        let cloth = Cloth::new_grid(
            Vec3Fix::ZERO,
            Fix128::from_int(2),
            Fix128::from_int(2),
            4,
            4,
            Fix128::ONE,
        );
        let entry = cloth_to_cache(&cloth);
        assert_eq!(entry.ttl_secs, 30, "no self-collision should get 30s TTL");
    }

    // ── Test 8: Fluid → Edge basic ─────────────────────────────────────

    #[test]
    fn test_fluid_to_edge() {
        let config = FluidConfig::default();
        let positions = vec![Vec3Fix::ZERO; 50];
        let fluid = Fluid::new(positions, config);
        let evt = fluid_to_edge(&fluid);

        assert_eq!(evt.particle_count, 50);
        assert_eq!(evt.ttl_secs, 30, "small fluid should get 30s TTL");
        assert!(evt.gravity_magnitude > 9.0); // gravity ~10 m/s^2
        assert_ne!(evt.content_hash, 0);
    }

    // ── Test 9: Deformable → View basic ────────────────────────────────

    #[test]
    fn test_deformable_to_view() {
        let body = DeformableBody::new_cube(Vec3Fix::ZERO, Fix128::ONE, Fix128::ONE);
        let view = deformable_to_view(&body);

        assert_eq!(view.positions.len(), 8); // cube has 8 vertices
        assert_eq!(view.tetrahedra_count, 5); // cube decomposed into 5 tets
        assert!(!view.surface_triangles.is_empty());
        assert_ne!(view.content_hash, 0);
    }

    // ── Test 10: Deformable view hash determinism ──────────────────────

    #[test]
    fn test_deformable_view_hash_determinism() {
        let body = DeformableBody::new_cube(Vec3Fix::ZERO, Fix128::ONE, Fix128::ONE);
        let v1 = deformable_to_view(&body);
        let v2 = deformable_to_view(&body);
        assert_eq!(v1.content_hash, v2.content_hash);
    }
}
