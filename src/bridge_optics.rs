//! Optics bridges — ALICE-Optics ↔ DB, Cache, Analytics, Render, Physics
//!
//! 5 bridges connecting optical simulation to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Optics → DB (lens design) ──────────────────────────────────

/// Lens design record for ALICE-DB persistence.
pub struct OpticsDbRecord {
    /// Content hash over the lens parameters.
    pub content_hash: u64,
    /// Focal length in millimetres.
    pub focal_length_mm: f64,
    /// Primary wavelength in nanometres.
    pub wavelength_nm: f64,
    /// Index of refraction (dimensionless).
    pub refraction_index: f64,
    /// Aperture (f-number, dimensionless).
    pub aperture_f: f64,
    /// Optical resolution in line pairs per millimetre.
    pub resolution_lpmm: f32,
    /// Number of lens elements.
    pub element_count: u16,
}

/// Serialize lens design for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn optics_to_db_record(
    focal_length_mm: f64,
    wavelength_nm: f64,
    refraction_index: f64,
    aperture_f: f64,
    resolution_lpmm: f32,
    element_count: u16,
) -> OpticsDbRecord {
    let mut key = [0u8; 34];
    key[0..8].copy_from_slice(&focal_length_mm.to_le_bytes());
    key[8..16].copy_from_slice(&wavelength_nm.to_le_bytes());
    key[16..24].copy_from_slice(&refraction_index.to_le_bytes());
    key[24..32].copy_from_slice(&aperture_f.to_le_bytes());
    key[32..34].copy_from_slice(&element_count.to_le_bytes());
    OpticsDbRecord {
        content_hash: fnv1a(&key),
        focal_length_mm,
        wavelength_nm,
        refraction_index,
        aperture_f,
        resolution_lpmm,
        element_count,
    }
}

// ── Bridge 2: Optics → Cache (ray cache) ─────────────────────────────────

/// Ray intersection cache entry for ALICE-Cache.
pub struct OpticsCacheEntry {
    /// Content hash for cache key derivation.
    pub content_hash: u64,
    /// Number of rays traced in the cached batch.
    pub ray_count: u64,
    /// Hit ratio of rays that intersected a surface (0.0–1.0).
    pub hit_ratio: f32,
    /// Mean path length of traced rays in scene units.
    pub mean_path_length: f32,
    /// TTL in seconds (branchless: shorter when hit ratio is low).
    pub ttl_secs: u32,
}

/// Cache ray intersection batch for ALICE-Cache.
#[inline]
#[must_use]
pub fn optics_to_cache_entry(
    ray_count: u64,
    hit_count: u64,
    total_path_length: f32,
) -> OpticsCacheEntry {
    let rcp_rays = 1.0 / ray_count.max(1) as f32;
    let hit_ratio = hit_count as f32 * rcp_rays;
    let mean_path_length = total_path_length * rcp_rays;

    // Branchless TTL: 120 s normally, 30 s when hit_ratio < 0.5 (low-utility cache).
    let low_hit = (hit_ratio < 0.5) as u32;
    let ttl_secs = 120_u32 - low_hit * 90;

    let data = ray_count.to_le_bytes();
    OpticsCacheEntry {
        content_hash: fnv1a(&data),
        ray_count,
        hit_ratio,
        mean_path_length,
        ttl_secs,
    }
}

// ── Bridge 3: Optics → Analytics (optical metrics) ────────────────────────

/// Optical system metrics for ALICE-Analytics ingestion.
pub struct OpticsAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total rays traced in the reporting period.
    pub total_rays: u64,
    /// Average rays per second (throughput).
    pub rays_per_sec: f64,
    /// Mean modulation transfer function value (0.0–1.0).
    pub mean_mtf: f32,
    /// Peak aberration in waves (RMS wavefront error).
    pub peak_aberration_waves: f32,
    /// Number of distinct wavelengths simulated.
    pub wavelength_count: u16,
    /// Simulation wall-clock time in milliseconds.
    pub elapsed_ms: u64,
}

