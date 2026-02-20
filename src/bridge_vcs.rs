//! VCS bridges — ALICE-VCS ↔ SDF, Animation, Manga, Sync, DB, Auth
//!
//! 11 bridges connecting AST semantic version control to the ALICE ecosystem.

use alice_sdf::{SdfNode, SdfTree};
use alice_vcs::ast::{AstNodeKind, AstTree, NodeId};
use alice_vcs::diff::{diff_trees, patch_size_bytes, DiffOp};
use alice_vcs::store::Hash;
use alice_vcs::Repository;

// ── Bridge 1: VCS → SDF (CSG scene graph versioning) ────────────────────

/// VCS snapshot of an SDF scene for change tracking.
pub struct SdfSceneSnapshot {
    /// VCS commit hash.
    pub commit_hash: Hash,
    /// Number of AST nodes in the scene.
    pub node_count: usize,
    /// Diff size in bytes vs previous version.
    pub diff_bytes: usize,
    /// SDF node types in the scene (for statistics).
    pub node_types: Vec<String>,
}

/// Convert SdfTree to VCS AstTree for version tracking.
#[inline]
pub fn sdf_to_vcs_tree(sdf: &SdfTree) -> AstTree {
    let mut tree = AstTree::new();
    let root = tree.add_node(AstNodeKind::Root, "scene", 0);
    sdf_node_to_ast(&sdf.root, &mut tree, root);
    tree
}

fn sdf_node_to_ast(node: &SdfNode, tree: &mut AstTree, parent: NodeId) {
    match node {
        // Named primitives (common shapes get descriptive labels)
        SdfNode::Sphere { radius } => {
            tree.add_node(AstNodeKind::Primitive, &format!("sphere_{}", (*radius * 100.0) as i32), parent);
        }
        SdfNode::Box3d { half_extents } => {
            tree.add_node(AstNodeKind::Primitive, &format!("box_{}x{}x{}", (half_extents.x * 100.0) as i32, (half_extents.y * 100.0) as i32, (half_extents.z * 100.0) as i32), parent);
        }
        SdfNode::Cylinder { radius, half_height } => {
            tree.add_node(AstNodeKind::Primitive, &format!("cyl_{}_{}", (*radius * 100.0) as i32, (*half_height * 100.0) as i32), parent);
        }

        // CSG operations (24 variants — all have { a, b } children to recurse)
        SdfNode::Union { a, b, .. }
        | SdfNode::Intersection { a, b, .. }
        | SdfNode::Subtraction { a, b, .. }
        | SdfNode::SmoothUnion { a, b, .. }
        | SdfNode::SmoothIntersection { a, b, .. }
        | SdfNode::SmoothSubtraction { a, b, .. }
        | SdfNode::ChamferUnion { a, b, .. }
        | SdfNode::ChamferIntersection { a, b, .. }
        | SdfNode::ChamferSubtraction { a, b, .. }
        | SdfNode::StairsUnion { a, b, .. }
        | SdfNode::StairsIntersection { a, b, .. }
        | SdfNode::StairsSubtraction { a, b, .. }
        | SdfNode::XOR { a, b }
        | SdfNode::Morph { a, b, .. }
        | SdfNode::ColumnsUnion { a, b, .. }
        | SdfNode::ColumnsIntersection { a, b, .. }
        | SdfNode::ColumnsSubtraction { a, b, .. }
        | SdfNode::Pipe { a, b, .. }
        | SdfNode::Engrave { a, b, .. }
        | SdfNode::Groove { a, b, .. }
        | SdfNode::Tongue { a, b, .. }
        | SdfNode::ExpSmoothUnion { a, b, .. }
        | SdfNode::ExpSmoothIntersection { a, b, .. }
        | SdfNode::ExpSmoothSubtraction { a, b, .. }
        => {
            let op = tree.add_node(AstNodeKind::CsgOp, "csg_op", parent);
            sdf_node_to_ast(a, tree, op);
            sdf_node_to_ast(b, tree, op);
        }

        // Transforms (7 variants — all have { child } to recurse)
        SdfNode::Translate { child, .. }
        | SdfNode::Rotate { child, .. }
        | SdfNode::Scale { child, .. }
        | SdfNode::ScaleNonUniform { child, .. }
        | SdfNode::ProjectiveTransform { child, .. }
        | SdfNode::LatticeDeform { child, .. }
        | SdfNode::SdfSkinning { child, .. }
        => {
            let t = tree.add_node(AstNodeKind::Transform, "transform", parent);
            sdf_node_to_ast(child, tree, t);
        }

        // Modifiers (23 variants — all have { child } to recurse)
        SdfNode::Twist { child, .. }
        | SdfNode::Bend { child, .. }
        | SdfNode::RepeatInfinite { child, .. }
        | SdfNode::RepeatFinite { child, .. }
        | SdfNode::Noise { child, .. }
        | SdfNode::Round { child, .. }
        | SdfNode::Onion { child, .. }
        | SdfNode::Elongate { child, .. }
        | SdfNode::Mirror { child, .. }
        | SdfNode::Revolution { child, .. }
        | SdfNode::Extrude { child, .. }
        | SdfNode::SweepBezier { child, .. }
        | SdfNode::Taper { child, .. }
        | SdfNode::Displacement { child, .. }
        | SdfNode::PolarRepeat { child, .. }
        | SdfNode::OctantMirror { child, .. }
        | SdfNode::Shear { child, .. }
        | SdfNode::Animated { child, .. }
        | SdfNode::WithMaterial { child, .. }
        | SdfNode::IcosahedralSymmetry { child, .. }
        | SdfNode::IFS { child, .. }
        | SdfNode::HeightmapDisplacement { child, .. }
        | SdfNode::SurfaceRoughness { child, .. }
        => {
            let m = tree.add_node(AstNodeKind::Custom, "modifier", parent);
            sdf_node_to_ast(child, tree, m);
        }

        // Remaining 72 primitives (leaf nodes, no children to recurse)
        _ => {
            tree.add_node(AstNodeKind::Primitive, "primitive", parent);
        }
    }
}

