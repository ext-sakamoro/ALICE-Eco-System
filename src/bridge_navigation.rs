//! Navigation bridges — Navigation ↔ DB, Cache, Analytics, Physics, Render
//!
//! 5 bridges connecting path-planning and map management to the ALICE ecosystem.
//! Covers map storage, path caching, routing metrics,
//! obstacle descriptors for physics, and visualisation geometry for render.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Navigation → DB (map storage) ─────────────────────────────

/// Navigation map storage record for ALICE-DB.
///
/// One record per map version. `content_hash` covers `map_id + version`
/// so each map revision is individually addressable.
pub struct NavigationDbMapRecord {
    /// FNV-1a hash of map_id + version — deduplication key.
    pub content_hash: u64,
    /// Unique map identifier.
    pub map_id: u32,
    /// Map schema version (bump on structural changes).
    pub version: u32,
    /// Number of grid cells (total = width_cells * height_cells).
    pub grid_cells: u64,
    /// Grid cell size in millimetres.
    pub cell_size_mm: u32,
    /// Number of static obstacles encoded in the map.
    pub obstacle_count: u32,
    /// Number of waypoints in the map graph.
    pub waypoint_count: u32,
    /// Map data checksum (FNV-1a of serialised obstacle list).
    pub map_data_hash: u64,
    /// Timestamp of last map update in microseconds since epoch.
    pub updated_us: u64,
}

/// Build a `NavigationDbMapRecord` for a map revision.
#[inline]
#[must_use]
pub fn navigation_to_db_map_record(
    map_id: u32,
    version: u32,
    grid_cells: u64,
    cell_size_mm: u32,
    obstacle_count: u32,
    waypoint_count: u32,
    map_data_hash: u64,
    updated_us: u64,
) -> NavigationDbMapRecord {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&map_id.to_le_bytes());
    buf[4..8].copy_from_slice(&version.to_le_bytes());
    let content_hash = fnv1a(&buf);
    NavigationDbMapRecord {
        content_hash,
        map_id,
        version,
        grid_cells,
        cell_size_mm,
        obstacle_count,
        waypoint_count,
        map_data_hash,
        updated_us,
    }
}

// ── Bridge 2: Navigation → Cache (path cache) ────────────────────────────

/// Planned path cache entry for ALICE-Cache.
///
/// Stores the result of an A*/Dijkstra/RRT plan so repeated queries
/// for the same start/goal pair can skip replanning.
/// TTL is shorter for paths passing through dynamic-obstacle zones.
pub struct NavigationPathCacheEntry {
    /// FNV-1a hash of map_id + start_cell + goal_cell — cache lookup key.
    pub content_hash: u64,
    /// Map identifier.
    pub map_id: u32,
    /// Start grid cell index.
    pub start_cell: u64,
    /// Goal grid cell index.
    pub goal_cell: u64,
    /// Planned path length in millimetres.
    pub path_length_mm: u64,
    /// Number of waypoints along the path.
    pub waypoint_count: u32,
    /// Planning time in microseconds (for analytics).
    pub planning_time_us: u32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Whether the path passes through a dynamic-obstacle zone.
    pub has_dynamic_obstacles: bool,
}

/// Build a `NavigationPathCacheEntry` for a completed plan.
///
/// TTL is 10 s if `has_dynamic_obstacles` and 120 s otherwise (branchless).
#[inline]
#[must_use]
pub fn navigation_to_path_cache_entry(
    map_id: u32,
    start_cell: u64,
    goal_cell: u64,
    path_length_mm: u64,
    waypoint_count: u32,
    planning_time_us: u32,
    has_dynamic_obstacles: bool,
) -> NavigationPathCacheEntry {
    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&map_id.to_le_bytes());
    buf[4..12].copy_from_slice(&start_cell.to_le_bytes());
    buf[12..20].copy_from_slice(&goal_cell.to_le_bytes());
    let content_hash = fnv1a(&buf);
    // Branchless TTL: dynamic obstacles → 10s, static only → 120s.
    let has_dyn_u32 = has_dynamic_obstacles as u32;
    let ttl_secs = 120 - has_dyn_u32 * 110;
    NavigationPathCacheEntry {
        content_hash,
        map_id,
        start_cell,
        goal_cell,
        path_length_mm,
        waypoint_count,
        planning_time_us,
        ttl_secs,
        has_dynamic_obstacles,
    }
}

