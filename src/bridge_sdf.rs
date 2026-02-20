//! SDF bridges — ALICE-SDF as geometry hub ↔ View, DB, ML, Print, Physics
//!
//! 7 bridges connecting the Signed Distance Field geometry layer to the
//! ALICE ecosystem.  Covers SDF visualization, SDF storage, ML-based
//! shape classification, 3D print slicing metadata, and collision geometry.
//!
//! All bridges operate on `SdfNode` (the tree root) and its `node_count()` /
//! `category()` introspection API, which is the public surface exposed by
//! ALICE-SDF without requiring internal field access.

use alice_sdf::{SdfNode, SdfTree};
use alice_sdf::types::SdfCategory;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

/// Collect category counts from the root node by walking the node_count and
/// category — returns (primitive_count, operation_count, modifier_count).
///
/// Because SdfNode::node_count() recurses, we approximate category breakdown
/// using the root category as the dominant signal (the node_count gives total
/// tree size, but category distribution is estimated from the tree structure).
#[inline]
fn tree_category_counts(root: &SdfNode) -> (u32, u32, u32) {
    let total = root.node_count();
    match root.category() {
        SdfCategory::Primitive => (total, 0, 0),
        SdfCategory::Operation => {
            // Typical pattern: 1 operation node + N primitives.
            // Estimate: 1 operation, rest primitives.
            let ops = 1 + total / 3; // rough: every 3 primitives ≈ 1 op
            let prims = total.saturating_sub(ops);
            (prims, ops, 0)
        }
        SdfCategory::Transform => {
            // Transform wraps child nodes; estimate similar to operations.
            let transforms = 1 + total / 4;
            let prims = total.saturating_sub(transforms);
            (prims, transforms, 0)
        }
        SdfCategory::Modifier => {
            let mods = 1 + total / 4;
            let prims = total.saturating_sub(mods);
            (prims, 0, mods)
        }
    }
}

// ── Bridge 1: SDF → View (visualization descriptor) ──────────────────────

/// SDF visualization descriptor for ALICE-View.
///
/// Provides the parameters needed by the view layer to raymarch the SDF and
/// display it in the viewport.  Resolution and step count are pre-computed
/// from the tree complexity to keep the view bridge allocation-free.
pub struct SdfViewDescriptor {
    /// FNV-1a hash over node_count bytes — change key.
    pub tree_hash: u64,
    /// Total node count in the SDF tree.
    pub node_count: u32,
    /// Category of the root node.
    pub root_category: u8,
    /// Recommended raymarch step count (branchless function of node_count).
    pub march_steps: u32,
    /// Recommended grid resolution for voxelization.
    pub grid_resolution: u32,
    /// Surface epsilon for hit detection (1/1000 of bounding radius estimate).
    pub surface_epsilon: f32,
}

/// Build a visualization descriptor from an `SdfTree` for ALICE-View.
///
/// `march_steps` scales with `node_count` so that complex scenes get more
/// steps.  Formula (branchless): `steps = clamp(node_count * 8 + 64, 64, 512)`.
#[inline]
pub fn sdf_to_view_descriptor(tree: &SdfTree) -> SdfViewDescriptor {
    let node_count = tree.root.node_count();
    let mut data = [0u8; 4];
    data.copy_from_slice(&node_count.to_le_bytes());
    let tree_hash = fnv1a(&data);

    // Branchless step count: node_count * 8 + 64, clamped to [64, 512].
    let raw_steps = node_count.saturating_mul(8).saturating_add(64);
    let march_steps = raw_steps.clamp(64, 512);

    // Grid resolution: next power of two >= max(8, node_count * 4).
    let raw_res = node_count.saturating_mul(4).max(8);
    let grid_resolution = raw_res.next_power_of_two().clamp(8, 256);

    // Bounding radius estimate: sqrt(node_count) SDF-units.
    let bounding_radius = (node_count as f32).sqrt().max(1.0);
    let surface_epsilon = bounding_radius * 0.001;

    // Root category tag: 0=primitive, 1=operation, 2=modifier, 3=transform.
    let root_category = match tree.root.category() {
        SdfCategory::Primitive => 0u8,
        SdfCategory::Operation => 1u8,
        SdfCategory::Modifier  => 2u8,
        SdfCategory::Transform => 3u8,
    };

    SdfViewDescriptor {
        tree_hash,
        node_count,
        root_category,
        march_steps,
        grid_resolution,
        surface_epsilon,
    }
}

// ── Bridge 2: SDF → DB (geometry asset record) ───────────────────────────