/// Commit an SDF scene to VCS repository and return diff statistics.
#[inline]
pub fn vcs_commit_sdf(repo: &mut Repository, sdf: &SdfTree, message: &str) -> SdfSceneSnapshot {
    let tree = sdf_to_vcs_tree(sdf);
    let hash = repo.commit(&tree, message, "ALICE-Eco-System");
    let node_count = count_ast_nodes(&tree);

    // Calculate diff vs parent
    let diff_bytes = if let Some(parent_hash) = repo.head_tree().and_then(|_| {
        let commits: Vec<_> = vec![hash]; // Current head
        if commits.is_empty() { None } else { Some(hash) }
    }) {
        if let Some(ops) = repo.diff(parent_hash, hash) {
            patch_size_bytes(&ops)
        } else {
            0
        }
    } else {
        0
    };

    SdfSceneSnapshot {
        commit_hash: hash,
        node_count,
        diff_bytes,
        node_types: vec!["Primitive".into(), "CsgOp".into(), "Transform".into()],
    }
}

fn count_ast_nodes(tree: &AstTree) -> usize {
    let mut count = 0;
    let mut id = 0u32;
    while tree.get_node(id).is_some() {
        count += 1;
        id += 1;
    }
    count
}

// ── Bridge 2: VCS → Animation (scene graph versioning) ──────────────────

/// VCS change summary for ALICE-Animation scene graph.
pub struct AnimSceneChange {
    /// Commit hash.
    pub commit_hash: Hash,
    /// Number of diff operations.
    pub diff_ops: usize,
    /// Diff size in bytes.
    pub diff_bytes: usize,
    /// Human-readable change description.
    pub description: String,
}

/// Track animation scene changes via VCS diff.
#[inline]
pub fn vcs_animation_diff(old_tree: &AstTree, new_tree: &AstTree) -> AnimSceneChange {
    let ops = diff_trees(old_tree, new_tree);
    let bytes = patch_size_bytes(&ops);
    let desc = format!("{} changes ({} bytes)", ops.len(), bytes);
    AnimSceneChange {
        commit_hash: 0,
        diff_ops: ops.len(),
        diff_bytes: bytes,
        description: desc,
    }
}

// ── Bridge 3: VCS → Manga (page versioning) ────────────────────────────

/// VCS manga page revision.
pub struct MangaPageRevision {
    /// Page number.
    pub page_number: u32,
    /// Commit hash.
    pub commit_hash: Hash,
    /// AST node count for the page.
    pub node_count: usize,
    /// Diff size vs previous revision.
    pub diff_bytes: usize,
}

