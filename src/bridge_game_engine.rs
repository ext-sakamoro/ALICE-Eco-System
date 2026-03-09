//! GameEngine bridges — ALICE-GameEngine ↔ DB, Cache, Analytics, Physics, Render
//!
//! 5 bridges connecting game engine scene and frame data (extracted as
//! primitives) to the ALICE ecosystem. No external crate types are imported;
//! all fields use primitive types derived from serialised game engine state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: GameEngine → DB (scene snapshot persistence) ────────────────

/// Game engine scene snapshot for ALICE-DB persistence.
pub struct GameEngineDbRecord {
    /// Content hash over scene_hash, entity_count, and timestamp_ms.
    pub content_hash: u64,
    /// Opaque scene identifier hash.
    pub scene_hash: u64,
    /// Number of active entities in the scene.
    pub entity_count: u64,
    /// Total component count across all entities.
    pub component_count: u64,
    /// Total bytes of loaded asset data.
    pub asset_bytes: u64,
    /// Unix timestamp in milliseconds when the snapshot was captured.
    pub timestamp_ms: u64,
}

/// Build a DB persistence record from extracted game engine scene data.
#[inline]
#[must_use]
pub fn game_engine_to_db_record(
    scene_hash: u64,
    entity_count: u64,
    component_count: u64,
    asset_bytes: u64,
    timestamp_ms: u64,
) -> GameEngineDbRecord {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&scene_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&entity_count.to_le_bytes());
    buf[16..24].copy_from_slice(&timestamp_ms.to_le_bytes());
    GameEngineDbRecord {
        content_hash: fnv1a(&buf),
        scene_hash,
        entity_count,
        component_count,
        asset_bytes,
        timestamp_ms,
    }
}

// ── Bridge 2: GameEngine → Cache (live scene state caching) ───────────────

/// Cached game engine scene state entry for ALICE-Cache.
pub struct GameEngineCacheEntry {
    /// Content hash over scene_hash and tick_count.
    pub content_hash: u64,
    /// Hashed scene identifier used as cache key.
    pub scene_hash: u64,
    /// TTL in seconds for this cache entry.
    pub ttl_secs: u32,
    /// Serialised state size in bytes.
    pub state_bytes: u64,
    /// Simulation tick count at the time of caching.
    pub tick_count: u64,
}

/// Build a cache entry for a game engine scene's live state.
///
/// TTL is 30 s by default; reduced to 5 s when `state_bytes` exceeds 10 MB
/// to limit memory pressure from large scenes.
#[inline]
#[must_use]
pub fn game_engine_to_cache_entry(
    scene_hash: u64,
    state_bytes: u64,
    tick_count: u64,
) -> GameEngineCacheEntry {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&scene_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&tick_count.to_le_bytes());
    let large = (state_bytes > 10_000_000) as u32;
    let ttl_secs = 30 - large * 25;
    GameEngineCacheEntry {
        content_hash: fnv1a(&buf),
        scene_hash,
        ttl_secs,
        state_bytes,
        tick_count,
    }
}

// ── Bridge 3: GameEngine → Analytics (frame metrics ingestion) ────────────

/// Game engine frame metrics event for ALICE-Analytics ingestion.
pub struct GameEngineAnalyticsEvent {
    /// Content hash over scene_hash and timestamp_ms.
    pub content_hash: u64,
    /// Frames per second at the time of the event.
    pub fps: u32,
    /// Number of draw calls issued for the frame.
    pub draw_calls: u32,
    /// Number of active entities during the frame.
    pub entity_count: u64,
    /// Frame rendering time in microseconds.
    pub frame_time_us: u64,
    /// Unix timestamp in milliseconds when the event was recorded.
    pub timestamp_ms: u64,
}

/// Build an analytics ingestion event from game engine frame metrics.
#[inline]
#[must_use]
pub fn game_engine_to_analytics_event(
    scene_hash: u64,
    fps: u32,
    draw_calls: u32,
    entity_count: u64,
    frame_time_us: u64,
    timestamp_ms: u64,
) -> GameEngineAnalyticsEvent {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&scene_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    GameEngineAnalyticsEvent {
        content_hash: fnv1a(&buf),
        fps,
        draw_calls,
        entity_count,
        frame_time_us,
        timestamp_ms,
    }
}

// ── Bridge 4: GameEngine → Physics (simulation link) ──────────────────────

/// Game engine physics simulation link for ALICE-Physics.
pub struct GameEnginePhysicsLink {
    /// Content hash over scene_hash and step_time_us.
    pub content_hash: u64,
    /// Number of active rigid bodies in the simulation.
    pub body_count: u64,
    /// Number of collision events detected in the last step.
    pub collision_count: u32,
    /// Physics step duration in microseconds.
    pub step_time_us: u64,
    /// Number of solver substeps per physics tick.
    pub substeps: u8,
}

