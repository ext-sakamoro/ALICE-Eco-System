//! SLAM bridges — ALICE-SLAM ↔ DB, Cache, Analytics, Physics, Render
//!
//! 5 bridges connecting simultaneous localization and mapping to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// Bridge 1: SLAM → DB (map persistence)
pub struct SlamDbMap {
    pub content_hash: u64,
    pub point_count: usize,
    pub keyframe_count: usize,
    pub map_version: u32,
}

#[inline]
#[must_use]
pub fn slam_to_db(point_count: usize, keyframe_count: usize, map_version: u32) -> SlamDbMap {
    SlamDbMap {
        content_hash: fnv1a(b"slam_db")
            ^ (point_count as u64)
            ^ (keyframe_count as u64).wrapping_mul(0x9e37),
        point_count,
        keyframe_count,
        map_version,
    }
}

// Bridge 2: SLAM → Cache (pose cache)
pub struct SlamCachePose {
    pub content_hash: u64,
    pub ttl_secs: u32,
    pub pose_x: i32,
    pub pose_y: i32,
}

#[inline]
#[must_use]
pub fn slam_to_cache(pose_x: i32, pose_y: i32, ttl_secs: u32) -> SlamCachePose {
    SlamCachePose {
        content_hash: fnv1a(b"slam_cache") ^ ((pose_x as u64).wrapping_add(pose_y as u64)),
        ttl_secs,
        pose_x,
        pose_y,
    }
}

// Bridge 3: SLAM → Analytics (mapping metrics)
pub struct SlamAnalyticsMetric {
    pub content_hash: u64,
    pub point_count: usize,
    pub loop_closure_count: usize,
    pub drift_mm: u32,
}

#[inline]
#[must_use]
pub fn slam_to_analytics(
    point_count: usize,
    loop_closure_count: usize,
    drift_mm: u32,
) -> SlamAnalyticsMetric {
    SlamAnalyticsMetric {
        content_hash: fnv1a(b"slam_analytics") ^ (point_count as u64) ^ u64::from(drift_mm),
        point_count,
        loop_closure_count,
        drift_mm,
    }
}

// Bridge 4: SLAM → Physics (obstacle data)
pub struct SlamPhysicsObstacle {
    pub content_hash: u64,
    pub grid_cells: usize,
    pub cell_size_mm: u32,
    pub occupied_count: usize,
}

#[inline]
#[must_use]
pub fn slam_to_physics(
    grid_cells: usize,
    cell_size_mm: u32,
    occupied_count: usize,
) -> SlamPhysicsObstacle {
    SlamPhysicsObstacle {
        content_hash: fnv1a(b"slam_physics")
            ^ (grid_cells as u64)
            ^ (occupied_count as u64).wrapping_mul(0x1f),
        grid_cells,
        cell_size_mm,
        occupied_count,
    }
}

// Bridge 5: SLAM → Render (visualization)
pub struct SlamRenderScene {
    pub content_hash: u64,
    pub point_count: usize,
    pub trajectory_nodes: usize,
    pub show_uncertainty: bool,
}

#[inline]
#[must_use]
pub fn slam_to_render(
    point_count: usize,
    trajectory_nodes: usize,
    show_uncertainty: bool,
) -> SlamRenderScene {
    SlamRenderScene {
        content_hash: fnv1a(b"slam_render")
            ^ (point_count as u64)
            ^ (trajectory_nodes as u64).wrapping_mul(0x17),
        point_count,
        trajectory_nodes,
        show_uncertainty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slam_db_hash_nonzero() {
        let m = slam_to_db(50_000, 120, 1);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.point_count, 50_000);
        assert_eq!(m.map_version, 1);
    }

    #[test]
    fn test_slam_db_keyframes() {
        let m = slam_to_db(1000, 30, 2);
        assert_eq!(m.keyframe_count, 30);
    }

    #[test]
    fn test_slam_cache_pose() {
        let p = slam_to_cache(100, -50, 5);
        assert_eq!(p.pose_x, 100);
        assert_eq!(p.pose_y, -50);
        assert_eq!(p.ttl_secs, 5);
        assert_ne!(p.content_hash, 0);
    }

    #[test]
    fn test_slam_analytics_drift() {
        let m = slam_to_analytics(8000, 3, 12);
        assert_eq!(m.drift_mm, 12);
        assert_eq!(m.loop_closure_count, 3);
        assert_ne!(m.content_hash, 0);
    }

    #[test]
    fn test_slam_physics_occupancy() {
        let o = slam_to_physics(10000, 50, 3200);
        assert_eq!(o.grid_cells, 10000);
        assert_eq!(o.occupied_count, 3200);
        assert_ne!(o.content_hash, 0);
    }

    #[test]
    fn test_slam_render_scene() {
        let s = slam_to_render(20000, 85, true);
        assert!(s.show_uncertainty);
        assert_eq!(s.trajectory_nodes, 85);
        assert_ne!(s.content_hash, 0);
    }

    #[test]
    fn test_slam_hash_determinism() {
        let m1 = slam_to_db(1000, 10, 1);
        let m2 = slam_to_db(1000, 10, 1);
        assert_eq!(m1.content_hash, m2.content_hash);
    }

    #[test]
    fn test_slam_physics_cell_size() {
        let o = slam_to_physics(400, 100, 80);
        assert_eq!(o.cell_size_mm, 100);
    }
}
