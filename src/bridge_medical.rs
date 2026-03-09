//! Medical bridges — ALICE-Medical ↔ DB, Cache, Analytics, Render, ML
//!
//! 5 bridges connecting medical imaging to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// Bridge 1: Medical → DB (DICOM storage)
pub struct MedicalDbDicom {
    pub content_hash: u64,
    pub slice_count: usize,
    pub image_width: u32,
    pub image_height: u32,
}

#[inline]
#[must_use]
pub fn medical_to_db(slice_count: usize, image_width: u32, image_height: u32) -> MedicalDbDicom {
    let pixel_volume = u64::from(image_width) * u64::from(image_height) * slice_count as u64;
    MedicalDbDicom {
        content_hash: fnv1a(b"medical_db") ^ pixel_volume,
        slice_count,
        image_width,
        image_height,
    }
}

// Bridge 2: Medical → Cache (volume cache)
pub struct MedicalCacheVolume {
    pub content_hash: u64,
    pub voxel_count: usize,
    pub ttl_secs: u32,
    pub bytes_per_voxel: u8,
}

#[inline]
#[must_use]
pub fn medical_to_cache(
    voxel_count: usize,
    ttl_secs: u32,
    bytes_per_voxel: u8,
) -> MedicalCacheVolume {
    MedicalCacheVolume {
        content_hash: fnv1a(b"medical_cache") ^ (voxel_count as u64) ^ u64::from(bytes_per_voxel),
        voxel_count,
        ttl_secs,
        bytes_per_voxel,
    }
}

// Bridge 3: Medical → Analytics (diagnostic metrics)
pub struct MedicalAnalyticsMetric {
    pub content_hash: u64,
    pub slice_count: usize,
    pub voxel_count: usize,
    pub hounsfield_min: i16,
}

#[inline]
#[must_use]
pub fn medical_to_analytics(
    slice_count: usize,
    voxel_count: usize,
    hounsfield_min: i16,
) -> MedicalAnalyticsMetric {
    MedicalAnalyticsMetric {
        content_hash: fnv1a(b"medical_analytics")
            ^ (slice_count as u64)
            ^ (voxel_count as u64).wrapping_mul(0x3f),
        slice_count,
        voxel_count,
        hounsfield_min,
    }
}

// Bridge 4: Medical → Render (volume visualization)
pub struct MedicalRenderVolume {
    pub content_hash: u64,
    pub voxel_count: usize,
    pub dim_x: u32,
    pub dim_y: u32,
    pub dim_z: u32,
}

#[inline]
#[must_use]
pub fn medical_to_render(dim_x: u32, dim_y: u32, dim_z: u32) -> MedicalRenderVolume {
    let voxel_count = u64::from(dim_x) * u64::from(dim_y) * u64::from(dim_z);
    MedicalRenderVolume {
        content_hash: fnv1a(b"medical_render") ^ voxel_count,
        voxel_count: voxel_count as usize,
        dim_x,
        dim_y,
        dim_z,
    }
}

// Bridge 5: Medical → ML (segmentation input)
pub struct MedicalMlInput {
    pub content_hash: u64,
    pub voxel_count: usize,
    pub patch_size: u32,
    pub normalized: bool,
}

#[inline]
#[must_use]
pub fn medical_to_ml(voxel_count: usize, patch_size: u32) -> MedicalMlInput {
    MedicalMlInput {
        content_hash: fnv1a(b"medical_ml") ^ (voxel_count as u64) ^ u64::from(patch_size),
        voxel_count,
        patch_size,
        normalized: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_medical_db_hash_nonzero() {
        let r = medical_to_db(256, 512, 512);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.slice_count, 256);
        assert_eq!(r.image_width, 512);
    }

    #[test]
    fn test_medical_db_dimensions() {
        let r = medical_to_db(128, 256, 256);
        assert_eq!(r.image_height, 256);
        assert_eq!(r.slice_count, 128);
    }

    #[test]
    fn test_medical_cache_voxel_count() {
        let c = medical_to_cache(512 * 512 * 256, 600, 2);
        assert_eq!(c.voxel_count, 512 * 512 * 256);
        assert_eq!(c.ttl_secs, 600);
        assert_ne!(c.content_hash, 0);
    }

    #[test]
    fn test_medical_cache_bytes_per_voxel() {
        let c = medical_to_cache(1000, 300, 4);
        assert_eq!(c.bytes_per_voxel, 4);
    }

    #[test]
    fn test_medical_analytics_hounsfield() {
        let m = medical_to_analytics(64, 64 * 64 * 64, -1024);
        assert_eq!(m.hounsfield_min, -1024);
        assert_ne!(m.content_hash, 0);
    }

    #[test]
    fn test_medical_render_dimensions() {
        let v = medical_to_render(512, 512, 256);
        assert_eq!(v.dim_x, 512);
        assert_eq!(v.dim_z, 256);
        assert_eq!(v.voxel_count, 512 * 512 * 256);
        assert_ne!(v.content_hash, 0);
    }

    #[test]
    fn test_medical_ml_normalized() {
        let f = medical_to_ml(1_000_000, 64);
        assert!(f.normalized);
        assert_eq!(f.patch_size, 64);
        assert_ne!(f.content_hash, 0);
    }

    #[test]
    fn test_medical_hash_determinism() {
        let m1 = medical_to_analytics(100, 512_000, -500);
        let m2 = medical_to_analytics(100, 512_000, -500);
        assert_eq!(m1.content_hash, m2.content_hash);
    }
}