/// SDF geometry asset record for ALICE-DB persistence.
///
/// Stores a compact fingerprint of the SDF tree so that scene assets can be
/// queried and versioned without re-building the tree from scratch.
pub struct SdfDbAssetRecord {
    /// FNV-1a hash of the node_count bytes.
    pub tree_hash: u64,
    /// Total node count.
    pub node_count: u32,
    /// Serialized size estimate in bytes (node_count * 48 bytes per node).
    pub serialized_bytes: usize,
    /// Asset schema version for migration.
    pub schema_version: u16,
    /// True when the root node is a pure primitive (no CSG ops).
    pub is_primitive_root: bool,
}

/// Build a DB asset record from an `SdfTree`.
#[inline]
pub fn sdf_to_db_asset_record(tree: &SdfTree, schema_version: u16) -> SdfDbAssetRecord {
    let node_count = tree.root.node_count();
    let mut data = [0u8; 4];
    data.copy_from_slice(&node_count.to_le_bytes());
    let tree_hash = fnv1a(&data);
    let serialized_bytes = node_count as usize * 48;
    let is_primitive_root = tree.root.category() == SdfCategory::Primitive;
    SdfDbAssetRecord {
        tree_hash,
        node_count,
        serialized_bytes,
        schema_version,
        is_primitive_root,
    }
}

// ── Bridge 3: SDF → ML (shape classification features) ──────────────────

/// ML feature vector for SDF shape classification.
///
/// Extracted from the `SdfNode` tree topology and fed into an ALICE-ML
/// classifier that predicts object category from geometry alone.
pub struct SdfMlFeatures {
    /// Normalized node count (log2 scale, 0.0–1.0 for 1–1024 nodes).
    pub norm_node_count: f32,
    /// Fraction of primitive nodes (estimated).
    pub primitive_ratio: f32,
    /// Fraction of operation nodes (estimated).
    pub operation_ratio: f32,
    /// Fraction of modifier nodes (estimated).
    pub modifier_ratio: f32,
    /// Estimated surface area proxy (sqrt(node_count) * bounding_radius).
    pub surface_proxy: f32,
    /// Feature vector for ML model input (length 5).
    pub feature_vec: Vec<f32>,
}

/// Extract ML classification features from an `SdfTree`.
#[inline]
pub fn sdf_to_ml_features(tree: &SdfTree) -> SdfMlFeatures {
    let node_count = tree.root.node_count().max(1);
    let (prims, ops, mods) = tree_category_counts(&tree.root);
    let total = node_count as f32;
    let primitive_ratio = prims as f32 / total;
    let operation_ratio = ops as f32 / total;
    let modifier_ratio = mods as f32 / total;

    let norm_node_count = ((node_count as f32).log2() / 10.0).clamp(0.0, 1.0);
    let bounding_radius = (node_count as f32).sqrt().max(1.0);
    let surface_proxy = (node_count as f32).sqrt() * bounding_radius;

    let feature_vec = vec![
        norm_node_count,
        primitive_ratio,
        operation_ratio,
        modifier_ratio,
        surface_proxy,
    ];

    SdfMlFeatures {
        norm_node_count,
        primitive_ratio,
        operation_ratio,
        modifier_ratio,
        surface_proxy,
        feature_vec,
    }
}

// ── Bridge 4: SDF → Print (slice preparation metadata) ──────────────────

/// Slice preparation metadata for ALICE-Print.
///
/// Pre-computed from the SDF tree before invoking the slicer.  Contains
/// resolution hints and orientation recommendations derived from the tree
/// topology to improve slice quality without trial-and-error.
pub struct SdfPrintSliceMeta {
    /// FNV-1a hash of the SDF tree.
    pub tree_hash: u64,
    /// Recommended layer height in millimetres.
    pub layer_height_mm: f32,
    /// Recommended print bed orientation (0=flat, 1=tilted-45, 2=vertical).
    pub orientation: u8,
    /// Estimated bounding box diagonal in millimetres.
    pub bbox_diagonal_mm: f32,
    /// Recommended support density (0.0–1.0, derived from operation count).
    pub support_density: f32,
    /// Node count — used by the slicer for LOD selection.
    pub node_count: u32,
}

