//! SDF Material bridges — ALICE-SDF Material ↔ View, CDN, Cache, Analytics, Edge
//!
//! 5 bridges connecting PBR material definitions from ALICE-SDF to the
//! ALICE ecosystem. Covers material visualization, CDN delivery, caching,
//! analytics tracking, and edge snapshot for real-time rendering pipelines.

use alice_sdf::material::Material;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Material → View (PBR visualization descriptor) ──────────

/// PBR visualization descriptor for ALICE-View.
///
/// Provides the view layer with the essential PBR parameters needed to
/// render a material in the viewport. Texture paths are omitted; only
/// flat parameter values are forwarded for shader uniform upload.
pub struct SdfMaterialViewDescriptor {
    /// FNV-1a hash of the material content.
    pub content_hash: u64,
    /// Base color RGBA (linear space).
    pub base_color: [f32; 4],
    /// Metallic factor (0.0 = dielectric, 1.0 = metal).
    pub metallic: f32,
    /// Roughness factor (0.0 = mirror, 1.0 = diffuse).
    pub roughness: f32,
    /// Emissive color RGB (linear space).
    pub emission: [f32; 3],
    /// Emissive intensity multiplier.
    pub emission_strength: f32,
    /// Opacity (0.0 = transparent, 1.0 = opaque).
    pub opacity: f32,
    /// True when any texture map is present (requires texture binding).
    pub has_textures: bool,
    /// Number of texture slots bound (0..7).
    pub texture_slot_count: u8,
}

/// Build a PBR visualization descriptor from a `Material` for ALICE-View.
#[inline]
#[must_use]
pub fn sdf_material_to_view_descriptor(mat: &Material) -> SdfMaterialViewDescriptor {
    // Hash: base_color (16 bytes) + metallic (4) + roughness (4) = 24 bytes
    let mut data = [0u8; 24];
    data[0..4].copy_from_slice(&mat.base_color[0].to_le_bytes());
    data[4..8].copy_from_slice(&mat.base_color[1].to_le_bytes());
    data[8..12].copy_from_slice(&mat.base_color[2].to_le_bytes());
    data[12..16].copy_from_slice(&mat.base_color[3].to_le_bytes());
    data[16..20].copy_from_slice(&mat.metallic.to_le_bytes());
    data[20..24].copy_from_slice(&mat.roughness.to_le_bytes());
    let content_hash = fnv1a(&data);

    let mut texture_slot_count: u8 = 0;
    if mat.albedo_map.is_some() {
        texture_slot_count += 1;
    }
    if mat.normal_map.is_some() {
        texture_slot_count += 1;
    }
    if mat.metallic_map.is_some() {
        texture_slot_count += 1;
    }
    if mat.roughness_map.is_some() {
        texture_slot_count += 1;
    }
    if mat.ao_map.is_some() {
        texture_slot_count += 1;
    }
    if mat.emissive_map.is_some() {
        texture_slot_count += 1;
    }
    if mat.metallic_roughness_map.is_some() {
        texture_slot_count += 1;
    }

    SdfMaterialViewDescriptor {
        content_hash,
        base_color: mat.base_color,
        metallic: mat.metallic,
        roughness: mat.roughness,
        emission: mat.emission,
        emission_strength: mat.emission_strength,
        opacity: mat.opacity,
        has_textures: texture_slot_count > 0,
        texture_slot_count,
    }
}

// ── Bridge 2: Material → CDN (content delivery entry) ─────────────────

/// CDN content entry for material asset delivery.
///
/// Provides ALICE-CDN with the metadata needed to cache and serve
/// PBR materials, including an estimated payload size based on
/// texture slot count and material parameter complexity.
pub struct SdfMaterialCdnEntry {
    /// FNV-1a hash of the material content — CDN cache key.
    pub content_hash: u64,
    /// Material name (for CDN path routing).
    pub name_len: u32,
    /// Estimated payload size in bytes.
    pub estimated_bytes: usize,
    /// Content type identifier (0x20 = PBR material type).
    pub content_type_id: u8,
    /// Number of texture slots (affects download priority).
    pub texture_count: u8,
    /// True when the material uses transmission (glass/water).
    pub is_transmissive: bool,
}

