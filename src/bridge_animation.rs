//! Animation bridges — ALICE-Animation ↔ SDF, CDN, Cache, DB, Sync, View, Codec, ML
//!
//! 8 bridges connecting SDF anime direction engine to the ALICE ecosystem.

use alice_animation::{SceneGraph, Director};

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Animation → SDF (scene → SDF evaluation) ─────────────────

/// SDF scene evaluation stats from ALICE-Animation.
pub struct AnimSdfScene {
    /// Number of scene graph nodes.
    pub node_count: usize,
    /// Number of actors.
    pub actor_count: usize,
    /// Scene duration in seconds.
    pub duration_secs: f32,
}

/// Evaluate Animation scene for ALICE-SDF integration.
pub fn animation_to_sdf_scene(scene: &SceneGraph, _time: f32) -> AnimSdfScene {
    let actors = scene.actor_count();
    AnimSdfScene { node_count: actors, actor_count: actors, duration_secs: 0.0 }
}

// ── Bridge 2: Animation → CDN (episode delivery) ────────────────────────

/// Episode delivery package for ALICE-CDN.
pub struct AnimCdnPackage {
    /// Episode name.
    pub episode_name: String,
    /// Content hash.
    pub content_hash: u64,
    /// Episode duration.
    pub duration_secs: f32,
    /// Number of cuts.
    pub cut_count: usize,
}

/// Package episode for ALICE-CDN delivery.
pub fn animation_to_cdn_package(director: &Director) -> AnimCdnPackage {
    let dur = director.duration();
    let name = "episode".to_string();
    let data = format!("{}_{}", name, dur);
    AnimCdnPackage {
        episode_name: name,
        content_hash: fnv1a(data.as_bytes()),
        duration_secs: dur,
        cut_count: 0,
    }
}

// ── Bridge 3: Animation → Cache (scene state caching) ───────────────────

/// Cached scene state for ALICE-Cache.
pub struct AnimCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Actor count.
    pub actor_count: usize,
    /// Time key (quantized to 10ms).
    pub time_key: u32,
}

/// Cache Animation scene state for ALICE-Cache.
pub fn animation_to_cache_entry(scene: &SceneGraph, time: f32) -> AnimCacheEntry {
    let actors = scene.actor_count();
    let time_key = (time * 100.0) as u32;
    let data = [actors.to_le_bytes().as_slice(), &time_key.to_le_bytes()].concat();
    AnimCacheEntry { content_hash: fnv1a(&data), actor_count: actors, time_key }
}

// ── Bridge 4: Animation → DB (scene persistence) ────────────────────────

/// Scene persistence record for ALICE-DB.
pub struct AnimDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Actor count.
    pub actor_count: usize,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// Cut count.
    pub cut_count: usize,
}

/// Serialize Animation director for ALICE-DB persistence.
pub fn animation_to_db_record(director: &Director) -> AnimDbRecord {
    let dur = director.duration();
    let data = dur.to_le_bytes();
    AnimDbRecord {
        content_hash: fnv1a(&data),
        actor_count: 0,
        duration_secs: dur,
        cut_count: 0,
    }
}

// ── Bridge 5: Animation → Sync (collaborative editing) ──────────────────

/// Collaborative editing packet for ALICE-Sync.
pub struct AnimSyncPacket {
    /// Actor count.
    pub actor_count: usize,
    /// Current time.
    pub time: f32,
    /// Content hash.
    pub content_hash: u64,
    /// Player slot.
    pub player_slot: u8,
}

/// Package scene state for ALICE-Sync collaborative editing.
pub fn animation_to_sync_packet(scene: &SceneGraph, time: f32, player_slot: u8) -> AnimSyncPacket {
    let actors = scene.actor_count();
    let data = [&actors.to_le_bytes()[..], &time.to_le_bytes()].concat();
    AnimSyncPacket { actor_count: actors, time, content_hash: fnv1a(&data), player_slot }
}

// ── Bridge 6: Animation → View (render pipeline config) ─────────────────

/// Render pipeline config for ALICE-View.
pub struct AnimViewConfig {
    /// Actor count.
    pub actor_count: usize,
    /// Whether camera is active.
    pub camera_active: bool,
    /// Current time.
    pub time: f32,
    /// Whether NPR shading needed.
    pub needs_npr: bool,
}

/// Configure render pipeline for ALICE-View.
pub fn animation_to_view_config(scene: &SceneGraph, time: f32) -> AnimViewConfig {
    AnimViewConfig {
        actor_count: scene.actor_count(),
        camera_active: true,
        time,
        needs_npr: true, // anime always uses NPR
    }
}

