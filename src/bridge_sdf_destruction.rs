//! SDF Destruction bridges — ALICE-SDF Destruction ↔ DB, View, Cache, Analytics, Physics
//!
//! 5 bridges connecting real-time destructible environment data from
//! ALICE-SDF to the ALICE ecosystem. Covers destruction event persistence,
//! visual feedback, cache invalidation, analytics, and physics collision updates.

use alice_sdf::destruction::{CarveShape, DestructionResult, FracturePiece};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: DestructionResult → DB (destruction event record) ───────

/// DB record for a destruction event.
///
/// Persists the outcome of a carve/fracture operation so that destruction
/// state can be replayed, versioned, and synced across network sessions.
pub struct SdfDestructionDbRecord {
    /// FNV-1a hash of the destruction event content.
    pub content_hash: u64,
    /// Number of dirty chunks affected by the destruction.
    pub dirty_chunk_count: u32,
    /// Approximate volume of removed material (world units cubed).
    pub removed_volume: f32,
    /// Number of voxels modified.
    pub modified_voxels: u32,
    /// Schema version for migration.
    pub schema_version: u16,
    /// Event type: 0=carve, `1=batch_carve`, 2=explode, 3=fracture.
    pub event_type: u8,
}

/// Build a DB record from a `DestructionResult` for ALICE-DB.
#[inline]
#[must_use]
pub fn sdf_destruction_to_db_record(
    result: &DestructionResult,
    event_type: u8,
    schema_version: u16,
) -> SdfDestructionDbRecord {
    let dirty_chunk_count = result.dirty_chunks.len() as u32;
    // Hash: modified_voxels (4) + removed_volume bits (4) + event_type (1) = 9 bytes
    let mut data = [0u8; 9];
    data[0..4].copy_from_slice(&result.modified_voxels.to_le_bytes());
    data[4..8].copy_from_slice(&result.removed_volume.to_le_bytes());
    data[8] = event_type;
    let content_hash = fnv1a(&data);

    SdfDestructionDbRecord {
        content_hash,
        dirty_chunk_count,
        removed_volume: result.removed_volume,
        modified_voxels: result.modified_voxels,
        schema_version,
        event_type,
    }
}

// ── Bridge 2: CarveShape → View (destruction shape visualization) ─────

/// View descriptor for a carve shape visualization overlay.
///
/// Provides ALICE-View with the parameters to render a wireframe or
/// semi-transparent preview of the destruction shape before confirmation.
pub struct SdfDestructionViewDescriptor {
    /// FNV-1a hash of the carve shape content.
    pub content_hash: u64,
    /// Shape type: 0=sphere, 1=box, `2=arbitrary_sdf`.
    pub shape_type: u8,
    /// Bounding radius estimate for camera framing.
    pub bounding_radius: f32,
    /// Center position (for sphere/box shapes; zero for arbitrary SDF).
    pub center: [f32; 3],
    /// Recommended wireframe line width (scaled by bounding radius).
    pub wireframe_width: f32,
}

/// Build a view descriptor from a `CarveShape` for ALICE-View.
#[inline]
#[must_use]
pub fn sdf_carve_shape_to_view_descriptor(shape: &CarveShape) -> SdfDestructionViewDescriptor {
    let (shape_type, bounding_radius, center) = match shape {
        CarveShape::Sphere { center, radius } => (0u8, *radius, [center.x, center.y, center.z]),
        CarveShape::Box {
            center,
            half_extents,
            ..
        } => {
            let radius = half_extents.length();
            (1u8, radius, [center.x, center.y, center.z])
        }
        CarveShape::Sdf(_node) => {
            // Arbitrary SDF: conservative bounding radius, zero center
            (2u8, 10.0, [0.0, 0.0, 0.0])
        }
    };

    // Hash: shape_type (1) + bounding_radius (4) + center (12) = 17 bytes
    let mut data = [0u8; 17];
    data[0] = shape_type;
    data[1..5].copy_from_slice(&bounding_radius.to_le_bytes());
    data[5..9].copy_from_slice(&center[0].to_le_bytes());
    data[9..13].copy_from_slice(&center[1].to_le_bytes());
    data[13..17].copy_from_slice(&center[2].to_le_bytes());
    let content_hash = fnv1a(&data);

    let wireframe_width = (bounding_radius * 0.01).clamp(0.001, 0.1);

    SdfDestructionViewDescriptor {
        content_hash,
        shape_type,
        bounding_radius,
        center,
        wireframe_width,
    }
}

