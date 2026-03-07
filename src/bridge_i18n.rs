//! i18n bridges — ALICE-i18n ↔ DB, Cache, CDN, Text, Edge
//!
//! 5 bridges connecting the internationalization layer to the ALICE ecosystem.
//! Covers translation storage in DB, translation caching, locale CDN delivery,
//! text processing linkage, and locale detection events via Edge.

use alice_i18n::{Locale, MessageBundle, PluralCategory};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Map a `PluralCategory` variant to its numeric code.
///
/// Zero=0, One=1, Two=2, Few=3, Many=4, Other=5.
#[inline(always)]
const fn plural_category_to_u8(cat: &PluralCategory) -> u8 {
    match cat {
        PluralCategory::Zero => 0,
        PluralCategory::One => 1,
        PluralCategory::Two => 2,
        PluralCategory::Few => 3,
        PluralCategory::Many => 4,
        PluralCategory::Other => 5,
    }
}

// ── Bridge 1: i18n → DB (translation storage record) ─────────────────────

/// Translation storage record for ALICE-DB.
///
/// Written when a message bundle is committed so the database layer can
/// store and query translations by locale tag, message key, or bundle ID.
pub struct I18nDbTranslationRecord {
    /// FNV-1a hash over locale tag and message count bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the locale tag string — database record key.
    pub locale_hash: u64,
    /// Number of messages in the bundle.
    pub message_count: u32,
    /// Byte length of the locale tag string.
    pub locale_tag_len: u8,
}

/// Convert a message bundle and locale into a DB translation record for ALICE-DB.
#[inline]
#[must_use]
pub fn i18n_bundle_to_db_record(bundle: &MessageBundle, locale: &Locale) -> I18nDbTranslationRecord {
    let tag = locale.tag();
    let locale_hash = fnv1a(tag.as_bytes());
    let message_count = bundle.locale_count() as u32;
    let mut key = [0u8; 12];
    key[0..8].copy_from_slice(&locale_hash.to_le_bytes());
    key[8..12].copy_from_slice(&message_count.to_le_bytes());
    I18nDbTranslationRecord {
        content_hash: fnv1a(&key),
        locale_hash,
        message_count,
        locale_tag_len: tag.len().min(255) as u8,
    }
}

// ── Bridge 2: i18n → Cache (translation cache entry) ─────────────────────

/// Translation cache entry for ALICE-Cache.
///
/// Caches a resolved translation message so repeated lookups avoid
/// re-traversing the fallback locale chain.
/// Locale bundles with many messages (> 500) receive a longer TTL because
/// they are expensive to re-load from DB.
pub struct I18nCacheEntry {
    /// FNV-1a hash over locale hash and message key hash bytes — cache key.
    pub content_hash: u64,
    /// FNV-1a hash of the locale tag.
    pub locale_hash: u64,
    /// FNV-1a hash of the message key.
    pub message_key_hash: u64,
    /// Byte length of the resolved message string.
    pub message_byte_len: usize,
    /// Cache TTL in seconds: 600 for large bundles (> 500 messages), else 120.
    pub ttl_secs: u32,
}

/// Build a translation cache entry for ALICE-Cache.
///
/// TTL is computed branchlessly: large=1 → 120+480=600, small=0 → 120.
#[inline]
#[must_use]
pub fn i18n_translation_to_cache_entry(
    locale: &Locale,
    message_key: &str,
    resolved_message: &str,
    bundle_message_count: u32,
) -> I18nCacheEntry {
    let locale_hash = fnv1a(locale.tag().as_bytes());
    let message_key_hash = fnv1a(message_key.as_bytes());
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&locale_hash.to_le_bytes());
    key[8..16].copy_from_slice(&message_key_hash.to_le_bytes());
    // Branchless TTL: large=1 → 120+480=600, small=0 → 120.
    let large = (bundle_message_count > 500) as u32;
    let ttl_secs = 120 + large * 480;
    I18nCacheEntry {
        content_hash: fnv1a(&key),
        locale_hash,
        message_key_hash,
        message_byte_len: resolved_message.len(),
        ttl_secs,
    }
}

// ── Bridge 3: i18n → CDN (locale delivery descriptor) ────────────────────

