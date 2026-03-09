//! OCR bridges — ALICE-OCR ↔ DB, Cache, Analytics, Search, ML
//!
//! 5 bridges connecting optical character recognition to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// Bridge 1: OCR → DB (recognition result persistence)
pub struct OcrDbResult {
    pub content_hash: u64,
    pub image_width: u32,
    pub image_height: u32,
    pub character_count: usize,
}

#[inline]
#[must_use]
pub fn ocr_to_db(image_width: u32, image_height: u32, character_count: usize) -> OcrDbResult {
    let pixel_count = u64::from(image_width) * u64::from(image_height);
    OcrDbResult {
        content_hash: fnv1a(b"ocr_db") ^ pixel_count ^ (character_count as u64),
        image_width,
        image_height,
        character_count,
    }
}

// Bridge 2: OCR → Cache (template cache)
pub struct OcrCacheEntry {
    pub content_hash: u64,
    pub template_bytes: usize,
    pub ttl_secs: u32,
    pub image_width: u32,
}

#[inline]
#[must_use]
pub fn ocr_to_cache(image_width: u32, image_height: u32, ttl_secs: u32) -> OcrCacheEntry {
    let template_bytes = (image_width as usize) * (image_height as usize);
    OcrCacheEntry {
        content_hash: fnv1a(b"ocr_cache") ^ (u64::from(image_width) * u64::from(image_height)),
        template_bytes,
        ttl_secs,
        image_width,
    }
}

// Bridge 3: OCR → Analytics (accuracy metrics)
pub struct OcrAnalyticsMetric {
    pub content_hash: u64,
    pub character_count: usize,
    pub confidence_pct: u8,
    pub region_count: usize,
}

#[inline]
#[must_use]
pub fn ocr_to_analytics(
    character_count: usize,
    confidence_pct: u8,
    region_count: usize,
) -> OcrAnalyticsMetric {
    OcrAnalyticsMetric {
        content_hash: fnv1a(b"ocr_analytics")
            ^ (character_count as u64)
            ^ (u64::from(confidence_pct)),
        character_count,
        confidence_pct,
        region_count,
    }
}

// Bridge 4: OCR → Search (indexed text)
pub struct OcrSearchDoc {
    pub content_hash: u64,
    pub text_len: usize,
    pub token_count: usize,
    pub language_hint: u8,
}

#[inline]
#[must_use]
pub fn ocr_to_search(text: &str, language_hint: u8) -> OcrSearchDoc {
    let token_count = text.split_whitespace().count();
    OcrSearchDoc {
        content_hash: fnv1a(b"ocr_search") ^ fnv1a(text.as_bytes()),
        text_len: text.len(),
        token_count,
        language_hint,
    }
}

// Bridge 5: OCR → ML (feature extraction for inference)
pub struct OcrMlFeatures {
    pub content_hash: u64,
    pub feature_dim: usize,
    pub patch_count: usize,
    pub normalized: bool,
}

#[inline]
#[must_use]
pub fn ocr_to_ml(image_width: u32, image_height: u32, patch_size: u32) -> OcrMlFeatures {
    let patch_count = if patch_size > 0 {
        ((image_width / patch_size) * (image_height / patch_size)) as usize
    } else {
        0
    };
    OcrMlFeatures {
        content_hash: fnv1a(b"ocr_ml") ^ (u64::from(image_width) * u64::from(image_height)),
        feature_dim: 768,
        patch_count,
        normalized: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_db_hash_nonzero() {
        let r = ocr_to_db(1920, 1080, 256);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.image_width, 1920);
        assert_eq!(r.character_count, 256);
    }

    #[test]
    fn test_ocr_db_dimensions() {
        let r = ocr_to_db(640, 480, 42);
        assert_eq!(r.image_height, 480);
        assert_eq!(r.character_count, 42);
    }

    #[test]
    fn test_ocr_cache_template_bytes() {
        let c = ocr_to_cache(320, 240, 120);
        assert_eq!(c.template_bytes, 320 * 240);
        assert_eq!(c.ttl_secs, 120);
        assert_ne!(c.content_hash, 0);
    }

    #[test]
    fn test_ocr_analytics_confidence() {
        let m = ocr_to_analytics(100, 95, 4);
        assert_eq!(m.confidence_pct, 95);
        assert_eq!(m.region_count, 4);
        assert_ne!(m.content_hash, 0);
    }

    #[test]
    fn test_ocr_search_token_count() {
        let d = ocr_to_search("hello world foo bar", 0);
        assert_eq!(d.token_count, 4);
        assert_eq!(d.text_len, 19);
        assert_ne!(d.content_hash, 0);
    }

    #[test]
    fn test_ocr_ml_patch_count() {
        let f = ocr_to_ml(224, 224, 16);
        assert_eq!(f.patch_count, 196);
        assert!(f.normalized);
        assert_ne!(f.content_hash, 0);
    }

    #[test]
    fn test_ocr_ml_zero_patch_size() {
        let f = ocr_to_ml(224, 224, 0);
        assert_eq!(f.patch_count, 0);
    }

    #[test]
    fn test_ocr_hash_determinism() {
        let m1 = ocr_to_analytics(50, 80, 2);
        let m2 = ocr_to_analytics(50, 80, 2);
        assert_eq!(m1.content_hash, m2.content_hash);
    }
}