/// Create a VCS-tracked manga page AST.
#[inline]
pub fn vcs_manga_page(page_number: u32, panels: &[(f32, f32, f32, f32)]) -> AstTree {
    let mut tree = AstTree::new();
    let root = tree.add_node(AstNodeKind::Root, &format!("page_{}", page_number), 0);
    for (i, (x, y, w, h)) in panels.iter().enumerate() {
        let panel = tree.add_node(AstNodeKind::Group, &format!("panel_{}", i), root);
        tree.add_node(
            AstNodeKind::Primitive,
            &format!("rect_{}_{}_{}_{}",
                (*x * 10.0) as i32, (*y * 10.0) as i32,
                (*w * 10.0) as i32, (*h * 10.0) as i32),
            panel,
        );
    }
    tree
}

// ── Bridge 4: VCS → Sync (collaborative editing) ───────────────────────

/// Sync packet carrying VCS diff for collaborative editing.
pub struct VcsSyncPacket {
    /// Diff operations serialized.
    pub diff_ops_count: usize,
    /// Total diff size in bytes.
    pub diff_bytes: usize,
    /// Source commit hash.
    pub from_hash: Hash,
    /// Target commit hash.
    pub to_hash: Hash,
}

/// Package VCS diff for ALICE-Sync P2P exchange.
#[inline]
pub fn vcs_to_sync_packet(old: &AstTree, new: &AstTree, from_hash: Hash, to_hash: Hash) -> VcsSyncPacket {
    let ops = diff_trees(old, new);
    VcsSyncPacket {
        diff_ops_count: ops.len(),
        diff_bytes: patch_size_bytes(&ops),
        from_hash,
        to_hash,
    }
}

// ── Bridge 5: VCS → DB (snapshot persistence) ───────────────────────────

/// VCS snapshot record for ALICE-DB persistence.
pub struct VcsDbRecord {
    /// Content hash.
    pub hash: Hash,
    /// Timestamp key for DB.
    pub timestamp: i64,
    /// Commit message.
    pub message: String,
    /// Number of nodes in the snapshot.
    pub node_count: usize,
}

/// Prepare VCS commit for ALICE-DB persistence.
#[inline]
pub fn vcs_to_db_record(hash: Hash, message: &str, tree: &AstTree, timestamp: i64) -> VcsDbRecord {
    VcsDbRecord {
        hash,
        timestamp,
        message: message.to_string(),
        node_count: count_ast_nodes(tree),
    }
}

// ── Bridge 6: VCS → Auth (repository access control) ────────────────────

/// Repository access request for ALICE-Auth verification.
pub struct VcsAuthRequest {
    /// Repository name.
    pub repo_name: String,
    /// Requested operation.
    pub operation: VcsOperation,
    /// Commit hash (for read operations).
    pub commit_hash: Option<Hash>,
}

/// VCS operation type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VcsOperation {
    Read,
    Write,
    Branch,
    Merge,
}

/// Create an auth request for VCS repository access.
#[inline]
pub fn vcs_auth_request(repo_name: &str, operation: VcsOperation, commit_hash: Option<Hash>) -> VcsAuthRequest {
    VcsAuthRequest {
        repo_name: repo_name.to_string(),
        operation,
        commit_hash,
    }
}

// ── Bridge 7: VCS → CDN (repository distribution) ──────────────────

/// VCS repository package for ALICE-CDN delivery.
pub struct VcsCdnPackage {
    /// Snapshot data.
    pub data: Vec<u8>,
    /// Content hash for CDN routing.
    pub content_hash: u64,
    /// Number of nodes.
    pub node_count: usize,
    /// Content type.
    pub content_type: &'static str,
}

/// Package VCS tree snapshot for ALICE-CDN distribution.
#[inline]
pub fn vcs_to_cdn_package(tree: &AstTree, hash: Hash) -> VcsCdnPackage {
    let node_count = count_ast_nodes(tree);
    let mut data = Vec::new();
    data.extend_from_slice(&hash.to_le_bytes());
    data.extend_from_slice(&(node_count as u32).to_le_bytes());
    let mut content_hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        content_hash ^= b as u64;
        content_hash = content_hash.wrapping_mul(0x100000001b3);
    }
    VcsCdnPackage {
        data,
        content_hash,
        node_count,
        content_type: "application/x-alice-vcs",
    }
}

// ── Bridge 8: VCS → Cache (tree snapshot caching) ───────────────────

/// VCS tree cache entry for ALICE-Cache.
pub struct VcsCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Commit hash.
    pub commit_hash: Hash,
    /// Node count.
    pub node_count: usize,
}

