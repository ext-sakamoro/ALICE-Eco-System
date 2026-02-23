//! Scene I/O bridges — ALICE-Physics (scene_io) ↔ DB, CDN, Cache, Analytics, Edge
//!
//! 5 bridges connecting physics scene serialization to the ALICE ecosystem.
//!
//! Note: `PhysicsScene`, `SerializedBody`, and `SerializedJoint` come from
//! `alice_physics::scene_io`. The `PhysicsConfig` used here is
//! `alice_physics::scene_io::PhysicsConfig` (serialized format), NOT
//! `alice_physics::solver::PhysicsConfig` (runtime format).

use alice_physics::{PhysicsScene, SerializedBody, SerializedJoint};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: PhysicsScene → DB asset record ───────────────────────────

/// Persistence record for a physics scene asset in ALICE-DB.
///
/// Captures scene topology (body/joint counts), version, and a
/// determinism checksum for asset management and replay indexing.
pub struct SceneDbAssetRecord {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Number of bodies in the scene.
    pub body_count: usize,
    /// Number of joints in the scene.
    pub joint_count: usize,
    /// Scene format version.
    pub version: u32,
    /// Solver substeps.
    pub substeps: u32,
    /// Solver iterations per substep.
    pub iterations: u32,
    /// Determinism checksum: XOR-fold of body position high-words.
    pub determinism_checksum: u64,
}

/// Build a `SceneDbAssetRecord` from a `PhysicsScene`.
///
/// Determinism checksum XOR-folds position[0] (x.hi) of each body for
/// cross-platform verification.
#[inline]
#[must_use]
pub fn scene_to_db_asset(scene: &PhysicsScene) -> SceneDbAssetRecord {
    let body_count = scene.bodies.len();
    let joint_count = scene.joints.len();

    let determinism_checksum = scene
        .bodies
        .iter()
        .fold(0u64, |acc, b| acc ^ (b.position[0] as u64));

    let mut bytes = [0u8; 32];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(joint_count as u64).to_le_bytes());
    bytes[16..20].copy_from_slice(&scene.version.to_le_bytes());
    bytes[20..28].copy_from_slice(&determinism_checksum.to_le_bytes());
    bytes[28..32].copy_from_slice(&scene.config.substeps.to_le_bytes());
    let content_hash = fnv1a(&bytes);

    SceneDbAssetRecord {
        content_hash,
        body_count,
        joint_count,
        version: scene.version,
        substeps: scene.config.substeps,
        iterations: scene.config.iterations,
        determinism_checksum,
    }
}

// ── Bridge 2: Scene → CDN descriptor ───────────────────────────────────

/// CDN descriptor for a physics scene asset.
///
/// Contains size estimates and content type metadata for ALICE-CDN
/// distribution of serialized physics scene files (.aphys / .json).
pub struct SceneCdnDescriptor {
    /// Content hash for CDN cache keying.
    pub content_hash: u64,
    /// Estimated binary size in bytes (.aphys format).
    pub estimated_binary_size: usize,
    /// Estimated JSON size in bytes (rough approximation).
    pub estimated_json_size: usize,
    /// Number of bodies (complexity indicator for CDN edge routing).
    pub body_count: usize,
    /// Number of joints.
    pub joint_count: usize,
    /// Scene format version.
    pub version: u32,
}

/// Build a `SceneCdnDescriptor` from a `PhysicsScene`.
///
/// Binary size estimate: header(18) + config(40) + bodies(body_count * 128) + joints(joint_count * 57).
/// JSON size estimate: ~4x binary size (hex encoding + formatting).
#[inline]
#[must_use]
pub fn scene_to_cdn(scene: &PhysicsScene) -> SceneCdnDescriptor {
    let body_count = scene.bodies.len();
    let joint_count = scene.joints.len();

    // Binary format: 6(magic) + 4(version) + 4(body_count) + 4(joint_count)
    //   + 40(config) + 128*bodies + 57*joints
    let estimated_binary_size = 18 + 40 + body_count * 128 + joint_count * 57;

    // JSON: roughly 4x binary due to decimal encoding and formatting.
    let estimated_json_size = estimated_binary_size << 2;

    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(joint_count as u64).to_le_bytes());
    bytes[16..20].copy_from_slice(&scene.version.to_le_bytes());
    bytes[20..24].copy_from_slice(&(estimated_binary_size as u32).to_le_bytes());
    let content_hash = fnv1a(&bytes);

    SceneCdnDescriptor {
        content_hash,
        estimated_binary_size,
        estimated_json_size,
        body_count,
        joint_count,
        version: scene.version,
    }
}

