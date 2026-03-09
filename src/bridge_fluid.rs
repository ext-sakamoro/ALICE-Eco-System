//! Fluid bridges — ALICE-Fluid ↔ DB, Cache, Analytics, Physics, Render
//!
//! 5 bridges connecting fluid simulation to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Fluid → DB (simulation record) ─────────────────────────────

/// Simulation record for ALICE-DB persistence.
pub struct FluidDbRecord {
    /// Content hash over the simulation configuration.
    pub content_hash: u64,
    /// Number of simulated particles.
    pub particle_count: u64,
    /// Grid resolution in the X axis.
    pub grid_x: u32,
    /// Grid resolution in the Y axis.
    pub grid_y: u32,
    /// Grid resolution in the Z axis.
    pub grid_z: u32,
    /// Dynamic viscosity multiplied by 1000.
    pub viscosity_x1000: u32,
    /// Simulation time step in microseconds.
    pub time_step_us: u64,
}

/// Serialize a fluid simulation configuration for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn fluid_to_db_record(
    particle_count: u64,
    grid_x: u32,
    grid_y: u32,
    grid_z: u32,
    viscosity_x1000: u32,
    time_step_us: u64,
) -> FluidDbRecord {
    let mut buf = [0u8; 36];
    buf[0..8].copy_from_slice(&particle_count.to_le_bytes());
    buf[8..12].copy_from_slice(&grid_x.to_le_bytes());
    buf[12..16].copy_from_slice(&grid_y.to_le_bytes());
    buf[16..20].copy_from_slice(&grid_z.to_le_bytes());
    buf[20..24].copy_from_slice(&viscosity_x1000.to_le_bytes());
    buf[24..32].copy_from_slice(&time_step_us.to_le_bytes());
    FluidDbRecord {
        content_hash: fnv1a(&buf[..32]),
        particle_count,
        grid_x,
        grid_y,
        grid_z,
        viscosity_x1000,
        time_step_us,
    }
}

// ── Bridge 2: Fluid → Cache (frame state cache) ───────────────────────────

/// Frame state cache entry for ALICE-Cache.
pub struct FluidCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Simulation frame index.
    pub frame_index: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Number of particles in this frame.
    pub particle_count: u64,
    /// Byte size of the serialised frame.
    pub frame_bytes: u64,
}

/// Build a frame state cache entry for ALICE-Cache.
///
/// Early frames (low frame index) receive a longer TTL (600 s vs 120 s)
/// because they serve as simulation restart points.
#[inline]
#[must_use]
pub fn fluid_to_cache_entry(
    frame_index: u64,
    particle_count: u64,
    frame_bytes: u64,
) -> FluidCacheEntry {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&frame_index.to_le_bytes());
    buf[8..16].copy_from_slice(&particle_count.to_le_bytes());
    buf[16..24].copy_from_slice(&frame_bytes.to_le_bytes());
    let late_frame = (frame_index > 1_000) as u32;
    let ttl_secs = 600 - late_frame * 480;
    FluidCacheEntry {
        content_hash: fnv1a(&buf),
        frame_index,
        ttl_secs,
        particle_count,
        frame_bytes,
    }
}

// ── Bridge 3: Fluid → Analytics (solver performance event) ───────────────

/// Solver performance event for ALICE-Analytics ingestion.
pub struct FluidAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of particles processed.
    pub particle_count: u64,
    /// Solver step duration in microseconds.
    pub sim_time_us: u64,
    /// Number of pressure solver iterations.
    pub pressure_iters: u32,
    /// Velocity divergence multiplied by 1000.
    pub divergence_x1000: u32,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a solver performance event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn fluid_to_analytics_event(
    particle_count: u64,
    sim_time_us: u64,
    pressure_iters: u32,
    divergence_x1000: u32,
    timestamp_ms: u64,
) -> FluidAnalyticsEvent {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&particle_count.to_le_bytes());
    buf[8..16].copy_from_slice(&sim_time_us.to_le_bytes());
    buf[16..20].copy_from_slice(&pressure_iters.to_le_bytes());
    buf[20..24].copy_from_slice(&divergence_x1000.to_le_bytes());
    buf[24..32].copy_from_slice(&timestamp_ms.to_le_bytes());
    FluidAnalyticsEvent {
        content_hash: fnv1a(&buf),
        particle_count,
        sim_time_us,
        pressure_iters,
        divergence_x1000,
        timestamp_ms,
    }
}

// ── Bridge 4: Fluid → Physics (state vector) ─────────────────────────────