/// Build optical system metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn optics_to_analytics_metrics(
    total_rays: u64,
    elapsed_ms: u64,
    mean_mtf: f32,
    peak_aberration_waves: f32,
    wavelength_count: u16,
) -> OpticsAnalyticsMetrics {
    let elapsed_sec = elapsed_ms as f64 * 1e-3;
    let rcp_elapsed = 1.0 / elapsed_sec.max(1e-9);
    let rays_per_sec = total_rays as f64 * rcp_elapsed;

    let mut key = [0u8; 18];
    key[0..8].copy_from_slice(&total_rays.to_le_bytes());
    key[8..16].copy_from_slice(&elapsed_ms.to_le_bytes());
    key[16..18].copy_from_slice(&wavelength_count.to_le_bytes());
    OpticsAnalyticsMetrics {
        content_hash: fnv1a(&key),
        total_rays,
        rays_per_sec,
        mean_mtf,
        peak_aberration_waves,
        wavelength_count,
        elapsed_ms,
    }
}

// ── Bridge 4: Optics → Render (ray tracing) ───────────────────────────────

/// Ray tracing configuration for ALICE-Render integration.
pub struct OpticsRenderConfig {
    /// Content hash over the render configuration.
    pub content_hash: u64,
    /// Focal length in millimetres.
    pub focal_length_mm: f64,
    /// Aperture (f-number).
    pub aperture_f: f64,
    /// Index of refraction of the primary medium.
    pub refraction_index: f64,
    /// Maximum ray bounce depth.
    pub max_bounces: u16,
    /// Samples per pixel for Monte Carlo integration.
    pub samples_per_pixel: u32,
    /// Wavelength in nanometres for chromatic dispersion simulation.
    pub wavelength_nm: f64,
}

/// Build ray tracing configuration for ALICE-Render.
#[inline]
#[must_use]
pub fn optics_to_render_config(
    focal_length_mm: f64,
    aperture_f: f64,
    refraction_index: f64,
    max_bounces: u16,
    samples_per_pixel: u32,
    wavelength_nm: f64,
) -> OpticsRenderConfig {
    let mut key = [0u8; 30];
    key[0..8].copy_from_slice(&focal_length_mm.to_le_bytes());
    key[8..16].copy_from_slice(&aperture_f.to_le_bytes());
    key[16..18].copy_from_slice(&max_bounces.to_le_bytes());
    key[18..22].copy_from_slice(&samples_per_pixel.to_le_bytes());
    key[22..30].copy_from_slice(&wavelength_nm.to_le_bytes());
    OpticsRenderConfig {
        content_hash: fnv1a(&key),
        focal_length_mm,
        aperture_f,
        refraction_index,
        max_bounces,
        samples_per_pixel,
        wavelength_nm,
    }
}

// ── Bridge 5: Optics → Physics (refraction) ───────────────────────────────

/// Refraction parameters for ALICE-Physics integration.
pub struct OpticsPhysicsRefraction {
    /// Content hash over the refraction parameters.
    pub content_hash: u64,
    /// Index of refraction of medium 1 (incident side).
    pub n1: f64,
    /// Index of refraction of medium 2 (transmitted side).
    pub n2: f64,
    /// Critical angle for total internal reflection in degrees.
    pub critical_angle_deg: f64,
    /// Fresnel reflectance at normal incidence (0.0–1.0).
    pub fresnel_r0: f64,
    /// Wavelength in nanometres used for dispersion calculation.
    pub wavelength_nm: f64,
    /// Abbe number (reciprocal dispersive power, dimensionless).
    pub abbe_number: f32,
}