// ── Bridge 3: Scene binary → Cache entry ───────────────────────────────

/// Cache entry for a serialized physics scene.
///
/// Enables caching of deserialized scene data to avoid repeated
/// parsing of .aphys binary files.
pub struct SceneCacheEntry {
    /// Cache key derived from scene content.
    pub content_hash: u64,
    /// Number of bodies in the cached scene.
    pub body_count: usize,
    /// Estimated memory cost (bytes).
    pub state_size_bytes: usize,
    /// Time-to-live in seconds.
    pub ttl_secs: u32,
    /// Eviction priority (higher = keep longer).
    pub eviction_priority: u32,
}

/// Build a `SceneCacheEntry` from a `PhysicsScene`.
///
/// Branchless TTL: scenes with many joints (>50) are likely dynamic
/// (ragdoll, vehicle) and get shorter TTL (15s). Scenes with few joints
/// get 120s.
#[inline]
#[must_use]
pub fn scene_to_cache(scene: &PhysicsScene) -> SceneCacheEntry {
    let body_count = scene.bodies.len();
    let joint_count = scene.joints.len();

    // Memory: each SerializedBody ~120B, each SerializedJoint ~60B.
    let state_size_bytes = body_count * 120 + joint_count * 60;

    // Branchless TTL: many joints → 15s, few → 120s.
    let is_complex = (joint_count > 50) as u32;
    let ttl_secs = 120 - is_complex * 105;

    // Eviction priority: based on body count (larger scenes more costly to rebuild).
    let eviction_priority = (body_count as u32).min(u32::MAX);

    // Hash: body_count + joint_count + version + first body position[0].
    let first_body_pos = scene
        .bodies
        .first()
        .map(|b| b.position[0])
        .unwrap_or(0);
    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&(joint_count as u64).to_le_bytes());
    bytes[16..24].copy_from_slice(&(first_body_pos as u64).to_le_bytes());
    let content_hash = fnv1a(&bytes);

    SceneCacheEntry {
        content_hash,
        body_count,
        state_size_bytes,
        ttl_secs,
        eviction_priority,
    }
}

// ── Bridge 4: Scene config → Analytics event ───────────────────────────

/// Analytics event for physics scene configuration.
///
/// Tracks solver parameter choices across a fleet of scenes for
/// performance tuning and anomaly detection.
pub struct SceneConfigAnalyticsEvent {
    /// Content hash for time-series deduplication.
    pub content_hash: u64,
    /// Number of bodies in the scene.
    pub body_count: usize,
    /// Number of joints.
    pub joint_count: usize,
    /// Solver substeps.
    pub substeps: u32,
    /// Solver iterations per substep.
    pub iterations: u32,
    /// Scene format version.
    pub version: u32,
    /// Body type distribution: [dynamic, static, kinematic].
    pub body_type_distribution: [usize; 3],
}

/// Build a `SceneConfigAnalyticsEvent` from a `PhysicsScene`.
///
/// Body type distribution counted via match on body_type byte:
/// 0=Dynamic, 1=Static, 2=Kinematic.
#[inline]
#[must_use]
pub fn scene_config_to_analytics(scene: &PhysicsScene) -> SceneConfigAnalyticsEvent {
    let body_count = scene.bodies.len();
    let joint_count = scene.joints.len();

    let mut distribution = [0usize; 3];
    for body in &scene.bodies {
        let idx = match body.body_type {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 0, // unknown defaults to dynamic
        };
        distribution[idx] += 1;
    }

    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..12].copy_from_slice(&scene.config.substeps.to_le_bytes());
    bytes[12..16].copy_from_slice(&scene.config.iterations.to_le_bytes());
    bytes[16..20].copy_from_slice(&scene.version.to_le_bytes());
    bytes[20..24].copy_from_slice(&(joint_count as u32).to_le_bytes());
    let content_hash = fnv1a(&bytes);

    SceneConfigAnalyticsEvent {
        content_hash,
        body_count,
        joint_count,
        substeps: scene.config.substeps,
        iterations: scene.config.iterations,
        version: scene.version,
        body_type_distribution: distribution,
    }
}