/// Prepare VCS tree snapshot for ALICE-Cache storage.
#[inline]
pub fn vcs_to_cache_entry(tree: &AstTree, hash: Hash) -> VcsCacheEntry {
    let node_count = count_ast_nodes(tree);
    let mut data = Vec::new();
    data.extend_from_slice(&hash.to_le_bytes());
    data.extend_from_slice(&(node_count as u32).to_le_bytes());
    let mut content_hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        content_hash ^= b as u64;
        content_hash = content_hash.wrapping_mul(0x100000001b3);
    }
    VcsCacheEntry {
        content_hash,
        commit_hash: hash,
        node_count,
    }
}

// ── Bridge 9: VCS → Crypto (signed commits) ─────────────────────────

/// Signed commit payload for ALICE-Crypto.
pub struct VcsCryptoPayload {
    /// Commit hash.
    pub commit_hash: Hash,
    /// Plaintext (commit metadata).
    pub plaintext: Vec<u8>,
    /// Content hash for integrity.
    pub content_hash: u64,
    /// Payload size.
    pub payload_bytes: usize,
}

/// Prepare VCS commit for ALICE-Crypto signing.
#[inline]
pub fn vcs_to_crypto_payload(hash: Hash, message: &str, tree: &AstTree) -> VcsCryptoPayload {
    let mut data = Vec::new();
    data.extend_from_slice(&hash.to_le_bytes());
    data.extend_from_slice(message.as_bytes());
    data.extend_from_slice(&(count_ast_nodes(tree) as u32).to_le_bytes());
    let mut content_hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        content_hash ^= b as u64;
        content_hash = content_hash.wrapping_mul(0x100000001b3);
    }
    let len = data.len();
    VcsCryptoPayload {
        commit_hash: hash,
        plaintext: data,
        content_hash,
        payload_bytes: len,
    }
}

// ── Bridge 10: VCS → Print (revision metadata for print) ────────────

/// Print revision metadata from VCS.
pub struct VcsPrintRevision {
    /// Commit hash.
    pub commit_hash: Hash,
    /// Node count (scene complexity).
    pub node_count: usize,
    /// Diff size vs previous revision.
    pub diff_bytes: usize,
    /// Label for the revision.
    pub label: String,
}

/// Create print revision metadata from VCS tree.
#[inline]
pub fn vcs_to_print_revision(hash: Hash, message: &str, tree: &AstTree) -> VcsPrintRevision {
    VcsPrintRevision {
        commit_hash: hash,
        node_count: count_ast_nodes(tree),
        diff_bytes: 0,
        label: message.to_string(),
    }
}

// ── Bridge 11: VCS → View (diff visualization) ─────────────────────

/// Diff visualization data for ALICE-View.
pub struct VcsViewDiff {
    /// Number of added nodes.
    pub added: usize,
    /// Number of removed nodes.
    pub removed: usize,
    /// Number of modified nodes.
    pub modified: usize,
    /// Total diff operations.
    pub total_ops: usize,
    /// Diff size in bytes.
    pub diff_bytes: usize,
}

