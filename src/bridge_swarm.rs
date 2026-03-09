//! Swarm bridges — Swarm ↔ DB, Cache, Analytics, Physics, Navigation
//!
//! 5 bridges connecting multi-agent swarm coordination to the ALICE ecosystem.
//! Covers formation log persistence, agent state caching, swarm metrics,
//! physics collision descriptors, and navigation path handoff.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Swarm → DB (formation log) ────────────────────────────────

/// Swarm formation log record for ALICE-DB persistence.
///
/// One record per formation snapshot. `content_hash` is derived from
/// `swarm_id + tick` so each tick is individually addressable.
pub struct SwarmDbFormationRecord {
    /// FNV-1a hash of swarm_id + tick — deduplication key.
    pub content_hash: u64,
    /// Unique swarm identifier.
    pub swarm_id: u32,
    /// Simulation tick at which the snapshot was taken.
    pub tick: u64,
    /// Number of active agents in the swarm.
    pub agent_count: u32,
    /// Centroid X coordinate in fixed-point millimetres.
    pub centroid_x_mm: i64,
    /// Centroid Y coordinate in fixed-point millimetres.
    pub centroid_y_mm: i64,
    /// Average agent speed in millimetres per tick.
    pub avg_speed_mm_per_tick: u32,
    /// Spatial spread (RMS distance from centroid) in millimetres.
    pub spread_mm: u32,
    /// Order parameter in permille (1000 = fully aligned, 0 = random).
    pub order_param_permille: u16,
}

/// Build a `SwarmDbFormationRecord` for a formation snapshot.
#[inline]
#[must_use]
pub fn swarm_to_db_formation_record(
    swarm_id: u32,
    tick: u64,
    agent_count: u32,
    centroid_x_mm: i64,
    centroid_y_mm: i64,
    avg_speed_mm_per_tick: u32,
    spread_mm: u32,
    order_param_permille: u16,
) -> SwarmDbFormationRecord {
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&swarm_id.to_le_bytes());
    buf[4..12].copy_from_slice(&tick.to_le_bytes());
    let content_hash = fnv1a(&buf);
    SwarmDbFormationRecord {
        content_hash,
        swarm_id,
        tick,
        agent_count,
        centroid_x_mm,
        centroid_y_mm,
        avg_speed_mm_per_tick,
        spread_mm,
        order_param_permille,
    }
}

// ── Bridge 2: Swarm → Cache (agent state cache) ──────────────────────────