// ── Bridge 3: Navigation → Analytics (routing metrics) ───────────────────

/// Routing metrics event for ALICE-Analytics.
///
/// Emitted after each completed planning request for latency and
/// quality-of-path analysis.
pub struct NavigationAnalyticsRoutingMetrics {
    /// FNV-1a hash of map_id + request_id — deduplication key.
    pub content_hash: u64,
    /// Map identifier.
    pub map_id: u32,
    /// Opaque request identifier.
    pub request_id: u64,
    /// Planned path length in millimetres.
    pub path_length_mm: u64,
    /// Number of waypoints.
    pub waypoint_count: u32,
    /// Number of obstacles considered during planning.
    pub obstacle_count: u32,
    /// Planning time in microseconds.
    pub planning_time_us: u32,
    /// Number of grid cells explored by the planner.
    pub cells_explored: u64,
    /// Whether the plan succeeded (false = no path found).
    pub success: bool,
}

/// Build a `NavigationAnalyticsRoutingMetrics` event.
#[inline]
#[must_use]
pub fn navigation_to_analytics_routing_metrics(
    map_id: u32,
    request_id: u64,
    path_length_mm: u64,
    waypoint_count: u32,
    obstacle_count: u32,
    planning_time_us: u32,
    cells_explored: u64,
    success: bool,
) -> NavigationAnalyticsRoutingMetrics {
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&map_id.to_le_bytes());
    buf[4..12].copy_from_slice(&request_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    NavigationAnalyticsRoutingMetrics {
        content_hash,
        map_id,
        request_id,
        path_length_mm,
        waypoint_count,
        obstacle_count,
        planning_time_us,
        cells_explored,
        success,
    }
}

// ── Bridge 4: Navigation → Physics (obstacle descriptor) ─────────────────

/// Static obstacle descriptor for ALICE-Physics.
///
/// Describes an axis-aligned bounding box obstacle so the physics engine
/// can register it as a static rigid body for collision detection.
pub struct NavigationPhysicsObstacle {
    /// FNV-1a hash of map_id + obstacle_id — physics body key.
    pub content_hash: u64,
    /// Map identifier.
    pub map_id: u32,
    /// Obstacle index within the map.
    pub obstacle_id: u32,
    /// AABB minimum X in millimetres.
    pub min_x_mm: i64,
    /// AABB minimum Y in millimetres.
    pub min_y_mm: i64,
    /// AABB maximum X in millimetres.
    pub max_x_mm: i64,
    /// AABB maximum Y in millimetres.
    pub max_y_mm: i64,
    /// Restitution coefficient in permille (0 = inelastic, 1000 = elastic).
    pub restitution_permille: u16,
    /// Friction coefficient in permille.
    pub friction_permille: u16,
    /// Whether the obstacle is passable (e.g. soft barrier with penalty).
    pub is_passable: bool,
}

/// Build a `NavigationPhysicsObstacle` for one AABB obstacle.
#[inline]
#[must_use]
pub fn navigation_to_physics_obstacle(
    map_id: u32,
    obstacle_id: u32,
    min_x_mm: i64,
    min_y_mm: i64,
    max_x_mm: i64,
    max_y_mm: i64,
    restitution_permille: u16,
    friction_permille: u16,
    is_passable: bool,
) -> NavigationPhysicsObstacle {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&map_id.to_le_bytes());
    buf[4..8].copy_from_slice(&obstacle_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    NavigationPhysicsObstacle {
        content_hash,
        map_id,
        obstacle_id,
        min_x_mm,
        min_y_mm,
        max_x_mm,
        max_y_mm,
        restitution_permille,
        friction_permille,
        is_passable,
    }
}