// ── Bridge 7: Animation → Codec (episode compression config) ────────────

/// Episode compression config for ALICE-Codec.
pub struct AnimCodecConfig {
    /// Total frame count.
    pub frame_count: usize,
    /// Keyframe interval.
    pub keyframe_interval: usize,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// Estimated bitrate (bits/sec).
    pub estimated_bitrate: usize,
}

/// Configure episode compression for ALICE-Codec.
pub fn animation_to_codec_config(director: &Director, fps: usize) -> AnimCodecConfig {
    let dur = director.duration();
    let frames = (dur * fps as f32) as usize;
    AnimCodecConfig {
        frame_count: frames,
        keyframe_interval: fps, // one keyframe per second
        duration_secs: dur,
        estimated_bitrate: frames * 1024, // rough estimate
    }
}

// ── Bridge 8: Animation → ML (AI-assisted direction features) ───────────

/// ML feature vector from Animation scene for AI direction.
pub struct AnimMlFeatures {
    /// Actor count (normalized).
    pub actor_count: f32,
    /// Scene complexity estimate.
    pub scene_complexity: f32,
    /// Time normalized (0.0-1.0).
    pub time_normalized: f32,
    /// Feature vector for ML input.
    pub feature_vec: Vec<f32>,
}

/// Extract ML features from Animation scene for AI direction.
pub fn animation_to_ml_features(scene: &SceneGraph, time: f32, duration: f32) -> AnimMlFeatures {
    let actors = scene.actor_count() as f32;
    let complexity = actors * 0.1;
    let t_norm = if duration > 0.0 { (time / duration).clamp(0.0, 1.0) } else { 0.0 };
    AnimMlFeatures {
        actor_count: actors,
        scene_complexity: complexity,
        time_normalized: t_norm,
        feature_vec: vec![actors, complexity, t_norm],
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_animation::{Actor, Cut};

    fn test_scene() -> SceneGraph {
        let mut scene = SceneGraph::new();
        let sdf = alice_sdf::SdfNode::sphere(1.0);
        scene.add_actor(Actor::new("actor1", sdf.clone()));
        scene.add_actor(Actor::new("actor2", sdf));
        scene
    }

    fn test_director() -> Director {
        let mut dir = Director::new("test_episode");
        dir.add_cut(Cut::new("cut1", 0.0, 5.0));
        dir.add_cut(Cut::new("cut2", 5.0, 10.0));
        dir
    }

    #[test]
    fn test_animation_to_sdf_scene() {
        let scene = test_scene();
        let result = animation_to_sdf_scene(&scene, 1.0);
        assert_eq!(result.actor_count, 2);
    }

    #[test]
    fn test_animation_to_cdn_package() {
        let dir = test_director();
        let pkg = animation_to_cdn_package(&dir);
        assert_ne!(pkg.content_hash, 0);
        assert!(pkg.duration_secs > 0.0);
    }

    #[test]
    fn test_animation_to_cache_entry() {
        let scene = test_scene();
        let entry = animation_to_cache_entry(&scene, 2.5);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.actor_count, 2);
        assert_eq!(entry.time_key, 250);
    }

    #[test]
    fn test_animation_to_db_record() {
        let dir = test_director();
        let rec = animation_to_db_record(&dir);
        assert_ne!(rec.content_hash, 0);
        assert!(rec.duration_secs > 0.0);
    }

    #[test]
    fn test_animation_to_sync_packet() {
        let scene = test_scene();
        let pkt = animation_to_sync_packet(&scene, 1.0, 2);
        assert_eq!(pkt.actor_count, 2);
        assert_eq!(pkt.player_slot, 2);
    }

    #[test]
    fn test_animation_to_view_config() {
        let scene = test_scene();
        let cfg = animation_to_view_config(&scene, 0.5);
        assert_eq!(cfg.actor_count, 2);
        assert!(cfg.needs_npr);
    }

    #[test]
    fn test_animation_to_codec_config() {
        let dir = test_director();
        let cfg = animation_to_codec_config(&dir, 24);
        assert!(cfg.frame_count > 0);
        assert_eq!(cfg.keyframe_interval, 24);
    }

    #[test]
    fn test_animation_to_ml_features() {
        let scene = test_scene();
        let f = animation_to_ml_features(&scene, 3.0, 10.0);
        assert!((f.actor_count - 2.0).abs() < 0.01);
        assert!((f.time_normalized - 0.3).abs() < 0.01);
        assert_eq!(f.feature_vec.len(), 3);
    }
}