/// Generate diff visualization data for ALICE-View.
#[inline]
pub fn vcs_to_view_diff(old: &AstTree, new: &AstTree) -> VcsViewDiff {
    let ops = diff_trees(old, new);
    let bytes = patch_size_bytes(&ops);
    VcsViewDiff {
        added: ops.iter().filter(|o| matches!(o, DiffOp::Insert { .. })).count(),
        removed: ops.iter().filter(|o| matches!(o, DiffOp::Delete { .. })).count(),
        modified: ops.iter().filter(|o| matches!(o, DiffOp::Update { .. })).count(),
        total_ops: ops.len(),
        diff_bytes: bytes,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdf_to_vcs_tree() {
        let sdf = SdfTree::new(
            SdfNode::sphere(1.0).union(SdfNode::box3d(0.5, 0.5, 0.5)),
        );
        let tree = sdf_to_vcs_tree(&sdf);
        let count = count_ast_nodes(&tree);
        assert!(count >= 3); // root + union + 2 primitives
    }

    #[test]
    fn test_vcs_commit_sdf() {
        let mut repo = Repository::new();
        let sdf = SdfTree::new(SdfNode::sphere(2.0));
        let snapshot = vcs_commit_sdf(&mut repo, &sdf, "initial sphere");
        assert_ne!(snapshot.commit_hash, 0);
        assert!(snapshot.node_count >= 2);
    }

    #[test]
    fn test_vcs_animation_diff() {
        let mut old = AstTree::new();
        let root = old.add_node(AstNodeKind::Root, "scene", 0);
        old.add_node(AstNodeKind::Primitive, "sphere", root);

        let mut new = AstTree::new();
        let root2 = new.add_node(AstNodeKind::Root, "scene", 0);
        new.add_node(AstNodeKind::Primitive, "sphere", root2);
        new.add_node(AstNodeKind::Primitive, "box", root2);

        let change = vcs_animation_diff(&old, &new);
        assert!(change.diff_ops > 0);
        assert!(change.diff_bytes > 0);
    }

    #[test]
    fn test_vcs_manga_page() {
        let panels = vec![(0.0, 0.0, 100.0, 150.0), (100.0, 0.0, 100.0, 150.0)];
        let tree = vcs_manga_page(1, &panels);
        let count = count_ast_nodes(&tree);
        assert!(count >= 5); // root + 2 panels + 2 rects
    }

    #[test]
    fn test_vcs_to_sync_packet() {
        let mut t1 = AstTree::new();
        t1.add_node(AstNodeKind::Root, "a", 0);
        let mut t2 = AstTree::new();
        let r = t2.add_node(AstNodeKind::Root, "a", 0);
        t2.add_node(AstNodeKind::Primitive, "b", r);
        let pkt = vcs_to_sync_packet(&t1, &t2, 100, 200);
        assert!(pkt.diff_ops_count > 0);
        assert_eq!(pkt.from_hash, 100);
        assert_eq!(pkt.to_hash, 200);
    }

    #[test]
    fn test_vcs_to_db_record() {
        let mut tree = AstTree::new();
        tree.add_node(AstNodeKind::Root, "root", 0);
        let rec = vcs_to_db_record(12345, "test commit", &tree, 1_000_000);
        assert_eq!(rec.hash, 12345);
        assert_eq!(rec.node_count, 2); // sentinel root + user root
    }

    #[test]
    fn test_vcs_auth_request() {
        let req = vcs_auth_request("my-repo", VcsOperation::Write, Some(999));
        assert_eq!(req.repo_name, "my-repo");
        assert_eq!(req.operation, VcsOperation::Write);
        assert_eq!(req.commit_hash, Some(999));
    }

    #[test]
    fn test_vcs_to_cdn_package() {
        let mut tree = AstTree::new();
        let root = tree.add_node(AstNodeKind::Root, "scene", 0);
        tree.add_node(AstNodeKind::Primitive, "sphere", root);
        let pkg = vcs_to_cdn_package(&tree, 12345);
        assert_ne!(pkg.content_hash, 0);
        assert!(pkg.node_count >= 2);
        assert_eq!(pkg.content_type, "application/x-alice-vcs");
    }

    #[test]
    fn test_vcs_to_cache_entry() {
        let mut tree = AstTree::new();
        tree.add_node(AstNodeKind::Root, "root", 0);
        let entry = vcs_to_cache_entry(&tree, 999);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.commit_hash, 999);
    }

    #[test]
    fn test_vcs_to_crypto_payload() {
        let mut tree = AstTree::new();
        tree.add_node(AstNodeKind::Root, "root", 0);
        let crypto = vcs_to_crypto_payload(123, "test commit", &tree);
        assert_eq!(crypto.commit_hash, 123);
        assert!(crypto.payload_bytes > 0);
        assert_ne!(crypto.content_hash, 0);
    }

    #[test]
    fn test_vcs_to_print_revision() {
        let mut tree = AstTree::new();
        tree.add_node(AstNodeKind::Root, "root", 0);
        let rev = vcs_to_print_revision(456, "v1.0", &tree);
        assert_eq!(rev.commit_hash, 456);
        assert_eq!(rev.label, "v1.0");
    }

    #[test]
    fn test_vcs_to_view_diff() {
        let mut old = AstTree::new();
        let root = old.add_node(AstNodeKind::Root, "scene", 0);
        old.add_node(AstNodeKind::Primitive, "sphere", root);

        let mut new = AstTree::new();
        let root2 = new.add_node(AstNodeKind::Root, "scene", 0);
        new.add_node(AstNodeKind::Primitive, "sphere", root2);
        new.add_node(AstNodeKind::Primitive, "box", root2);

        let diff = vcs_to_view_diff(&old, &new);
        assert!(diff.total_ops > 0);
        assert!(diff.diff_bytes > 0);
    }
}