/// Locale delivery descriptor for ALICE-CDN.
///
/// Packages a locale bundle for CDN delivery so the CDN layer can route
/// locale file requests to the nearest edge PoP and set appropriate
/// language-specific cache-control headers.
pub struct I18nCdnLocaleDelivery {
    /// FNV-1a hash over locale tag and format bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the locale tag — CDN object key component.
    pub locale_hash: u64,
    /// Locale tag string byte length.
    pub locale_tag_len: u8,
    /// Bundle format: 0=JSON, 1=YAML, 2=Fluent, 3=PO.
    pub format: u8,
    /// CDN cache-control max-age in seconds (always 3600 for locale bundles).
    pub max_age_secs: u32,
}

/// Build a CDN locale delivery descriptor for ALICE-CDN.
#[inline]
#[must_use]
pub fn i18n_locale_to_cdn_delivery(locale: &Locale, format: u8) -> I18nCdnLocaleDelivery {
    let tag = locale.tag();
    let locale_hash = fnv1a(tag.as_bytes());
    let fmt = format.min(3);
    let mut key = [0u8; 9];
    key[0..8].copy_from_slice(&locale_hash.to_le_bytes());
    key[8] = fmt;
    I18nCdnLocaleDelivery {
        content_hash: fnv1a(&key),
        locale_hash,
        locale_tag_len: tag.len().min(255) as u8,
        format: fmt,
        max_age_secs: 3_600,
    }
}

// ── Bridge 4: i18n → Text (text processing link) ─────────────────────────

/// Text processing link for ALICE-Text.
///
/// Connects a locale and plural category to the text processing layer so
/// ALICE-Text can apply locale-aware tokenization, stemming, and
/// segmentation rules for the correct language.
pub struct I18nTextProcessingLink {
    /// FNV-1a hash over locale tag and plural category byte.
    pub content_hash: u64,
    /// FNV-1a hash of the locale tag — text processor routing key.
    pub locale_hash: u64,
    /// Plural category code: 0=Zero … 5=Other.
    pub plural_category: u8,
    /// Language string byte length for text processor initialization.
    pub language_byte_len: u8,
    /// True when the locale has a regional variant (region field is Some).
    pub has_region: bool,
}

/// Build a text processing link from a locale and plural category for ALICE-Text.
#[inline]
#[must_use]
pub fn i18n_locale_to_text_link(
    locale: &Locale,
    plural_cat: &PluralCategory,
) -> I18nTextProcessingLink {
    let tag = locale.tag();
    let locale_hash = fnv1a(tag.as_bytes());
    let cat_byte = plural_category_to_u8(plural_cat);
    let mut key = [0u8; 9];
    key[0..8].copy_from_slice(&locale_hash.to_le_bytes());
    key[8] = cat_byte;
    I18nTextProcessingLink {
        content_hash: fnv1a(&key),
        locale_hash,
        plural_category: cat_byte,
        language_byte_len: locale.language.len().min(255) as u8,
        has_region: locale.region.is_some(),
    }
}

// ── Bridge 5: i18n → Edge (locale detection event) ───────────────────────

/// Locale detection event for ALICE-Edge.
///
/// Packages a client locale detection result as an edge event so the
/// edge layer can route requests to the correct locale-specific CDN path
/// and apply language-based A/B testing.
pub struct I18nEdgeLocaleEvent {
    /// FNV-1a hash over locale tag and detection method byte.
    pub content_hash: u64,
    /// FNV-1a hash of the detected locale tag.
    pub locale_hash: u64,
    /// Detection method: 0=Accept-Language header, 1=IP geolocation,
    /// 2=user preference, 3=URL prefix.
    pub detection_method: u8,
    /// Detection confidence in permille (0–1000).
    pub confidence_permille: u32,
    /// True when a fallback locale was used (primary not available).
    pub is_fallback: bool,
}