/// Build a physics simulation link from game engine state.
#[inline]
#[must_use]
pub fn game_engine_to_physics_link(
    scene_hash: u64,
    body_count: u64,
    collision_count: u32,
    step_time_us: u64,
    substeps: u8,
) -> GameEnginePhysicsLink {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&scene_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&step_time_us.to_le_bytes());
    GameEnginePhysicsLink {
        content_hash: fnv1a(&buf),
        body_count,
        collision_count,
        step_time_us,
        substeps,
    }
}

// ── Bridge 5: GameEngine → Render (frame submission) ──────────────────────

/// Game engine render frame descriptor for ALICE-Render submission.
pub struct GameEngineRenderFrame {
    /// Content hash over scene_hash and gpu_time_us.
    pub content_hash: u64,
    /// Number of draw calls issued for this frame.
    pub draw_calls: u32,
    /// Total triangles submitted to the GPU.
    pub triangle_count: u64,
    /// GPU-side rendering time in microseconds.
    pub gpu_time_us: u64,
    /// Total frame data transmitted to the GPU in bytes.
    pub frame_bytes: u64,
}

/// Build a render frame descriptor from game engine frame data.
#[inline]
#[must_use]
pub fn game_engine_to_render_frame(
    scene_hash: u64,
    draw_calls: u32,
    triangle_count: u64,
    gpu_time_us: u64,
    frame_bytes: u64,
) -> GameEngineRenderFrame {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&scene_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&gpu_time_us.to_le_bytes());
    GameEngineRenderFrame {
        content_hash: fnv1a(&buf),
        draw_calls,
        triangle_count,
        gpu_time_us,
        frame_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = game_engine_to_db_record(0xABCD_1234, 100, 500, 1_048_576, 1_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn db_record_fields_preserved() {
        let rec = game_engine_to_db_record(0x1111, 42, 210, 2_097_152, 9_999);
        assert_eq!(rec.scene_hash, 0x1111);
        assert_eq!(rec.entity_count, 42);
        assert_eq!(rec.component_count, 210);
        assert_eq!(rec.asset_bytes, 2_097_152);
        assert_eq!(rec.timestamp_ms, 9_999);
    }

    #[test]
    fn db_record_hash_deterministic() {
        let a = game_engine_to_db_record(0x42, 10, 50, 1024, 0);
        let b = game_engine_to_db_record(0x42, 10, 50, 1024, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_small_scene_long_ttl() {
        let entry = game_engine_to_cache_entry(0xBEEF, 1_000_000, 100);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn cache_entry_large_scene_short_ttl() {
        let entry = game_engine_to_cache_entry(0xBEEF, 20_000_000, 200);
        assert_eq!(entry.ttl_secs, 5);
    }

    #[test]
    fn cache_entry_hash_nonzero() {
        let entry = game_engine_to_cache_entry(0x1, 512, 1);
        assert_ne!(entry.content_hash, 0);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_fields_preserved() {
        let ev = game_engine_to_analytics_event(0xCAFE, 60, 1024, 500, 16_667, 7_000_000);
        assert_eq!(ev.fps, 60);
        assert_eq!(ev.draw_calls, 1024);
        assert_eq!(ev.entity_count, 500);
        assert_eq!(ev.frame_time_us, 16_667);
        assert_eq!(ev.timestamp_ms, 7_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    // ── Physics link tests ────────────────────────────────────────────────

    #[test]
    fn physics_link_fields_and_hash() {
        let link = game_engine_to_physics_link(0xDEAD, 300, 12, 8_333, 4);
        assert_eq!(link.body_count, 300);
        assert_eq!(link.collision_count, 12);
        assert_eq!(link.step_time_us, 8_333);
        assert_eq!(link.substeps, 4);
        assert_ne!(link.content_hash, 0);
    }

    // ── Render frame tests ────────────────────────────────────────────────

    #[test]
    fn render_frame_fields_and_hash() {
        let frame = game_engine_to_render_frame(0xF00D, 2048, 1_500_000, 6_000, 4_194_304);
        assert_eq!(frame.draw_calls, 2048);
        assert_eq!(frame.triangle_count, 1_500_000);
        assert_eq!(frame.gpu_time_us, 6_000);
        assert_eq!(frame.frame_bytes, 4_194_304);
        assert_ne!(frame.content_hash, 0);
    }

    #[test]
    fn different_inputs_produce_different_hashes() {
        let a = game_engine_to_db_record(0x01, 1, 1, 1, 1);
        let b = game_engine_to_db_record(0x02, 1, 1, 1, 1);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