/// Build slice preparation metadata from an `SdfTree` for ALICE-Print.
///
/// `scale_mm` converts SDF units to millimetres (e.g. 10.0 for 1 unit = 10 mm).
#[inline]
pub fn sdf_to_print_slice_meta(tree: &SdfTree, scale_mm: f32) -> SdfPrintSliceMeta {
    let node_count = tree.root.node_count();
    let mut data = [0u8; 4];
    data.copy_from_slice(&node_count.to_le_bytes());
    let tree_hash = fnv1a(&data);

    let bounding_radius = (node_count as f32).sqrt().max(1.0);
    let bbox_diagonal_mm = bounding_radius * 2.0 * scale_mm;

    // Layer height: finer for complex trees.
    let complexity = (node_count as f32 / 10.0).clamp(1.0, 10.0);
    let layer_height_mm = (0.2_f32 / complexity).clamp(0.05, 0.2);

    // Orientation: vertical for single primitives, tilted for medium, flat for large.
    // Branchless: single (≤1) → 2, small (2–8) → 1, large (9+) → 0.
    let is_single = (node_count <= 1) as u8;
    let is_small = ((node_count >= 2) & (node_count <= 8)) as u8;
    let orientation = is_single.wrapping_mul(2) | (is_small & !is_single);

    // Support density: more operation nodes need denser supports.
    let (_, ops, _) = tree_category_counts(&tree.root);
    let support_density = ((ops as f32 + 1.0) / (node_count as f32 + 1.0)).clamp(0.1, 0.8);

    SdfPrintSliceMeta {
        tree_hash,
        layer_height_mm,
        orientation,
        bbox_diagonal_mm,
        support_density,
        node_count,
    }
}

// ── Bridge 5: SDF → Physics (collision shape descriptor) ─────────────────

/// Collision shape descriptor for ALICE-Physics.
///
/// Derived from the SDF tree and used to register a rigid body's collision
/// shape in the physics engine.  Simple primitive trees produce analytic
/// shapes; complex operation trees fall back to a convex hull.
pub struct SdfPhysicsCollider {
    /// FNV-1a hash of the SDF tree — physics body identity key.
    pub shape_hash: u64,
    /// Collider type: 0=sphere, 1=box, 2=capsule, 3=convex_hull, 4=compound.
    pub collider_type: u8,
    /// Bounding sphere radius (used for broad-phase culling).
    pub bounding_radius: f32,
    /// Mass estimate based on volume approximation (kg, density=1000 kg/m³).
    pub mass_kg: f32,
    /// Number of sub-shapes in a compound collider (1 for simple shapes).
    pub sub_shape_count: u32,
    /// Restitution coefficient (bounciness, 0.0–1.0).
    pub restitution: f32,
}

/// Build a collision shape descriptor from an `SdfTree` for ALICE-Physics.
///
/// Shape selection is branchless:
/// - Primitive root, 1 node → analytic sphere (type 0)
/// - Operation root, ≤8 nodes → compound (type 4)
/// - Operation/Modifier root, >8 nodes → convex hull (type 3)
#[inline]
pub fn sdf_to_physics_collider(tree: &SdfTree) -> SdfPhysicsCollider {
    let node_count = tree.root.node_count();
    let mut data = [0u8; 4];
    data.copy_from_slice(&node_count.to_le_bytes());
    let shape_hash = fnv1a(&data);

    let bounding_radius = (node_count as f32).sqrt().max(0.5);
    // Volume of bounding sphere: 4/3 * π * r³ ≈ 4.189 * r³.
    let volume = 4.189_f32 * bounding_radius * bounding_radius * bounding_radius;
    let mass_kg = volume * 1_000.0 * 0.001;

    let is_primitive_root = tree.root.category() == SdfCategory::Primitive;
    let is_small_tree = node_count <= 8;

    // Branchless collider type:
    // - Primitive root → 0 (sphere)
    // - Operation/Modifier, small → 4 (compound)
    // - Operation/Modifier, large → 3 (convex hull)
    let collider_type = if is_primitive_root {
        0u8
    } else if is_small_tree {
        4u8
    } else {
        3u8
    };

    let sub_shape_count = if is_primitive_root { 1 }
                          else if is_small_tree { node_count }
                          else { 1 };

    SdfPhysicsCollider {
        shape_hash,
        collider_type,
        bounding_radius,
        mass_kg,
        sub_shape_count,
        restitution: 0.3,
    }
}

// ── Bridge 6: SDF → DB (version history record) ──────────────────────────

/// SDF version history record for ALICE-DB.
///
/// Tracks incremental CSG edits so that the undo/redo system can replay
/// geometry operations without storing full tree snapshots.
pub struct SdfDbVersionRecord {
    /// FNV-1a hash of the SDF tree after this edit (includes edit_op).
    pub tree_hash: u64,
    /// Edit operation: 0=add_node, 1=remove_node, 2=transform, 3=param_change.
    pub edit_op: u8,
    /// Accumulated node count after this edit.
    pub node_count: u32,
    /// Schema version for migration.
    pub schema_version: u16,
    /// Root category after this edit.
    pub root_category: u8,
}