// ── Bridge 5: Navigation → Render (path visualisation) ───────────────────

/// Path visualisation geometry descriptor for ALICE-View/Render.
///
/// Packages a planned path as a polyline with colour and width hints
/// so the render layer can draw it as an overlay or debug view.
pub struct NavigationRenderPath {
    /// FNV-1a hash of map_id + request_id — render object key.
    pub content_hash: u64,
    /// Map identifier.
    pub map_id: u32,
    /// Planning request identifier.
    pub request_id: u64,
    /// Number of line segments in the polyline (waypoint_count - 1).
    pub segment_count: u32,
    /// Path length in millimetres (for scale-bar rendering).
    pub path_length_mm: u64,
    /// Packed RGBA colour for the path line (0xRRGGBBAA).
    pub line_color_rgba: u32,
    /// Line width in milli-pixels (actual px = value / 1000).
    pub line_width_milli_px: u32,
    /// Whether to render waypoint markers at each vertex.
    pub show_waypoints: bool,
    /// Whether to render a dashed line (true) or solid (false).
    pub dashed: bool,
}

/// Build a `NavigationRenderPath` geometry descriptor.
#[inline]
#[must_use]
pub fn navigation_to_render_path(
    map_id: u32,
    request_id: u64,
    waypoint_count: u32,
    path_length_mm: u64,
    line_color_rgba: u32,
    line_width_milli_px: u32,
    show_waypoints: bool,
    dashed: bool,
) -> NavigationRenderPath {
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&map_id.to_le_bytes());
    buf[4..12].copy_from_slice(&request_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    let segment_count = waypoint_count.saturating_sub(1);
    NavigationRenderPath {
        content_hash,
        map_id,
        request_id,
        segment_count,
        path_length_mm,
        line_color_rgba,
        line_width_milli_px,
        show_waypoints,
        dashed,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_db_map_record_hash_nonzero() {
        let rec = navigation_to_db_map_record(1, 1, 1024 * 1024, 100, 50, 200, 0xabc, 0);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_navigation_db_map_record_deterministic() {
        let a = navigation_to_db_map_record(3, 2, 512, 250, 10, 30, 0xdeadbeef, 999);
        let b = navigation_to_db_map_record(3, 2, 512, 250, 10, 30, 0xdeadbeef, 999);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_navigation_path_cache_static_ttl() {
        let entry = navigation_to_path_cache_entry(1, 0, 100, 50_000, 10, 2000, false);
        assert_eq!(entry.ttl_secs, 120);
    }

    #[test]
    fn test_navigation_path_cache_dynamic_ttl() {
        let entry = navigation_to_path_cache_entry(1, 0, 100, 50_000, 10, 2000, true);
        assert_eq!(entry.ttl_secs, 10);
    }

    #[test]
    fn test_navigation_analytics_routing_metrics_fields() {
        let m = navigation_to_analytics_routing_metrics(1, 42, 8_000, 15, 5, 3_500, 2048, true);
        assert_ne!(m.content_hash, 0);
        assert!(m.success);
        assert_eq!(m.waypoint_count, 15);
        assert_eq!(m.cells_explored, 2048);
    }

    #[test]
    fn test_navigation_physics_obstacle_fields() {
        let obs = navigation_to_physics_obstacle(1, 0, 0, 0, 1000, 500, 200, 800, false);
        assert_ne!(obs.content_hash, 0);
        assert_eq!(obs.max_x_mm, 1000);
        assert!(!obs.is_passable);
    }

    #[test]
    fn test_navigation_render_path_segment_count() {
        let p = navigation_to_render_path(1, 7, 10, 15_000, 0xff0000ff, 2000, true, false);
        assert_eq!(p.segment_count, 9);
        assert!(p.show_waypoints);
        assert!(!p.dashed);
    }

    #[test]
    fn test_navigation_render_path_single_waypoint() {
        // 1 waypoint → 0 segments, should not underflow.
        let p = navigation_to_render_path(1, 8, 1, 0, 0xffffffff, 1000, false, false);
        assert_eq!(p.segment_count, 0);
    }
}