// ── Bridge 3: DestructionResult → Cache (invalidation entry) ──────────

/// Cache invalidation entry for destruction events.
///
/// When a carve modifies voxels, previously cached mesh chunks must be
/// invalidated. This entry lists the affected chunk coordinates and
/// provides a branchless TTL: large destructions (many dirty chunks)
/// get a shorter TTL to trigger faster re-caching.
pub struct SdfDestructionCacheInvalidation {
    /// FNV-1a hash of the destruction event — invalidation key.
    pub content_hash: u64,
    /// Number of chunks to invalidate.
    pub invalidated_chunk_count: u32,
    /// Number of modified voxels.
    pub modified_voxels: u32,
    /// Cache TTL in seconds for the re-cached data.
    pub ttl_secs: u32,
    /// True when the destruction affected more than 8 chunks (major event).
    pub is_major_destruction: bool,
}

/// Build a cache invalidation entry from a `DestructionResult` for ALICE-Cache.
///
/// Branchless TTL: >8 dirty chunks → short TTL (5s), <=8 → long TTL (60s).
#[inline]
#[must_use]
pub fn sdf_destruction_to_cache_invalidation(
    result: &DestructionResult,
) -> SdfDestructionCacheInvalidation {
    let dirty_chunk_count = result.dirty_chunks.len() as u32;
    let mut data = [0u8; 8];
    data[0..4].copy_from_slice(&result.modified_voxels.to_le_bytes());
    data[4..8].copy_from_slice(&dirty_chunk_count.to_le_bytes());
    let content_hash = fnv1a(&data);

    // Branchless TTL: large destruction (>8 chunks) → 5s, small → 60s
    let is_major = (dirty_chunk_count > 8) as u32;
    let ttl_secs = 60 - is_major * 55;

    SdfDestructionCacheInvalidation {
        content_hash,
        invalidated_chunk_count: dirty_chunk_count,
        modified_voxels: result.modified_voxels,
        ttl_secs,
        is_major_destruction: dirty_chunk_count > 8,
    }
}

// ── Bridge 4: DestructionResult → Analytics (destruction event) ───────

/// Analytics event for destruction telemetry.
///
/// Tracks destruction frequency, scale, and shape type to inform
/// level design balance and performance budgeting.
pub struct SdfDestructionAnalyticsEvent {
    /// FNV-1a hash of the destruction event.
    pub content_hash: u64,
    /// Number of dirty chunks.
    pub dirty_chunk_count: u32,
    /// Approximate removed volume (world units cubed).
    pub removed_volume: f32,
    /// Number of modified voxels.
    pub modified_voxels: u32,
    /// Destruction intensity: `removed_volume` / `max(modified_voxels`, 1) — higher = denser removal.
    pub destruction_intensity: f32,
    /// True when no material was actually removed (miss / out-of-bounds).
    pub is_no_op: bool,
}

/// Build an analytics event from a `DestructionResult` for ALICE-Analytics.
#[inline]
#[must_use]
pub fn sdf_destruction_to_analytics_event(
    result: &DestructionResult,
) -> SdfDestructionAnalyticsEvent {
    let dirty_chunk_count = result.dirty_chunks.len() as u32;
    let mut data = [0u8; 12];
    data[0..4].copy_from_slice(&result.modified_voxels.to_le_bytes());
    data[4..8].copy_from_slice(&result.removed_volume.to_le_bytes());
    data[8..12].copy_from_slice(&dirty_chunk_count.to_le_bytes());
    let content_hash = fnv1a(&data);

    let destruction_intensity = result.removed_volume / (result.modified_voxels.max(1) as f32);
    let is_no_op = result.modified_voxels == 0;

    SdfDestructionAnalyticsEvent {
        content_hash,
        dirty_chunk_count,
        removed_volume: result.removed_volume,
        modified_voxels: result.modified_voxels,
        destruction_intensity,
        is_no_op,
    }
}