/// Agent state cache entry for ALICE-Cache.
///
/// Per-agent position and velocity snapshot stored for fast neighbour queries.
/// TTL is shorter for high-speed agents (state becomes stale faster).
pub struct SwarmAgentCacheEntry {
    /// FNV-1a hash of swarm_id + agent_id — cache lookup key.
    pub content_hash: u64,
    /// Swarm identifier.
    pub swarm_id: u32,
    /// Agent identifier within the swarm.
    pub agent_id: u32,
    /// Position X in millimetres.
    pub pos_x_mm: i64,
    /// Position Y in millimetres.
    pub pos_y_mm: i64,
    /// Velocity magnitude in millimetres per tick.
    pub speed_mm_per_tick: u32,
    /// Heading direction in milli-degrees (0–359999).
    pub heading_milli_deg: u32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build a `SwarmAgentCacheEntry` for one agent.
///
/// TTL is 5 s for agents with `speed_mm_per_tick >= 100` (fast-moving)
/// and 30 s otherwise (branchless).
#[inline]
#[must_use]
pub fn swarm_to_agent_cache_entry(
    swarm_id: u32,
    agent_id: u32,
    pos_x_mm: i64,
    pos_y_mm: i64,
    speed_mm_per_tick: u32,
    heading_milli_deg: u32,
) -> SwarmAgentCacheEntry {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&swarm_id.to_le_bytes());
    buf[4..8].copy_from_slice(&agent_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    // Branchless TTL: fast agent (speed >= 100 mm/tick) → 5s, slow → 30s.
    let is_fast = (speed_mm_per_tick >= 100) as u32;
    let ttl_secs = 30 - is_fast * 25;
    SwarmAgentCacheEntry {
        content_hash,
        swarm_id,
        agent_id,
        pos_x_mm,
        pos_y_mm,
        speed_mm_per_tick,
        heading_milli_deg,
        ttl_secs,
    }
}

// ── Bridge 3: Swarm → Analytics (swarm metrics) ─────────────────────────

/// Swarm analytics metrics event for ALICE-Analytics.
///
/// Emitted each simulation tick so the analytics layer can track
/// emergent behaviour, convergence, and throughput.
pub struct SwarmAnalyticsMetrics {
    /// FNV-1a hash of swarm_id + tick — deduplication key.
    pub content_hash: u64,
    /// Swarm identifier.
    pub swarm_id: u32,
    /// Simulation tick.
    pub tick: u64,
    /// Number of active agents.
    pub agent_count: u32,
    /// Number of inter-agent collisions detected this tick.
    pub collision_count: u32,
    /// Mean neighbour count per agent (permille — actual = value / 1000).
    pub mean_neighbours_permille: u32,
    /// Order parameter in permille (1000 = fully aligned).
    pub order_param_permille: u16,
    /// Tick computation time in microseconds.
    pub compute_time_us: u32,
    /// Spatial spread in millimetres.
    pub spread_mm: u32,
}

/// Build a `SwarmAnalyticsMetrics` event for one simulation tick.
#[inline]
#[must_use]
pub fn swarm_to_analytics_metrics(
    swarm_id: u32,
    tick: u64,
    agent_count: u32,
    collision_count: u32,
    mean_neighbours_permille: u32,
    order_param_permille: u16,
    compute_time_us: u32,
    spread_mm: u32,
) -> SwarmAnalyticsMetrics {
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&swarm_id.to_le_bytes());
    buf[4..12].copy_from_slice(&tick.to_le_bytes());
    let content_hash = fnv1a(&buf);
    SwarmAnalyticsMetrics {
        content_hash,
        swarm_id,
        tick,
        agent_count,
        collision_count,
        mean_neighbours_permille,
        order_param_permille,
        compute_time_us,
        spread_mm,
    }
}

// ── Bridge 4: Swarm → Physics (collision descriptor) ────────────────────

/// Per-agent physics collision descriptor for ALICE-Physics.
///
/// Supplies the physics engine with the bounding volume and mass of each
/// agent so it can perform broadphase and narrowphase collision detection.
pub struct SwarmPhysicsAgent {
    /// FNV-1a hash of swarm_id + agent_id — physics body key.
    pub content_hash: u64,
    /// Swarm identifier.
    pub swarm_id: u32,
    /// Agent identifier.
    pub agent_id: u32,
    /// Position X in millimetres.
    pub pos_x_mm: i64,
    /// Position Y in millimetres.
    pub pos_y_mm: i64,
    /// Bounding radius in millimetres.
    pub radius_mm: u32,
    /// Agent mass in grams.
    pub mass_g: u32,
    /// Velocity X component in millimetres per tick.
    pub vel_x_mm: i32,
    /// Velocity Y component in millimetres per tick.
    pub vel_y_mm: i32,
}

/// Build a `SwarmPhysicsAgent` descriptor for the physics engine.
#[inline]
#[must_use]
pub fn swarm_to_physics_agent(
    swarm_id: u32,
    agent_id: u32,
    pos_x_mm: i64,
    pos_y_mm: i64,
    radius_mm: u32,
    mass_g: u32,
    vel_x_mm: i32,
    vel_y_mm: i32,
) -> SwarmPhysicsAgent {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&swarm_id.to_le_bytes());
    buf[4..8].copy_from_slice(&agent_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    SwarmPhysicsAgent {
        content_hash,
        swarm_id,
        agent_id,
        pos_x_mm,
        pos_y_mm,
        radius_mm,
        mass_g,
        vel_x_mm,
        vel_y_mm,
    }
}

// ── Bridge 5: Swarm → Navigation (path handoff) ──────────────────────────