/// Build refraction parameters for ALICE-Physics.
///
/// `critical_angle_deg` is computed from Snell's law: arcsin(n1/n2) in degrees.
/// Returns `None` if `n2 <= n1` (no total internal reflection regime).
#[inline]
#[must_use]
pub fn optics_to_physics_refraction(
    n1: f64,
    n2: f64,
    wavelength_nm: f64,
    abbe_number: f32,
) -> Option<OpticsPhysicsRefraction> {
    if n2 <= 0.0 || n1 <= 0.0 {
        return None;
    }
    // Fresnel reflectance at normal incidence.
    let n_diff = n2 - n1;
    let n_sum = n2 + n1;
    let rcp_sum = 1.0 / n_sum;
    let fresnel_r0 = (n_diff * rcp_sum) * (n_diff * rcp_sum);

    // Critical angle: only defined when n1 > n2.
    let ratio = n1 / n2;
    let critical_angle_deg = if ratio <= 1.0 {
        ratio.asin() * (180.0 / core::f64::consts::PI)
    } else {
        90.0 // no TIR when n1 <= n2; use 90° as sentinel
    };

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&n1.to_le_bytes());
    key[8..16].copy_from_slice(&n2.to_le_bytes());
    key[16..24].copy_from_slice(&wavelength_nm.to_le_bytes());
    Some(OpticsPhysicsRefraction {
        content_hash: fnv1a(&key),
        n1,
        n2,
        critical_angle_deg,
        fresnel_r0,
        wavelength_nm,
        abbe_number,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optics_to_db_record_hash_nonzero() {
        let rec = optics_to_db_record(50.0, 550.0, 1.5, 2.8, 100.0, 6);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.element_count, 6);
    }

    #[test]
    fn test_optics_to_db_record_deterministic() {
        let a = optics_to_db_record(35.0, 632.8, 1.52, 1.4, 80.0, 4);
        let b = optics_to_db_record(35.0, 632.8, 1.52, 1.4, 80.0, 4);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_optics_to_cache_entry_high_hit_ttl() {
        let entry = optics_to_cache_entry(1000, 900, 450.0);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 120); // hit_ratio 0.9 >= 0.5
        assert!((entry.hit_ratio - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_optics_to_cache_entry_low_hit_ttl() {
        let entry = optics_to_cache_entry(1000, 400, 200.0);
        assert_eq!(entry.ttl_secs, 30); // hit_ratio 0.4 < 0.5
    }

    #[test]
    fn test_optics_to_analytics_metrics_throughput() {
        // 1 million rays in 1000 ms → 1_000_000 rays/s.
        let m = optics_to_analytics_metrics(1_000_000, 1_000, 0.8, 0.05, 3);
        assert_ne!(m.content_hash, 0);
        assert!((m.rays_per_sec - 1_000_000.0).abs() < 1.0);
        assert_eq!(m.wavelength_count, 3);
    }

    #[test]
    fn test_optics_to_render_config_fields() {
        let cfg = optics_to_render_config(85.0, 1.8, 1.0, 8, 256, 550.0);
        assert_ne!(cfg.content_hash, 0);
        assert_eq!(cfg.max_bounces, 8);
        assert_eq!(cfg.samples_per_pixel, 256);
        assert!((cfg.focal_length_mm - 85.0).abs() < 0.001);
    }

    #[test]
    fn test_optics_to_physics_refraction_valid() {
        // Glass (n2=1.5) in air (n1=1.0).
        let r = optics_to_physics_refraction(1.0, 1.5, 550.0, 64.2).unwrap();
        assert_ne!(r.content_hash, 0);
        // Fresnel R0 = ((1.5-1.0)/(1.5+1.0))^2 = (0.5/2.5)^2 = 0.04
        assert!((r.fresnel_r0 - 0.04).abs() < 1e-9);
    }

    #[test]
    fn test_optics_to_physics_refraction_none_on_zero_n() {
        assert!(optics_to_physics_refraction(0.0, 1.5, 550.0, 64.0).is_none());
        assert!(optics_to_physics_refraction(1.0, 0.0, 550.0, 64.0).is_none());
    }
}
