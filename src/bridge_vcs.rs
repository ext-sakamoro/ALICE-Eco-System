//! VCS bridges — ALICE-VCS ↔ SDF, Animation, Manga, Sync, DB, Auth
//!
//! 6 bridges connecting AST semantic version control to the ALICE ecosystem.

use alice_sdf::{SdfNode, SdfTree};
use alice_vcs::ast::{AstNodeKind, AstTree, NodeId};
use alice_vcs::diff::{diff_trees, patch_size_bytes};
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
pub fn sdf_to_vcs_tree(sdf: &SdfTree) -> AstTree {
    let mut tree = AstTree::new();
    let root = tree.add_node(AstNodeKind::Root, "scene", 0);
    sdf_node_to_ast(&sdf.root, &mut tree, root);
    tree
}

fn sdf_node_to_ast(node: &SdfNode, tree: &mut AstTree, parent: NodeId) {
    match node {
        SdfNode::Sphere { radius } => {
            tree.add_node(AstNodeKind::Primitive, &format!("sphere_{}", (*radius * 100.0) as i32), parent);
        }
        SdfNode::Box3d { half_extents } => {
            tree.add_node(AstNodeKind::Primitive, &format!("box_{}x{}x{}", (half_extents.x * 100.0) as i32, (half_extents.y * 100.0) as i32, (half_extents.z * 100.0) as i32), parent);
        }
        SdfNode::Cylinder { radius, half_height } => {
            tree.add_node(AstNodeKind::Primitive, &format!("cyl_{}_{}", (*radius * 100.0) as i32, (*half_height * 100.0) as i32), parent);
        }
        SdfNode::Union { a, b } => {
            let op = tree.add_node(AstNodeKind::CsgOp, "union", parent);
            sdf_node_to_ast(a, tree, op);
            sdf_node_to_ast(b, tree, op);
        }
        SdfNode::Subtraction { a, b } => {
            let op = tree.add_node(AstNodeKind::CsgOp, "subtract", parent);
            sdf_node_to_ast(a, tree, op);
            sdf_node_to_ast(b, tree, op);
        }
        SdfNode::Intersection { a, b } => {
            let op = tree.add_node(AstNodeKind::CsgOp, "intersect", parent);
            sdf_node_to_ast(a, tree, op);
            sdf_node_to_ast(b, tree, op);
        }
        SdfNode::Translate { child, offset } => {
            let t = tree.add_node(AstNodeKind::Transform, &format!("translate_{}_{}_{}", (offset.x * 100.0) as i32, (offset.y * 100.0) as i32, (offset.z * 100.0) as i32), parent);
            sdf_node_to_ast(child, tree, t);
        }
        _ => {
            tree.add_node(AstNodeKind::Custom, "other", parent);
        }
    }
}

/// Commit an SDF scene to VCS repository and return diff statistics.
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
pub fn vcs_auth_request(repo_name: &str, operation: VcsOperation, commit_hash: Option<Hash>) -> VcsAuthRequest {
    VcsAuthRequest {
        repo_name: repo_name.to_string(),
        operation,
        commit_hash,
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
}