/// Per-agent navigation goal for ALICE-Navigation.
///
/// Hands off a goal position and behavioural hints to the navigation planner
/// so each swarm agent can receive an individually planned path.
pub struct SwarmNavigationGoal {
    /// FNV-1a hash of swarm_id + agent_id + goal position — goal identity key.
    pub content_hash: u64,
    /// Swarm identifier.
    pub swarm_id: u32,
    /// Agent identifier.
    pub agent_id: u32,
    /// Current position X in millimetres.
    pub from_x_mm: i64,
    /// Current position Y in millimetres.
    pub from_y_mm: i64,
    /// Goal position X in millimetres.
    pub goal_x_mm: i64,
    /// Goal position Y in millimetres.
    pub goal_y_mm: i64,
    /// Maximum planning time budget in microseconds.
    pub time_budget_us: u32,
    /// Agent bounding radius for clearance checks.
    pub clearance_mm: u32,
    /// Priority level (0 = lowest, 255 = highest).
    pub priority: u8,
}

/// Build a `SwarmNavigationGoal` for one agent's path request.
#[inline]
#[must_use]
pub fn swarm_to_navigation_goal(
    swarm_id: u32,
    agent_id: u32,
    from_x_mm: i64,
    from_y_mm: i64,
    goal_x_mm: i64,
    goal_y_mm: i64,
    time_budget_us: u32,
    clearance_mm: u32,
    priority: u8,
) -> SwarmNavigationGoal {
    let mut buf = [0u8; 32];
    buf[0..4].copy_from_slice(&swarm_id.to_le_bytes());
    buf[4..8].copy_from_slice(&agent_id.to_le_bytes());
    buf[8..16].copy_from_slice(&goal_x_mm.to_le_bytes());
    buf[16..24].copy_from_slice(&goal_y_mm.to_le_bytes());
    buf[24..28].copy_from_slice(&from_x_mm.to_le_bytes()[..4]);
    buf[28..32].copy_from_slice(&from_y_mm.to_le_bytes()[..4]);
    let content_hash = fnv1a(&buf);
    SwarmNavigationGoal {
        content_hash,
        swarm_id,
        agent_id,
        from_x_mm,
        from_y_mm,
        goal_x_mm,
        goal_y_mm,
        time_budget_us,
        clearance_mm,
        priority,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_db_formation_hash_nonzero() {
        let rec = swarm_to_db_formation_record(1, 100, 50, 0, 0, 10, 200, 800);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_swarm_db_formation_deterministic() {
        let a = swarm_to_db_formation_record(1, 100, 50, 1000, -500, 10, 200, 800);
        let b = swarm_to_db_formation_record(1, 100, 50, 1000, -500, 10, 200, 800);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_swarm_agent_cache_fast_ttl() {
        let entry = swarm_to_agent_cache_entry(1, 0, 0, 0, 150, 0);
        assert_eq!(entry.ttl_secs, 5);
    }

    #[test]
    fn test_swarm_agent_cache_slow_ttl() {
        let entry = swarm_to_agent_cache_entry(1, 0, 0, 0, 50, 0);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_swarm_analytics_metrics_fields() {
        let m = swarm_to_analytics_metrics(2, 500, 100, 3, 2000, 750, 1500, 5000);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.agent_count, 100);
        assert_eq!(m.order_param_permille, 750);
    }

    #[test]
    fn test_swarm_physics_agent_fields() {
        let a = swarm_to_physics_agent(1, 7, 1000, -2000, 50, 250, 5, -3);
        assert_ne!(a.content_hash, 0);
        assert_eq!(a.radius_mm, 50);
        assert_eq!(a.mass_g, 250);
        assert_eq!(a.vel_x_mm, 5);
    }

    #[test]
    fn test_swarm_navigation_goal_hash_nonzero() {
        let g = swarm_to_navigation_goal(1, 3, 0, 0, 10_000, 20_000, 5_000, 50, 200);
        assert_ne!(g.content_hash, 0);
    }

    #[test]
    fn test_swarm_navigation_goal_different_goals_differ() {
        let g1 = swarm_to_navigation_goal(1, 3, 0, 0, 10_000, 20_000, 5_000, 50, 200);
        let g2 = swarm_to_navigation_goal(1, 3, 0, 0, 99_000, 20_000, 5_000, 50, 200);
        assert_ne!(g1.content_hash, g2.content_hash);
    }
}