/// Build a CDN content entry from a `Material` for ALICE-CDN.
///
/// `estimated_bytes`: 256 base (material params) + 4096 per texture slot.
#[inline]
#[must_use]
pub fn sdf_material_to_cdn_entry(mat: &Material) -> SdfMaterialCdnEntry {
    let mut data = [0u8; 24];
    data[0..4].copy_from_slice(&mat.base_color[0].to_le_bytes());
    data[4..8].copy_from_slice(&mat.base_color[1].to_le_bytes());
    data[8..12].copy_from_slice(&mat.base_color[2].to_le_bytes());
    data[12..16].copy_from_slice(&mat.base_color[3].to_le_bytes());
    data[16..20].copy_from_slice(&mat.metallic.to_le_bytes());
    data[20..24].copy_from_slice(&mat.roughness.to_le_bytes());
    let content_hash = fnv1a(&data);

    let mut texture_count: u8 = 0;
    if mat.albedo_map.is_some() {
        texture_count += 1;
    }
    if mat.normal_map.is_some() {
        texture_count += 1;
    }
    if mat.metallic_map.is_some() {
        texture_count += 1;
    }
    if mat.roughness_map.is_some() {
        texture_count += 1;
    }
    if mat.ao_map.is_some() {
        texture_count += 1;
    }
    if mat.emissive_map.is_some() {
        texture_count += 1;
    }
    if mat.metallic_roughness_map.is_some() {
        texture_count += 1;
    }

    let estimated_bytes = 256 + texture_count as usize * 4096;

    SdfMaterialCdnEntry {
        content_hash,
        name_len: mat.name.len() as u32,
        estimated_bytes,
        content_type_id: 0x20,
        texture_count,
        is_transmissive: mat.transmission > 0.0,
    }
}

// ── Bridge 3: Material → Cache (entry with branchless TTL) ────────────

/// Cache entry for ALICE-Cache material lookup.
///
/// Materials with active emission or transmission are cached with
/// a shorter TTL because they are more likely to change during
/// real-time editing sessions.
pub struct SdfMaterialCacheEntry {
    /// FNV-1a hash of the material content — cache key.
    pub content_hash: u64,
    /// Metallic factor (cached for shader fast-path selection).
    pub metallic: f32,
    /// Roughness factor (cached for mip-level selection).
    pub roughness: f32,
    /// Opacity (cached for transparency sort).
    pub opacity: f32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// True when the material is fully opaque (no alpha blending needed).
    pub is_opaque: bool,
}

/// Build a cache entry from a `Material` for ALICE-Cache.
///
/// Branchless TTL: emissive or transmissive materials get short TTL (30s),
/// standard materials get long TTL (300s).
#[inline]
#[must_use]
pub fn sdf_material_to_cache_entry(mat: &Material) -> SdfMaterialCacheEntry {
    let mut data = [0u8; 24];
    data[0..4].copy_from_slice(&mat.base_color[0].to_le_bytes());
    data[4..8].copy_from_slice(&mat.base_color[1].to_le_bytes());
    data[8..12].copy_from_slice(&mat.base_color[2].to_le_bytes());
    data[12..16].copy_from_slice(&mat.base_color[3].to_le_bytes());
    data[16..20].copy_from_slice(&mat.metallic.to_le_bytes());
    data[20..24].copy_from_slice(&mat.roughness.to_le_bytes());
    let content_hash = fnv1a(&data);

    // Branchless TTL: active emission or transmission → short TTL
    let is_dynamic = (mat.emission_strength > 0.0 || mat.transmission > 0.0) as u32;
    let ttl_secs = 300 - is_dynamic * 270; // 300s normal, 30s dynamic

    SdfMaterialCacheEntry {
        content_hash,
        metallic: mat.metallic,
        roughness: mat.roughness,
        opacity: mat.opacity,
        ttl_secs,
        is_opaque: mat.opacity >= 1.0 && mat.transmission == 0.0,
    }
}

