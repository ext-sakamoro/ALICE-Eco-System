//! Print bridges — ALICE-Print ↔ DB, CDN, Cache, View, Analytics, Motion, Physics
//!
//! 8 bridges connecting 3D print slicer to the ALICE ecosystem.

use alice_print::SliceResult;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
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
#[inline]
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
#[inline]
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
#[inline]
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
#[inline]
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
#[inline]
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
#[inline]
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

// ── Bridge 7: Physics → Print (FEA stress analysis before printing) ──

/// FEA stress analysis result for pre-print structural validation.
///
/// Finite Element Analysis estimates whether the sliced geometry can
/// withstand gravity and handling forces without structural failure.
pub struct PrintFeaResult {
    /// Content hash linking to the slice result.
    pub content_hash: u64,
    /// Maximum von Mises stress in MPa.
    pub max_stress_mpa: f32,
    /// Estimated safety factor (yield_strength / max_stress).
    pub safety_factor: f32,
    /// Number of layers analysed.
    pub layer_count: usize,
    /// Material density used (kg/m³).
    pub material_density: f32,
    /// True if the part passes structural validation (safety_factor >= 1.5).
    pub passes_validation: bool,
}

/// Run simplified FEA stress estimate on a print slice.
///
/// Uses layer geometry and material properties to estimate whether the
/// printed part can support its own weight. `yield_strength_mpa` is the
/// filament material's yield strength (e.g. PLA ≈ 60 MPa, PETG ≈ 50).
#[inline]
pub fn physics_to_print_fea(
    result: &SliceResult,
    material_density: f32,
    yield_strength_mpa: f32,
) -> PrintFeaResult {
    let data = [
        result.layer_count.to_le_bytes().as_slice(),
        &result.gcode.len().to_le_bytes(),
    ]
    .concat();
    let content_hash = fnv1a(&data);

    // Simplified cantilever beam model: σ = ρ·g·L² / (2·t)
    // where L = total height (layers × layer_height), t = min wall thickness
    let layer_height_mm = 0.2_f32; // typical
    let total_height_m = result.layer_count as f32 * layer_height_mm * 0.001;
    let wall_thickness_m = 0.0012_f32; // 3 perimeters × 0.4mm nozzle
    let gravity = 9.81_f32;

    // Stress in Pa, then convert to MPa
    let rcp_wall = 1.0 / wall_thickness_m.max(1e-6);
    let stress_pa = material_density * gravity * total_height_m * total_height_m * 0.5 * rcp_wall;
    let max_stress_mpa = stress_pa * 1e-6;

    let rcp_stress = 1.0 / max_stress_mpa.max(1e-9);
    let safety_factor = yield_strength_mpa * rcp_stress;

    PrintFeaResult {
        content_hash,
        max_stress_mpa,
        safety_factor,
        layer_count: result.layer_count,
        material_density,
        passes_validation: safety_factor >= 1.5,
    }
}

// ── Bridge 8: Print → Physics (structural validation request) ────────

/// Structural parameters for ALICE-Physics rigid body simulation.
///
/// Provides mass, inertia estimate, and center-of-mass for the printed
/// part so it can be simulated as a rigid body in the physics engine.
pub struct PrintPhysicsBody {
    /// Content hash linking to the slice result.
    pub content_hash: u64,
    /// Estimated mass in kg.
    pub mass_kg: f32,
    /// Approximate bounding box half-extents [x, y, z] in metres.
    pub half_extents: [f32; 3],
    /// Estimated moment of inertia (diagonal, uniform density).
    pub inertia_diag: [f32; 3],
    /// Layer count (for LOD selection in physics sim).
    pub layer_count: usize,
}

/// Convert a print slice result into ALICE-Physics rigid body parameters.
///
/// Assumes a solid rectangular prism approximation from the layer stack.
/// Uses `material_density` (kg/m³) to estimate mass.
#[inline]
pub fn print_to_physics_body(
    result: &SliceResult,
    material_density: f32,
    bed_size_mm: (f32, f32),
) -> PrintPhysicsBody {
    let data = [
        result.layer_count.to_le_bytes().as_slice(),
        &result.gcode.len().to_le_bytes(),
    ]
    .concat();
    let content_hash = fnv1a(&data);

    let layer_height_m = 0.0002_f32; // 0.2mm
    let height_m = result.layer_count as f32 * layer_height_m;
    let width_m = bed_size_mm.0 * 0.001;
    let depth_m = bed_size_mm.1 * 0.001;

    // Volume estimate: filament_meters × π × (1.75mm/2)² cross section
    let filament_radius_m = 0.000875; // 1.75mm / 2
    let volume_m3 =
        result.filament_meters * std::f32::consts::PI * filament_radius_m * filament_radius_m;
    let mass_kg = volume_m3 * material_density;

    let hx = width_m * 0.5;
    let hy = height_m * 0.5;
    let hz = depth_m * 0.5;

    // Solid box inertia: I = m/12 * (b² + c²) per axis
    let rcp_12 = 1.0_f32 / 12.0;
    let ix = mass_kg * rcp_12 * (hy * hy + hz * hz);
    let iy = mass_kg * rcp_12 * (hx * hx + hz * hz);
    let iz = mass_kg * rcp_12 * (hx * hx + hy * hy);

    PrintPhysicsBody {
        content_hash,
        mass_kg,
        half_extents: [hx, hy, hz],
        inertia_diag: [ix, iy, iz],
        layer_count: result.layer_count,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_result() -> SliceResult {
        use alice_print::{GcodeFlavor, SlicerConfig};
        use alice_sdf::SdfNode;
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

    #[test]
    fn test_physics_to_print_fea() {
        let result = test_result();
        let fea = physics_to_print_fea(&result, 1250.0, 60.0);
        assert_ne!(fea.content_hash, 0);
        assert!(fea.max_stress_mpa >= 0.0);
        assert!(fea.safety_factor > 0.0);
        assert!(fea.layer_count > 0);
        assert!((fea.material_density - 1250.0).abs() < 0.01);
    }

    #[test]
    fn test_print_to_physics_body() {
        let result = test_result();
        let body = print_to_physics_body(&result, 1250.0, (220.0, 220.0));
        assert_ne!(body.content_hash, 0);
        assert!(body.mass_kg >= 0.0);
        assert!(body.half_extents[0] > 0.0);
        assert!(body.half_extents[1] > 0.0);
        assert!(body.half_extents[2] > 0.0);
        assert!(body.layer_count > 0);
    }
}
