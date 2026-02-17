//! Print bridges — ALICE-Print ↔ DB, CDN, Cache, View, Analytics, Motion
//!
//! 6 bridges connecting 3D print slicer to the ALICE ecosystem.

use alice_print::SliceResult;

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Print → DB (slice result persistence) ─────────────────────

/// Slice result persistence record for ALICE-DB.
pub struct PrintDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Layer count.
    pub layer_count: usize,
    /// Filament usage in meters.
    pub filament_meters: f32,
    /// Estimated print time in seconds.
    pub print_time_seconds: f32,
    /// G-code size in bytes.
    pub gcode_bytes: usize,
}

/// Serialize slice result for ALICE-DB persistence.
pub fn print_to_db_record(result: &SliceResult) -> PrintDbRecord {
    let data = [
        result.layer_count.to_le_bytes().as_slice(),
        &result.gcode.len().to_le_bytes(),
    ]
    .concat();
    PrintDbRecord {
        content_hash: fnv1a(&data),
        layer_count: result.layer_count,
        filament_meters: result.filament_meters,
        print_time_seconds: result.print_time_seconds,
        gcode_bytes: result.gcode.len(),
    }
}

// ── Bridge 2: Print → CDN (G-code delivery) ─────────────────────────────

/// G-code delivery package for ALICE-CDN.
pub struct PrintCdnPackage {
    /// Content hash.
    pub content_hash: u64,
    /// G-code size in bytes.
    pub gcode_bytes: usize,
    /// Layer count.
    pub layer_count: usize,
    /// MIME type.
    pub content_type: &'static str,
}

/// Package G-code for ALICE-CDN delivery.
pub fn print_to_cdn_package(result: &SliceResult) -> PrintCdnPackage {
    let hash = fnv1a(result.gcode.as_bytes());
    PrintCdnPackage {
        content_hash: hash,
        gcode_bytes: result.gcode.len(),
        layer_count: result.layer_count,
        content_type: "application/x-gcode",
    }
}

// ── Bridge 3: Print → Cache (slice result caching) ──────────────────────

/// Slice result cache entry for ALICE-Cache.
pub struct PrintCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Layer count.
    pub layer_count: usize,
    /// G-code size in bytes.
    pub gcode_bytes: usize,
}

/// Cache slice result for ALICE-Cache.
pub fn print_to_cache_entry(result: &SliceResult) -> PrintCacheEntry {
    let hash = fnv1a(result.gcode.as_bytes());
    PrintCacheEntry {
        content_hash: hash,
        layer_count: result.layer_count,
        gcode_bytes: result.gcode.len(),
    }
}

// ── Bridge 4: Print → View (layer preview) ──────────────────────────────

/// Layer preview config for ALICE-View.
pub struct PrintViewConfig {
    /// Layer count.
    pub layer_count: usize,
    /// Filament usage in meters.
    pub filament_meters: f32,
    /// Print time in seconds.
    pub print_time_seconds: f32,
    /// Estimated viewport triangles per layer.
    pub triangles_per_layer: usize,
}

/// Configure layer preview for ALICE-View.
pub fn print_to_view_config(result: &SliceResult) -> PrintViewConfig {
    // Estimate triangles: ~100 per layer for visualization
    let tri_per_layer = 100;
    PrintViewConfig {
        layer_count: result.layer_count,
        filament_meters: result.filament_meters,
        print_time_seconds: result.print_time_seconds,
        triangles_per_layer: tri_per_layer,
    }
}

// ── Bridge 5: Print → Analytics (slice performance metrics) ──────────────

/// Slice performance metrics for ALICE-Analytics.
pub struct PrintAnalyticsMetrics {
    /// Compile time in ms.
    pub compile_ms: f64,
    /// Slice time in ms.
    pub slice_ms: f64,
    /// G-code generation time in ms.
    pub gcode_ms: f64,
    /// Total processing time in ms.
    pub total_ms: f64,
    /// Layer count.
    pub layer_count: usize,
}

/// Extract slice performance metrics for ALICE-Analytics.
pub fn print_to_analytics_metrics(result: &SliceResult) -> PrintAnalyticsMetrics {
    PrintAnalyticsMetrics {
        compile_ms: result.compile_ms,
        slice_ms: result.slice_ms,
        gcode_ms: result.gcode_ms,
        total_ms: result.compile_ms + result.slice_ms + result.gcode_ms,
        layer_count: result.layer_count,
    }
}

// ── Bridge 6: Print → Motion (toolpath velocity) ────────────────────────

/// Toolpath velocity config for ALICE-Motion.
pub struct PrintMotionConfig {
    /// Layer count.
    pub layer_count: usize,
    /// Estimated travel distance in meters.
    pub filament_meters: f32,
    /// Print time in seconds.
    pub print_time_seconds: f32,
    /// Average feed rate in mm/s (estimated).
    pub avg_feed_rate: f32,
}

/// Configure toolpath velocity for ALICE-Motion S-curve planning.
pub fn print_to_motion_config(result: &SliceResult) -> PrintMotionConfig {
    let avg_feed = if result.print_time_seconds > 0.0 {
        (result.filament_meters * 1000.0) / result.print_time_seconds
    } else {
        0.0
    };
    PrintMotionConfig {
        layer_count: result.layer_count,
        filament_meters: result.filament_meters,
        print_time_seconds: result.print_time_seconds,
        avg_feed_rate: avg_feed,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_result() -> SliceResult {
        use alice_sdf::SdfNode;
        use alice_print::{SlicerConfig, GcodeFlavor};
        let sdf = SdfNode::sphere(10.0);
        let config = SlicerConfig::default();
        alice_print::slice_sdf(&sdf, &config, GcodeFlavor::Marlin)
    }

    #[test]
    fn test_print_to_db_record() {
        let result = test_result();
        let rec = print_to_db_record(&result);
        assert_ne!(rec.content_hash, 0);
        assert!(rec.layer_count > 0);
    }

    #[test]
    fn test_print_to_cdn_package() {
        let result = test_result();
        let pkg = print_to_cdn_package(&result);
        assert_ne!(pkg.content_hash, 0);
        assert_eq!(pkg.content_type, "application/x-gcode");
    }

    #[test]
    fn test_print_to_cache_entry() {
        let result = test_result();
        let entry = print_to_cache_entry(&result);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_print_to_view_config() {
        let result = test_result();
        let cfg = print_to_view_config(&result);
        assert!(cfg.layer_count > 0);
        assert_eq!(cfg.triangles_per_layer, 100);
    }

    #[test]
    fn test_print_to_analytics_metrics() {
        let result = test_result();
        let m = print_to_analytics_metrics(&result);
        assert!(m.total_ms >= 0.0);
        assert!(m.layer_count > 0);
    }

    #[test]
    fn test_print_to_motion_config() {
        let result = test_result();
        let cfg = print_to_motion_config(&result);
        assert!(cfg.layer_count > 0);
    }
}