// ── Bridge 4: Material → Analytics (material usage event) ─────────────

/// Analytics event for material usage tracking.
///
/// Tracks which PBR features are in use across the project to inform
/// shader permutation pruning and rendering pipeline optimization.
pub struct SdfMaterialAnalyticsEvent {
    /// FNV-1a hash of the material content.
    pub content_hash: u64,
    /// True when material is metallic (metallic >= 0.5).
    pub is_metal: bool,
    /// True when material has clearcoat.
    pub has_clearcoat: bool,
    /// True when material has subsurface scattering.
    pub has_subsurface: bool,
    /// True when material has sheen (fabric/velvet).
    pub has_sheen: bool,
    /// True when material has anisotropy.
    pub has_anisotropy: bool,
    /// True when material is transmissive.
    pub has_transmission: bool,
    /// Texture complexity score (0..7 texture slots used).
    pub texture_complexity: u8,
    /// PBR complexity score (count of active advanced PBR features, 0..6).
    pub pbr_complexity: u8,
}

/// Build an analytics event from a `Material` for ALICE-Analytics.
#[inline]
#[must_use]
pub fn sdf_material_to_analytics_event(mat: &Material) -> SdfMaterialAnalyticsEvent {
    let mut data = [0u8; 24];
    data[0..4].copy_from_slice(&mat.base_color[0].to_le_bytes());
    data[4..8].copy_from_slice(&mat.base_color[1].to_le_bytes());
    data[8..12].copy_from_slice(&mat.base_color[2].to_le_bytes());
    data[12..16].copy_from_slice(&mat.base_color[3].to_le_bytes());
    data[16..20].copy_from_slice(&mat.metallic.to_le_bytes());
    data[20..24].copy_from_slice(&mat.roughness.to_le_bytes());
    let content_hash = fnv1a(&data);

    let has_clearcoat = mat.clearcoat > 0.0;
    let has_subsurface = mat.subsurface > 0.0;
    let has_sheen =
        mat.sheen_color[0] > 0.0 || mat.sheen_color[1] > 0.0 || mat.sheen_color[2] > 0.0;
    let has_anisotropy = mat.anisotropy.abs() > 0.0;
    let has_transmission = mat.transmission > 0.0;
    let is_metal = mat.metallic >= 0.5;

    let mut texture_complexity: u8 = 0;
    if mat.albedo_map.is_some() {
        texture_complexity += 1;
    }
    if mat.normal_map.is_some() {
        texture_complexity += 1;
    }
    if mat.metallic_map.is_some() {
        texture_complexity += 1;
    }
    if mat.roughness_map.is_some() {
        texture_complexity += 1;
    }
    if mat.ao_map.is_some() {
        texture_complexity += 1;
    }
    if mat.emissive_map.is_some() {
        texture_complexity += 1;
    }
    if mat.metallic_roughness_map.is_some() {
        texture_complexity += 1;
    }

    let pbr_complexity = has_clearcoat as u8
        + has_subsurface as u8
        + has_sheen as u8
        + has_anisotropy as u8
        + has_transmission as u8
        + (mat.emission_strength > 0.0) as u8;

    SdfMaterialAnalyticsEvent {
        content_hash,
        is_metal,
        has_clearcoat,
        has_subsurface,
        has_sheen,
        has_anisotropy,
        has_transmission,
        texture_complexity,
        pbr_complexity,
    }
}

// ── Bridge 5: Material PBR → Edge (rendering snapshot) ────────────────