/// Build a locale detection edge event for ALICE-Edge.
#[inline]
#[must_use]
pub fn i18n_locale_to_edge_event(
    locale: &Locale,
    detection_method: u8,
    confidence: f64,
    is_fallback: bool,
) -> I18nEdgeLocaleEvent {
    let tag = locale.tag();
    let locale_hash = fnv1a(tag.as_bytes());
    let method = detection_method.min(3);
    let mut key = [0u8; 9];
    key[0..8].copy_from_slice(&locale_hash.to_le_bytes());
    key[8] = method;
    let confidence_permille = (confidence.clamp(0.0, 1.0) * 1000.0) as u32;
    I18nEdgeLocaleEvent {
        content_hash: fnv1a(&key),
        locale_hash,
        detection_method: method,
        confidence_permille,
        is_fallback,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_i18n::{Locale, MessageBundle, PluralCategory};

    fn make_locale(language: &str, region: Option<&str>) -> Locale {
        Locale {
            language: language.to_string(),
            region: region.map(|r| r.to_string()),
        }
    }

    fn make_bundle_with_entries(locale: &Locale, keys: &[(&str, &str)]) -> MessageBundle {
        let mut bundle = MessageBundle::new();
        let tag = locale.tag();
        for &(k, v) in keys {
            bundle.add(&tag, k, v);
        }
        bundle
    }

    #[test]
    fn test_bundle_to_db_record() {
        let locale = make_locale("ja", Some("JP"));
        let bundle = make_bundle_with_entries(&locale, &[
            ("hello", "こんにちは"),
            ("bye", "さようなら"),
        ]);
        let rec = i18n_bundle_to_db_record(&bundle, &locale);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.locale_hash, 0);
        assert!(rec.locale_tag_len > 0);
    }

    #[test]
    fn test_translation_cache_entry_small_bundle_ttl() {
        // bundle_message_count = 100 <= 500 → ttl = 120
        let locale = make_locale("en", Some("US"));
        let entry = i18n_translation_to_cache_entry(&locale, "greeting", "Hello", 100);
        assert_ne!(entry.content_hash, 0);
        assert_ne!(entry.locale_hash, 0);
        assert_ne!(entry.message_key_hash, 0);
        assert_eq!(entry.ttl_secs, 120);
        assert_eq!(entry.message_byte_len, 5); // "Hello"
    }

    #[test]
    fn test_translation_cache_entry_large_bundle_ttl() {
        // bundle_message_count = 1000 > 500 → ttl = 600
        let locale = make_locale("de", Some("DE"));
        let entry = i18n_translation_to_cache_entry(&locale, "title", "Titel", 1000);
        assert_eq!(entry.ttl_secs, 600);
    }

    #[test]
    fn test_cdn_locale_delivery_format_clamped() {
        let locale = make_locale("fr", Some("FR"));
        let d = i18n_locale_to_cdn_delivery(&locale, 99);
        assert_eq!(d.format, 3); // clamped to PO
        assert_eq!(d.max_age_secs, 3_600);
        assert_ne!(d.locale_hash, 0);
    }

    #[test]
    fn test_cdn_locale_delivery_json() {
        let locale = make_locale("zh", Some("CN"));
        let d = i18n_locale_to_cdn_delivery(&locale, 0);
        assert_eq!(d.format, 0); // JSON
        assert!(d.locale_tag_len > 0);
    }

    #[test]
    fn test_text_link_with_region() {
        let locale = make_locale("pt", Some("BR"));
        let link = i18n_locale_to_text_link(&locale, &PluralCategory::One);
        assert_ne!(link.content_hash, 0);
        assert_eq!(link.plural_category, 1); // One → 1
        assert!(link.has_region);
        assert!(link.language_byte_len > 0);
    }

    #[test]
    fn test_text_link_without_region() {
        let locale = make_locale("ar", None);
        let link = i18n_locale_to_text_link(&locale, &PluralCategory::Many);
        assert_eq!(link.plural_category, 4); // Many → 4
        assert!(!link.has_region);
    }

    #[test]
    fn test_edge_locale_event_accept_language() {
        let locale = make_locale("ko", Some("KR"));
        let ev = i18n_locale_to_edge_event(&locale, 0, 0.95, false);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.detection_method, 0);
        assert_eq!(ev.confidence_permille, 950);
        assert!(!ev.is_fallback);
    }

    #[test]
    fn test_edge_locale_event_fallback_detection_method_clamped() {
        let locale = make_locale("es", Some("MX"));
        let ev = i18n_locale_to_edge_event(&locale, 99, 0.5, true);
        assert_eq!(ev.detection_method, 3); // clamped to URL prefix
        assert!(ev.is_fallback);
        assert_eq!(ev.confidence_permille, 500);
    }

    #[test]
    fn test_hash_determinism() {
        let locale = make_locale("en", None);
        let bundle = make_bundle_with_entries(&locale, &[("k", "v")]);
        let r1 = i18n_bundle_to_db_record(&bundle, &locale);
        let r2 = i18n_bundle_to_db_record(&bundle, &locale);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.locale_hash, r2.locale_hash);
    }
}