/// Physical state vector for ALICE-Physics integration.
pub struct FluidPhysicsState {
    /// Content hash over the state vector.
    pub content_hash: u64,
    /// Number of particles in the state.
    pub particle_count: u64,
    /// Maximum particle velocity multiplied by 1000.
    pub velocity_max_x1000: u32,
    /// Maximum pressure multiplied by 1000.
    pub pressure_max_x1000: u32,
    /// Total kinetic energy multiplied by 1000.
    pub energy_x1000: u64,
}

/// Build a physical state vector for ALICE-Physics integration.
#[inline]
#[must_use]
pub fn fluid_to_physics_state(
    particle_count: u64,
    velocity_max_x1000: u32,
    pressure_max_x1000: u32,
    energy_x1000: u64,
) -> FluidPhysicsState {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&particle_count.to_le_bytes());
    buf[8..12].copy_from_slice(&velocity_max_x1000.to_le_bytes());
    buf[12..16].copy_from_slice(&pressure_max_x1000.to_le_bytes());
    buf[16..24].copy_from_slice(&energy_x1000.to_le_bytes());
    FluidPhysicsState {
        content_hash: fnv1a(&buf),
        particle_count,
        velocity_max_x1000,
        pressure_max_x1000,
        energy_x1000,
    }
}

// ── Bridge 5: Fluid → Render (surface mesh frame) ────────────────────────

/// Surface mesh frame for ALICE-Render.
pub struct FluidRenderFrame {
    /// Content hash over the render payload.
    pub content_hash: u64,
    /// Number of source particles.
    pub particle_count: u64,
    /// Number of surface mesh vertices.
    pub vertex_count: u64,
    /// Render latency in microseconds.
    pub render_time_us: u64,
    /// Byte size of the rendered frame.
    pub frame_bytes: u64,
}

/// Build a surface mesh frame for ALICE-Render.
#[inline]
#[must_use]
pub fn fluid_to_render_frame(
    particle_count: u64,
    vertex_count: u64,
    render_time_us: u64,
    frame_bytes: u64,
) -> FluidRenderFrame {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&particle_count.to_le_bytes());
    buf[8..16].copy_from_slice(&vertex_count.to_le_bytes());
    buf[16..24].copy_from_slice(&render_time_us.to_le_bytes());
    buf[24..32].copy_from_slice(&frame_bytes.to_le_bytes());
    FluidRenderFrame {
        content_hash: fnv1a(&buf),
        particle_count,
        vertex_count,
        render_time_us,
        frame_bytes,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fluid_db_record_hash_nonzero() {
        let rec = fluid_to_db_record(1_000_000, 128, 128, 64, 1_000, 16_667);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_fluid_db_record_fields() {
        let rec = fluid_to_db_record(500_000, 64, 64, 32, 890, 33_333);
        assert_eq!(rec.particle_count, 500_000);
        assert_eq!(rec.grid_x, 64);
        assert_eq!(rec.grid_z, 32);
        assert_eq!(rec.viscosity_x1000, 890);
    }

    #[test]
    fn test_fluid_db_record_determinism() {
        let a = fluid_to_db_record(200_000, 32, 32, 32, 1_000, 16_667);
        let b = fluid_to_db_record(200_000, 32, 32, 32, 1_000, 16_667);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_fluid_cache_entry_early_frame_ttl() {
        let entry = fluid_to_cache_entry(100, 500_000, 24_000_000);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 600);
    }

    #[test]
    fn test_fluid_cache_entry_late_frame_ttl() {
        let entry = fluid_to_cache_entry(5_000, 500_000, 24_000_000);
        assert_eq!(entry.ttl_secs, 120);
        assert_eq!(entry.frame_index, 5_000);
    }

    #[test]
    fn test_fluid_analytics_event() {
        let ev = fluid_to_analytics_event(1_000_000, 8_333, 50, 2, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.pressure_iters, 50);
        assert_eq!(ev.divergence_x1000, 2);
    }

    #[test]
    fn test_fluid_physics_state() {
        let s = fluid_to_physics_state(1_000_000, 3_500, 101_000, 7_500_000);
        assert_ne!(s.content_hash, 0);
        assert_eq!(s.velocity_max_x1000, 3_500);
        assert_eq!(s.energy_x1000, 7_500_000);
    }

    #[test]
    fn test_fluid_render_frame() {
        let f = fluid_to_render_frame(500_000, 2_000_000, 16_000, 48_000_000);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.vertex_count, 2_000_000);
        assert_eq!(f.frame_bytes, 48_000_000);
    }

    #[test]
    fn test_fluid_render_frame_determinism() {
        let a = fluid_to_render_frame(100_000, 400_000, 8_000, 12_000_000);
        let b = fluid_to_render_frame(100_000, 400_000, 8_000, 12_000_000);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