/// Edge rendering snapshot for ALICE-Edge real-time pipelines.
///
/// A compact, fixed-size representation of the most performance-critical
/// PBR parameters suitable for GPU uniform buffer upload on edge devices.
/// Extended PBR features (clearcoat, sheen, SSS) are collapsed into
/// a single `extended_features` bitmask.
pub struct SdfMaterialEdgeSnapshot {
    /// FNV-1a hash of the material content — edge cache key.
    pub content_hash: u64,
    /// Base color RGBA packed as [f32; 4].
    pub base_color: [f32; 4],
    /// Metallic factor.
    pub metallic: f32,
    /// Roughness factor.
    pub roughness: f32,
    /// Emission strength (0.0 = no emission).
    pub emission_strength: f32,
    /// Bitmask of extended PBR features:
    ///   bit 0 = clearcoat, bit 1 = sheen, bit 2 = transmission,
    ///   bit 3 = anisotropy, bit 4 = subsurface, bit 5 = emissive.
    pub extended_features: u8,
    /// Index of refraction (for transmission materials).
    pub ior: f32,
}

/// Build an edge rendering snapshot from a `Material` for ALICE-Edge.
#[inline]
#[must_use]
pub fn sdf_material_to_edge_snapshot(mat: &Material) -> SdfMaterialEdgeSnapshot {
    let mut data = [0u8; 24];
    data[0..4].copy_from_slice(&mat.base_color[0].to_le_bytes());
    data[4..8].copy_from_slice(&mat.base_color[1].to_le_bytes());
    data[8..12].copy_from_slice(&mat.base_color[2].to_le_bytes());
    data[12..16].copy_from_slice(&mat.base_color[3].to_le_bytes());
    data[16..20].copy_from_slice(&mat.metallic.to_le_bytes());
    data[20..24].copy_from_slice(&mat.roughness.to_le_bytes());
    let content_hash = fnv1a(&data);

    let mut extended_features: u8 = 0;
    if mat.clearcoat > 0.0 {
        extended_features |= 1 << 0;
    }
    if mat.sheen_color[0] > 0.0 || mat.sheen_color[1] > 0.0 || mat.sheen_color[2] > 0.0 {
        extended_features |= 1 << 1;
    }
    if mat.transmission > 0.0 {
        extended_features |= 1 << 2;
    }
    if mat.anisotropy.abs() > 0.0 {
        extended_features |= 1 << 3;
    }
    if mat.subsurface > 0.0 {
        extended_features |= 1 << 4;
    }
    if mat.emission_strength > 0.0 {
        extended_features |= 1 << 5;
    }

    SdfMaterialEdgeSnapshot {
        content_hash,
        base_color: mat.base_color,
        metallic: mat.metallic,
        roughness: mat.roughness,
        emission_strength: mat.emission_strength,
        extended_features,
        ior: mat.ior,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_material() -> Material {
        Material::default()
    }

    fn metal_material() -> Material {
        Material::metal("Chrome", 0.9, 0.9, 0.9, 0.2)
    }

    fn glass_material() -> Material {
        Material::glass("Glass", 1.5)
    }

    fn complex_material() -> Material {
        Material::metal("Complex", 0.8, 0.1, 0.1, 0.3)
            .with_clearcoat(0.8, 0.1)
            .with_sheen(0.5, 0.3, 0.1, 0.6)
            .with_anisotropy(0.7, 0.5)
            .with_subsurface(0.4, 1.0, 0.5, 0.4)
            .with_emission(1.0, 0.5, 0.0, 5.0)
            .with_albedo_map("textures/albedo.png")
            .with_normal_map("textures/normal.png")
    }

    // -- Bridge 1 tests --

    #[test]
    fn test_material_to_view_descriptor_default() {
        let desc = sdf_material_to_view_descriptor(&default_material());
        assert_ne!(desc.content_hash, 0);
        assert_eq!(desc.metallic, 0.0);
        assert_eq!(desc.roughness, 0.5);
        assert_eq!(desc.opacity, 1.0);
        assert!(!desc.has_textures);
        assert_eq!(desc.texture_slot_count, 0);
    }

    #[test]
    fn test_material_to_view_descriptor_with_textures() {
        let mat = Material::new("Textured")
            .with_albedo_map("tex/albedo.png")
            .with_normal_map("tex/normal.png");
        let desc = sdf_material_to_view_descriptor(&mat);
        assert!(desc.has_textures);
        assert_eq!(desc.texture_slot_count, 2);
    }

    // -- Bridge 2 tests --

    #[test]
    fn test_material_to_cdn_entry_default() {
        let entry = sdf_material_to_cdn_entry(&default_material());
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.content_type_id, 0x20);
        assert_eq!(entry.texture_count, 0);
        assert_eq!(entry.estimated_bytes, 256);
        assert!(!entry.is_transmissive);
    }

    #[test]
    fn test_material_to_cdn_entry_glass_transmissive() {
        let entry = sdf_material_to_cdn_entry(&glass_material());
        assert!(entry.is_transmissive);
    }

    // -- Bridge 3 tests --

    #[test]
    fn test_material_to_cache_entry_long_ttl() {
        let entry = sdf_material_to_cache_entry(&default_material());
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
        assert!(entry.is_opaque);
    }

    #[test]
    fn test_material_to_cache_entry_short_ttl_emissive() {
        let mat = Material::new("Emissive").with_emission(1.0, 0.0, 0.0, 5.0);
        let entry = sdf_material_to_cache_entry(&mat);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_material_to_cache_entry_short_ttl_transmissive() {
        let entry = sdf_material_to_cache_entry(&glass_material());
        assert_eq!(entry.ttl_secs, 30);
        assert!(!entry.is_opaque);
    }

    // -- Bridge 4 tests --

    #[test]
    fn test_material_to_analytics_event_default() {
        let event = sdf_material_to_analytics_event(&default_material());
        assert_ne!(event.content_hash, 0);
        assert!(!event.is_metal);
        assert!(!event.has_clearcoat);
        assert!(!event.has_subsurface);
        assert!(!event.has_sheen);
        assert!(!event.has_anisotropy);
        assert!(!event.has_transmission);
        assert_eq!(event.texture_complexity, 0);
        assert_eq!(event.pbr_complexity, 0);
    }

    #[test]
    fn test_material_to_analytics_event_complex() {
        let event = sdf_material_to_analytics_event(&complex_material());
        assert!(event.is_metal);
        assert!(event.has_clearcoat);
        assert!(event.has_subsurface);
        assert!(event.has_sheen);
        assert!(event.has_anisotropy);
        assert_eq!(event.texture_complexity, 2);
        assert!(event.pbr_complexity >= 5);
    }

    // -- Bridge 5 tests --

    #[test]
    fn test_material_to_edge_snapshot_default() {
        let snap = sdf_material_to_edge_snapshot(&default_material());
        assert_ne!(snap.content_hash, 0);
        assert_eq!(snap.metallic, 0.0);
        assert_eq!(snap.roughness, 0.5);
        assert_eq!(snap.emission_strength, 0.0);
        assert_eq!(snap.extended_features, 0);
        assert_eq!(snap.ior, 1.5);
    }

    #[test]
    fn test_material_to_edge_snapshot_complex_features() {
        let snap = sdf_material_to_edge_snapshot(&complex_material());
        // clearcoat(bit0) + sheen(bit1) + anisotropy(bit3) + subsurface(bit4) + emissive(bit5)
        assert_ne!(snap.extended_features & (1 << 0), 0, "clearcoat bit");
        assert_ne!(snap.extended_features & (1 << 1), 0, "sheen bit");
        assert_ne!(snap.extended_features & (1 << 3), 0, "anisotropy bit");
        assert_ne!(snap.extended_features & (1 << 4), 0, "subsurface bit");
        assert_ne!(snap.extended_features & (1 << 5), 0, "emissive bit");
    }

    // -- Hash determinism --

    #[test]
    fn test_hash_determinism() {
        let mat = metal_material();
        let h1 = sdf_material_to_view_descriptor(&mat).content_hash;
        let h2 = sdf_material_to_view_descriptor(&mat).content_hash;
        assert_eq!(h1, h2, "same material → same hash");
    }

    #[test]
    fn test_different_materials_different_hash() {
        let h1 = sdf_material_to_view_descriptor(&default_material()).content_hash;
        let h2 = sdf_material_to_view_descriptor(&metal_material()).content_hash;
        assert_ne!(h1, h2, "different materials → different hash");
    }
}