// ── Bridge 5: FracturePiece → Physics (collision update) ──────────────

/// Physics collision update for a fracture piece.
///
/// When Voronoi fracture produces debris pieces, each piece needs a
/// collision shape registered in ALICE-Physics. This bridge computes
/// the collision parameters from the fracture piece geometry.
pub struct SdfFracturePhysicsUpdate {
    /// FNV-1a hash of the fracture piece content.
    pub content_hash: u64,
    /// Voronoi cell index.
    pub cell_index: u32,
    /// Center of mass for the piece.
    pub center: [f32; 3],
    /// Voxel count (used for mass estimation).
    pub voxel_count: u32,
    /// Estimated mass (`voxel_count` * `voxel_volume` * density).
    pub estimated_mass_kg: f32,
    /// Collider type: always convex hull (3) for fracture pieces.
    pub collider_type: u8,
    /// Vertex count in the piece mesh (for convex hull complexity budgeting).
    pub vertex_count: u32,
}

/// Build a physics collision update from a `FracturePiece` for ALICE-Physics.
///
/// `voxel_volume` is the volume of a single voxel in world units cubed.
/// `density_kg_per_m3` is the material density (default: 1000.0 for water-like).
#[inline]
#[must_use]
pub fn sdf_fracture_to_physics_update(
    piece: &FracturePiece,
    voxel_volume: f32,
    density_kg_per_m3: f32,
) -> SdfFracturePhysicsUpdate {
    // Hash: cell_index (4) + voxel_count (4) + center x,y,z (12) = 20 bytes
    let mut data = [0u8; 20];
    data[0..4].copy_from_slice(&piece.cell_index.to_le_bytes());
    data[4..8].copy_from_slice(&piece.voxel_count.to_le_bytes());
    data[8..12].copy_from_slice(&piece.center.x.to_le_bytes());
    data[12..16].copy_from_slice(&piece.center.y.to_le_bytes());
    data[16..20].copy_from_slice(&piece.center.z.to_le_bytes());
    let content_hash = fnv1a(&data);

    let estimated_mass_kg = piece.voxel_count as f32 * voxel_volume * density_kg_per_m3;
    let vertex_count = piece.mesh.vertices.len() as u32;

    SdfFracturePhysicsUpdate {
        content_hash,
        cell_index: piece.cell_index,
        center: [piece.center.x, piece.center.y, piece.center.z],
        voxel_count: piece.voxel_count,
        estimated_mass_kg,
        collider_type: 3, // convex hull
        vertex_count,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_sdf::prelude::{Quat, Vec3};

    fn sample_destruction_result(modified: u32, volume: f32, chunks: u32) -> DestructionResult {
        let dirty_chunks: Vec<[u32; 3]> = (0..chunks).map(|i| [i, 0, 0]).collect();
        DestructionResult {
            dirty_chunks,
            removed_volume: volume,
            modified_voxels: modified,
        }
    }

    // -- Bridge 1 tests --

    #[test]
    fn test_destruction_to_db_record() {
        let result = sample_destruction_result(100, 1.5, 3);
        let rec = sdf_destruction_to_db_record(&result, 0, 1);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.dirty_chunk_count, 3);
        assert_eq!(rec.modified_voxels, 100);
        assert!((rec.removed_volume - 1.5).abs() < f32::EPSILON);
        assert_eq!(rec.event_type, 0);
        assert_eq!(rec.schema_version, 1);
    }

    #[test]
    fn test_destruction_to_db_record_different_types_differ() {
        let result = sample_destruction_result(50, 1.0, 2);
        let r0 = sdf_destruction_to_db_record(&result, 0, 1);
        let r1 = sdf_destruction_to_db_record(&result, 1, 1);
        assert_ne!(
            r0.content_hash, r1.content_hash,
            "different event_type → different hash"
        );
    }

    // -- Bridge 2 tests --

    #[test]
    fn test_carve_shape_to_view_sphere() {
        let shape = CarveShape::Sphere {
            center: Vec3::new(1.0, 2.0, 3.0),
            radius: 0.5,
        };
        let desc = sdf_carve_shape_to_view_descriptor(&shape);
        assert_ne!(desc.content_hash, 0);
        assert_eq!(desc.shape_type, 0);
        assert!((desc.bounding_radius - 0.5).abs() < f32::EPSILON);
        assert!((desc.center[0] - 1.0).abs() < f32::EPSILON);
        assert!(desc.wireframe_width > 0.0);
    }

    #[test]
    fn test_carve_shape_to_view_box() {
        let shape = CarveShape::Box {
            center: Vec3::ZERO,
            half_extents: Vec3::new(1.0, 1.0, 1.0),
            rotation: Quat::IDENTITY,
        };
        let desc = sdf_carve_shape_to_view_descriptor(&shape);
        assert_eq!(desc.shape_type, 1);
        // half_extents (1,1,1) → length = sqrt(3) ~ 1.732
        assert!(desc.bounding_radius > 1.7);
    }

    // -- Bridge 3 tests --

    #[test]
    fn test_destruction_to_cache_invalidation_small() {
        let result = sample_destruction_result(20, 0.5, 3);
        let inv = sdf_destruction_to_cache_invalidation(&result);
        assert_ne!(inv.content_hash, 0);
        assert_eq!(inv.invalidated_chunk_count, 3);
        assert_eq!(inv.ttl_secs, 60, "small destruction → long TTL");
        assert!(!inv.is_major_destruction);
    }

    #[test]
    fn test_destruction_to_cache_invalidation_large() {
        let result = sample_destruction_result(500, 10.0, 12);
        let inv = sdf_destruction_to_cache_invalidation(&result);
        assert_eq!(inv.ttl_secs, 5, "large destruction → short TTL");
        assert!(inv.is_major_destruction);
    }

    // -- Bridge 4 tests --

    #[test]
    fn test_destruction_to_analytics_event() {
        let result = sample_destruction_result(100, 2.5, 4);
        let event = sdf_destruction_to_analytics_event(&result);
        assert_ne!(event.content_hash, 0);
        assert_eq!(event.dirty_chunk_count, 4);
        assert_eq!(event.modified_voxels, 100);
        assert!((event.removed_volume - 2.5).abs() < f32::EPSILON);
        assert!(!event.is_no_op);
        assert!(event.destruction_intensity > 0.0);
    }

    #[test]
    fn test_destruction_to_analytics_event_no_op() {
        let result = sample_destruction_result(0, 0.0, 0);
        let event = sdf_destruction_to_analytics_event(&result);
        assert!(event.is_no_op);
    }

    // -- Bridge 5 tests --

    #[test]
    fn test_fracture_to_physics_update() {
        use alice_sdf::mesh::{Mesh, Vertex};
        let piece = FracturePiece {
            cell_index: 2,
            center: Vec3::new(0.5, 0.5, 0.5),
            mesh: Mesh {
                vertices: vec![Vertex::default(); 30],
                indices: vec![0; 90],
            },
            voxel_count: 100,
        };
        let update = sdf_fracture_to_physics_update(&piece, 0.001, 1000.0);
        assert_ne!(update.content_hash, 0);
        assert_eq!(update.cell_index, 2);
        assert_eq!(update.voxel_count, 100);
        assert_eq!(update.vertex_count, 30);
        assert_eq!(update.collider_type, 3);
        // mass = 100 * 0.001 * 1000 = 100 kg
        assert!((update.estimated_mass_kg - 100.0).abs() < f32::EPSILON);
    }

    // -- Hash determinism --

    #[test]
    fn test_hash_determinism() {
        let result = sample_destruction_result(50, 1.0, 3);
        let h1 = sdf_destruction_to_db_record(&result, 0, 1).content_hash;
        let h2 = sdf_destruction_to_db_record(&result, 0, 1).content_hash;
        assert_eq!(h1, h2, "same input → same hash");
    }
}