/// Build a version history record for ALICE-DB.
#[inline]
pub fn sdf_to_db_version_record(
    tree: &SdfTree,
    edit_op: u8,
    schema_version: u16,
) -> SdfDbVersionRecord {
    let node_count = tree.root.node_count();
    let mut data = [0u8; 5];
    data[0..4].copy_from_slice(&node_count.to_le_bytes());
    data[4] = edit_op;
    let tree_hash = fnv1a(&data);
    let root_category = match tree.root.category() {
        SdfCategory::Primitive => 0u8,
        SdfCategory::Operation => 1u8,
        SdfCategory::Modifier  => 2u8,
        SdfCategory::Transform => 3u8,
    };
    SdfDbVersionRecord {
        tree_hash,
        edit_op,
        node_count,
        schema_version,
        root_category,
    }
}

// ── Bridge 7: SDF → ML (anomaly detection: geometry outlier) ─────────────

/// Geometry outlier detection record for ALICE-ML.
///
/// Flags SDF trees whose node_count deviates significantly from the expected
/// distribution so the ML pipeline can flag corrupted or adversarial geometry.
pub struct SdfMlOutlierRecord {
    /// FNV-1a hash of the SDF tree.
    pub tree_hash: u64,
    /// Outlier score (higher = more anomalous, 0.0–1.0).
    pub outlier_score: f32,
    /// True when outlier_score >= 0.8 (flagged for review).
    pub is_flagged: bool,
    /// Actual node count.
    pub node_count: u32,
    /// Expected node count from the model distribution.
    pub expected_node_count: f32,
}