// ── Bridge 5: Scene body count → Edge summary ──────────────────────────

/// Lightweight edge summary for scene complexity telemetry.
///
/// Suitable for IoT/sensor pipeline ingestion to monitor simulation
/// fleet complexity distribution.
pub struct SceneEdgeSummary {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Number of bodies.
    pub body_count: usize,
    /// Number of joints.
    pub joint_count: usize,
    /// Ratio of joints to bodies (connectivity metric).
    pub joint_body_ratio: f32,
    /// Scene format version.
    pub version: u32,
    /// Time-to-live in seconds.
    pub ttl_secs: u32,
}

/// Build a `SceneEdgeSummary` from a `PhysicsScene`.
///
/// Joint-to-body ratio computed via reciprocal multiply.
/// Branchless TTL: empty scenes (0 bodies) get 5s; populated scenes get 60s.
#[inline]
#[must_use]
pub fn scene_to_edge_summary(scene: &PhysicsScene) -> SceneEdgeSummary {
    let body_count = scene.bodies.len();
    let joint_count = scene.joints.len();

    // Reciprocal multiply for ratio.
    let inv_bodies = 1.0_f32 / (body_count.max(1) as f32);
    let joint_body_ratio = joint_count as f32 * inv_bodies;

    // Branchless TTL: empty scene → 5s, populated → 60s.
    let is_empty = (body_count == 0) as u32;
    let ttl_secs = 60 - is_empty * 55;

    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&(body_count as u64).to_le_bytes());
    bytes[8..12].copy_from_slice(&(joint_count as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&scene.version.to_le_bytes());
    let content_hash = fnv1a(&bytes);

    SceneEdgeSummary {
        content_hash,
        body_count,
        joint_count,
        joint_body_ratio,
        version: scene.version,
        ttl_secs,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_physics::scene_io::PhysicsConfig as ScenePhysicsConfig;

    fn make_scene(n_bodies: usize, n_joints: usize) -> PhysicsScene {
        let bodies: Vec<SerializedBody> = (0..n_bodies)
            .map(|i| SerializedBody {
                position: [i as i64, 0, 0, 0, 0, 0],
                velocity: [0; 6],
                rotation: [0, 0, 0, 0, 0, 0, 1, 0], // identity quat (w.hi=1)
                mass: [1, 0],
                body_type: 0, // Dynamic
            })
            .collect();

        let joints: Vec<SerializedJoint> = (0..n_joints)
            .map(|i| SerializedJoint {
                body_a: i as u32,
                body_b: (i + 1).min(n_bodies.saturating_sub(1)) as u32,
                joint_type: 0,
                anchor_a: [0; 6],
                anchor_b: [0; 6],
            })
            .collect();

        PhysicsScene {
            bodies,
            joints,
            config: ScenePhysicsConfig::default(),
            version: 1,
        }
    }

    // ── Test 1: Scene → DB asset record ────────────────────────────────

    #[test]
    fn test_scene_to_db_asset() {
        let scene = make_scene(10, 3);
        let rec = scene_to_db_asset(&scene);

        assert_eq!(rec.body_count, 10);
        assert_eq!(rec.joint_count, 3);
        assert_eq!(rec.version, 1);
        assert_eq!(rec.substeps, 8); // default
        assert_eq!(rec.iterations, 4); // default
        assert_ne!(rec.content_hash, 0);
    }

    // ── Test 2: DB asset hash determinism ──────────────────────────────

    #[test]
    fn test_db_asset_hash_determinism() {
        let scene = make_scene(5, 2);
        let r1 = scene_to_db_asset(&scene);
        let r2 = scene_to_db_asset(&scene);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.determinism_checksum, r2.determinism_checksum);
    }

    // ── Test 3: Scene → CDN descriptor ─────────────────────────────────

    #[test]
    fn test_scene_to_cdn() {
        let scene = make_scene(8, 4);
        let cdn = scene_to_cdn(&scene);

        assert_eq!(cdn.body_count, 8);
        assert_eq!(cdn.joint_count, 4);
        assert!(cdn.estimated_binary_size > 0);
        assert_eq!(cdn.estimated_json_size, cdn.estimated_binary_size << 2);
        assert_ne!(cdn.content_hash, 0);
    }

    // ── Test 4: Scene → Cache entry (few joints, long TTL) ─────────────

    #[test]
    fn test_scene_cache_few_joints() {
        let scene = make_scene(10, 5);
        let entry = scene_to_cache(&scene);

        assert_eq!(entry.body_count, 10);
        assert_eq!(entry.ttl_secs, 120, "few joints should get 120s TTL");
        assert!(entry.state_size_bytes > 0);
        assert_ne!(entry.content_hash, 0);
    }

    // ── Test 5: Scene → Cache entry (many joints, short TTL) ───────────

    #[test]
    fn test_scene_cache_many_joints() {
        let scene = make_scene(100, 80);
        let entry = scene_to_cache(&scene);

        assert_eq!(entry.ttl_secs, 15, "many joints should get 15s TTL");
    }

    // ── Test 6: Scene config → Analytics event ─────────────────────────

    #[test]
    fn test_scene_config_to_analytics() {
        let scene = make_scene(20, 5);
        let evt = scene_config_to_analytics(&scene);

        assert_eq!(evt.body_count, 20);
        assert_eq!(evt.joint_count, 5);
        assert_eq!(evt.substeps, 8);
        assert_eq!(evt.iterations, 4);
        // All bodies are dynamic (body_type=0).
        assert_eq!(evt.body_type_distribution[0], 20);
        assert_eq!(evt.body_type_distribution[1], 0);
        assert_eq!(evt.body_type_distribution[2], 0);
        assert_ne!(evt.content_hash, 0);
    }

    // ── Test 7: Scene → Edge summary ───────────────────────────────────

    #[test]
    fn test_scene_to_edge_summary() {
        let scene = make_scene(10, 5);
        let summary = scene_to_edge_summary(&scene);

        assert_eq!(summary.body_count, 10);
        assert_eq!(summary.joint_count, 5);
        assert!((summary.joint_body_ratio - 0.5).abs() < 0.01);
        assert_eq!(summary.ttl_secs, 60);
        assert_ne!(summary.content_hash, 0);
    }

    // ── Test 8: Edge summary empty scene (short TTL) ───────────────────

    #[test]
    fn test_edge_summary_empty_scene() {
        let scene = make_scene(0, 0);
        let summary = scene_to_edge_summary(&scene);

        assert_eq!(summary.body_count, 0);
        assert_eq!(summary.ttl_secs, 5, "empty scene should get 5s TTL");
        assert_eq!(summary.joint_body_ratio, 0.0);
    }

    // ── Test 9: CDN hash determinism ───────────────────────────────────

    #[test]
    fn test_cdn_hash_determinism() {
        let scene = make_scene(12, 6);
        let c1 = scene_to_cdn(&scene);
        let c2 = scene_to_cdn(&scene);
        assert_eq!(c1.content_hash, c2.content_hash);
    }

    // ── Test 10: Mixed body types in analytics ─────────────────────────

    #[test]
    fn test_mixed_body_types_analytics() {
        let mut scene = make_scene(6, 0);
        // Set body types: 3 dynamic, 2 static, 1 kinematic.
        scene.bodies[0].body_type = 0;
        scene.bodies[1].body_type = 0;
        scene.bodies[2].body_type = 0;
        scene.bodies[3].body_type = 1;
        scene.bodies[4].body_type = 1;
        scene.bodies[5].body_type = 2;

        let evt = scene_config_to_analytics(&scene);
        assert_eq!(evt.body_type_distribution, [3, 2, 1]);
    }
}