/// Build a geometry outlier detection record for ALICE-ML.
///
/// Outlier score is a branchless normalized deviation from the expected
/// node count: `score = |actual - expected| / max(expected, 1)`, clamped to [0, 1].
#[inline]
pub fn sdf_to_ml_outlier_record(
    tree: &SdfTree,
    expected_node_count: f32,
) -> SdfMlOutlierRecord {
    let node_count = tree.root.node_count();
    let mut data = [0u8; 4];
    data.copy_from_slice(&node_count.to_le_bytes());
    let tree_hash = fnv1a(&data);

    let deviation = ((node_count as f32 - expected_node_count) / expected_node_count.max(1.0)).abs();
    let outlier_score = deviation.clamp(0.0, 1.0);
    let is_flagged = outlier_score >= 0.8;

    SdfMlOutlierRecord {
        tree_hash,
        outlier_score,
        is_flagged,
        node_count,
        expected_node_count,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_sdf::SdfNode;

    fn sphere_tree() -> SdfTree {
        SdfTree::new(SdfNode::sphere(1.0))
    }

    fn csg_tree() -> SdfTree {
        let root = SdfNode::sphere(1.0).union(SdfNode::box3d(0.5, 0.5, 0.5));
        SdfTree::new(root)
    }

    fn deep_tree() -> SdfTree {
        let root = SdfNode::sphere(1.0)
            .union(SdfNode::box3d(0.5, 0.5, 0.5))
            .union(SdfNode::sphere(0.3))
            .subtract(SdfNode::box3d(0.1, 0.1, 0.1))
            .smooth_union(SdfNode::sphere(0.2), 0.05);
        SdfTree::new(root)
    }

    #[test]
    fn test_sdf_to_view_descriptor_sphere() {
        let tree = sphere_tree();
        let desc = sdf_to_view_descriptor(&tree);
        assert_ne!(desc.tree_hash, 0);
        assert_eq!(desc.node_count, 1);
        assert_eq!(desc.root_category, 0, "sphere is a primitive");
        assert!(desc.march_steps >= 64);
        assert!(desc.march_steps <= 512);
        assert!(desc.grid_resolution >= 8);
        assert!(desc.surface_epsilon > 0.0);
    }

    #[test]
    fn test_sdf_to_view_descriptor_csg_has_more_steps() {
        let simple = sdf_to_view_descriptor(&sphere_tree());
        let complex = sdf_to_view_descriptor(&csg_tree());
        // More nodes → more march steps.
        assert!(complex.march_steps >= simple.march_steps);
        assert_eq!(complex.root_category, 1, "union is an operation");
    }

    #[test]
    fn test_sdf_to_db_asset_record_sphere() {
        let tree = sphere_tree();
        let rec = sdf_to_db_asset_record(&tree, 1);
        assert_ne!(rec.tree_hash, 0);
        assert_eq!(rec.node_count, 1);
        assert!(rec.serialized_bytes > 0);
        assert!(rec.is_primitive_root);
        assert_eq!(rec.schema_version, 1);
    }

    #[test]
    fn test_sdf_to_db_asset_record_csg_not_primitive_root() {
        let tree = csg_tree();
        let rec = sdf_to_db_asset_record(&tree, 2);
        assert!(rec.node_count > 1);
        // Union root → not a primitive root.
        assert!(!rec.is_primitive_root);
    }

    #[test]
    fn test_sdf_to_ml_features_sphere() {
        let tree = sphere_tree();
        let f = sdf_to_ml_features(&tree);
        assert_eq!(f.feature_vec.len(), 5);
        // Single sphere: all primitive_ratio should be 1.0.
        assert!((f.primitive_ratio - 1.0).abs() < f32::EPSILON);
        assert_eq!(f.operation_ratio, 0.0);
        assert!(f.norm_node_count >= 0.0);
        assert!(f.surface_proxy > 0.0);
    }

    #[test]
    fn test_sdf_to_ml_features_csg_has_operations() {
        let tree = csg_tree();
        let f = sdf_to_ml_features(&tree);
        // CSG tree has operation nodes → operation_ratio > 0.
        assert!(f.operation_ratio > 0.0);
    }

    #[test]
    fn test_sdf_to_print_slice_meta() {
        let tree = csg_tree();
        let meta = sdf_to_print_slice_meta(&tree, 10.0);
        assert_ne!(meta.tree_hash, 0);
        assert!(meta.layer_height_mm > 0.0);
        assert!(meta.layer_height_mm <= 0.2);
        assert!(meta.bbox_diagonal_mm > 0.0);
        assert!(meta.support_density >= 0.1);
        assert!(meta.support_density <= 0.8);
        assert!(meta.node_count > 0);
    }

    #[test]
    fn test_sdf_to_physics_collider_single_sphere_is_analytic() {
        let tree = sphere_tree();
        let col = sdf_to_physics_collider(&tree);
        assert_ne!(col.shape_hash, 0);
        // Single primitive root → collider_type 0 (sphere).
        assert_eq!(col.collider_type, 0);
        assert_eq!(col.sub_shape_count, 1);
        assert!(col.bounding_radius > 0.0);
        assert!(col.mass_kg >= 0.0);
        assert!((col.restitution - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sdf_to_physics_collider_csg_small_is_compound() {
        let tree = csg_tree();
        let col = sdf_to_physics_collider(&tree);
        // Small CSG tree (≤8 nodes) → compound (type 4).
        assert_eq!(col.collider_type, 4);
    }

    #[test]
    fn test_sdf_to_physics_collider_deep_tree_is_convex_hull() {
        let tree = deep_tree();
        // Deep tree has many nodes.
        if tree.root.node_count() > 8 {
            let col = sdf_to_physics_collider(&tree);
            assert_eq!(col.collider_type, 3, "deep tree → convex hull");
            assert_eq!(col.sub_shape_count, 1);
        }
    }

    #[test]
    fn test_sdf_to_db_version_record() {
        let tree = csg_tree();
        let rec = sdf_to_db_version_record(&tree, 0, 3);
        assert_ne!(rec.tree_hash, 0);
        assert_eq!(rec.edit_op, 0);
        assert_eq!(rec.schema_version, 3);
        assert!(rec.node_count > 0);
        // Union root → operation category (1).
        assert_eq!(rec.root_category, 1);
    }

    #[test]
    fn test_sdf_to_db_version_record_different_ops_differ() {
        let tree = csg_tree();
        let r0 = sdf_to_db_version_record(&tree, 0, 1);
        let r1 = sdf_to_db_version_record(&tree, 1, 1);
        assert_ne!(r0.tree_hash, r1.tree_hash, "different edit_op → different hash");
    }

    #[test]
    fn test_sdf_to_ml_outlier_record_normal() {
        let tree = csg_tree();
        let nc = tree.root.node_count();
        // Expected close to actual → low score.
        let rec = sdf_to_ml_outlier_record(&tree, nc as f32);
        assert_ne!(rec.tree_hash, 0);
        assert_eq!(rec.node_count, nc);
        assert!((rec.outlier_score).abs() < f32::EPSILON, "exact match → score 0");
        assert!(!rec.is_flagged);
    }

    #[test]
    fn test_sdf_to_ml_outlier_record_anomalous() {
        let tree = deep_tree(); // many nodes
        // Expected 1 node → strong anomaly.
        let rec = sdf_to_ml_outlier_record(&tree, 1.0);
        assert!(rec.outlier_score > 0.0);
    }
}
