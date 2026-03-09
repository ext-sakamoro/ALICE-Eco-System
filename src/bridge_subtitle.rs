//! Subtitle bridges — ALICE-Subtitle ↔ DB, Cache, Analytics, CDN, Search
//!
//! 5 bridges connecting subtitle cue processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Subtitle → DB (cue storage) ────────────────────────────────

/// Subtitle cue storage record for ALICE-DB persistence.
pub struct SubtitleDbRecord {
    /// Content hash over the cue metadata.
    pub content_hash: u64,
    /// Number of subtitle cues.
    pub cue_count: u32,
    /// Hash of the BCP-47 language tag.
    pub language_hash: u64,
    /// Total content duration in milliseconds.
    pub duration_ms: u64,
    /// Hash of the subtitle format identifier (SRT, VTT, ASS…).
    pub format_hash: u64,
    /// Total character count across all cues.
    pub char_count: u64,
}

/// Serialize subtitle cue metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn subtitle_to_db_record(
    cue_count: u32,
    language_hash: u64,
    duration_ms: u64,
    format_hash: u64,
    char_count: u64,
) -> SubtitleDbRecord {
    let mut buf = [0u8; 36];
    buf[0..4].copy_from_slice(&cue_count.to_le_bytes());
    buf[4..12].copy_from_slice(&language_hash.to_le_bytes());
    buf[12..20].copy_from_slice(&duration_ms.to_le_bytes());
    buf[20..28].copy_from_slice(&format_hash.to_le_bytes());
    buf[28..36].copy_from_slice(&char_count.to_le_bytes());
    SubtitleDbRecord {
        content_hash: fnv1a(&buf),
        cue_count,
        language_hash,
        duration_ms,
        format_hash,
        char_count,
    }
}

// ── Bridge 2: Subtitle → Cache (cue cache) ───────────────────────────────

/// Subtitle cue cache entry for ALICE-Cache.
pub struct SubtitleCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of subtitle cues.
    pub cue_count: u32,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Serialized cue text size in bytes.
    pub text_bytes: u64,
    /// Hash of the BCP-47 language tag.
    pub language_hash: u64,
}

/// Build a subtitle cue cache entry for ALICE-Cache.
///
/// Large cue sets (cue_count >= 500) get a longer TTL (600 s) to amortise
/// parse cost; small sets get 120 s.
#[inline]
#[must_use]
pub fn subtitle_to_cache_entry(
    cue_count: u32,
    text_bytes: u64,
    language_hash: u64,
) -> SubtitleCacheEntry {
    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&cue_count.to_le_bytes());
    buf[4..12].copy_from_slice(&text_bytes.to_le_bytes());
    buf[12..20].copy_from_slice(&language_hash.to_le_bytes());
    let large_set = (cue_count >= 500) as u32;
    let ttl_secs = 120 + large_set * 480;
    SubtitleCacheEntry {
        content_hash: fnv1a(&buf),
        cue_count,
        ttl_secs,
        text_bytes,
        language_hash,
    }
}

// ── Bridge 3: Subtitle → Analytics (sync event) ──────────────────────────

/// Subtitle sync analytics event for ALICE-Analytics.
pub struct SubtitleAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of cues in this event window.
    pub cue_count: u32,
    /// Synchronisation error in milliseconds.
    pub sync_error_ms: u32,
    /// Words per minute reading speed.
    pub wpm: u32,
    /// Hash of the BCP-47 language tag.
    pub language_hash: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a subtitle sync analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn subtitle_to_analytics_event(
    cue_count: u32,
    sync_error_ms: u32,
    wpm: u32,
    language_hash: u64,
    timestamp_ms: u64,
) -> SubtitleAnalyticsEvent {
    let mut buf = [0u8; 28];
    buf[0..4].copy_from_slice(&cue_count.to_le_bytes());
    buf[4..8].copy_from_slice(&sync_error_ms.to_le_bytes());
    buf[8..12].copy_from_slice(&wpm.to_le_bytes());
    buf[12..20].copy_from_slice(&language_hash.to_le_bytes());
    buf[20..28].copy_from_slice(&timestamp_ms.to_le_bytes());
    SubtitleAnalyticsEvent {
        content_hash: fnv1a(&buf),
        cue_count,
        sync_error_ms,
        wpm,
        language_hash,
        timestamp_ms,
    }
}

// ── Bridge 4: Subtitle → CDN (delivery descriptor) ───────────────────────

/// CDN delivery descriptor for ALICE-CDN subtitle distribution.
pub struct SubtitleCdnDelivery {
    /// Content hash used as CDN cache key.
    pub content_hash: u64,
    /// Serialised subtitle text size in bytes.
    pub text_bytes: u64,
    /// CDN edge TTL in seconds.
    pub edge_ttl_secs: u32,
    /// Hash of the BCP-47 language tag.
    pub language_hash: u64,
    /// Hash of the subtitle format identifier.
    pub format_hash: u64,
}

/// Build a subtitle CDN delivery descriptor for ALICE-CDN.
#[inline]
#[must_use]
pub fn subtitle_to_cdn_delivery(
    text_bytes: u64,
    edge_ttl_secs: u32,
    language_hash: u64,
    format_hash: u64,
) -> SubtitleCdnDelivery {
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&text_bytes.to_le_bytes());
    buf[8..12].copy_from_slice(&edge_ttl_secs.to_le_bytes());
    buf[12..20].copy_from_slice(&language_hash.to_le_bytes());
    buf[20..28].copy_from_slice(&format_hash.to_le_bytes());
    SubtitleCdnDelivery {
        content_hash: fnv1a(&buf),
        text_bytes,
        edge_ttl_secs,
        language_hash,
        format_hash,
    }
}

// ── Bridge 5: Subtitle → Search (index entry) ────────────────────────────

/// Full-text search index entry for ALICE-Search.
pub struct SubtitleSearchIndex {
    /// Content hash used as search index key.
    pub content_hash: u64,
    /// Number of cues indexed.
    pub cue_count: u32,
    /// Total word count across all cues.
    pub word_count: u64,
    /// Hash of the BCP-47 language tag.
    pub language_hash: u64,
    /// Search shard identifier for partitioned indices.
    pub shard_id: u32,
}

/// Build a subtitle full-text search index entry for ALICE-Search.
#[inline]
#[must_use]
pub fn subtitle_to_search_index(
    cue_count: u32,
    word_count: u64,
    language_hash: u64,
    shard_id: u32,
) -> SubtitleSearchIndex {
    let mut buf = [0u8; 24];
    buf[0..4].copy_from_slice(&cue_count.to_le_bytes());
    buf[4..12].copy_from_slice(&word_count.to_le_bytes());
    buf[12..20].copy_from_slice(&language_hash.to_le_bytes());
    buf[20..24].copy_from_slice(&shard_id.to_le_bytes());
    SubtitleSearchIndex {
        content_hash: fnv1a(&buf),
        cue_count,
        word_count,
        language_hash,
        shard_id,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtitle_to_db_record_hash_nonzero() {
        let rec = subtitle_to_db_record(300, 0x1234, 7_200_000, 0x5678, 15_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_subtitle_to_db_record_fields() {
        let rec = subtitle_to_db_record(100, 0xaaaa, 3_600_000, 0xbbbb, 5_000);
        assert_eq!(rec.cue_count, 100);
        assert_eq!(rec.language_hash, 0xaaaa);
        assert_eq!(rec.duration_ms, 3_600_000);
        assert_eq!(rec.format_hash, 0xbbbb);
        assert_eq!(rec.char_count, 5_000);
    }

    #[test]
    fn test_subtitle_to_db_record_deterministic() {
        let a = subtitle_to_db_record(1, 2, 3, 4, 5);
        let b = subtitle_to_db_record(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_subtitle_to_cache_entry_small_set_ttl() {
        let entry = subtitle_to_cache_entry(50, 2_048, 0x1111);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 120);
    }

    #[test]
    fn test_subtitle_to_cache_entry_large_set_ttl() {
        let entry = subtitle_to_cache_entry(500, 40_960, 0x2222);
        assert_eq!(entry.ttl_secs, 600);
        assert_eq!(entry.cue_count, 500);
    }

    #[test]
    fn test_subtitle_to_analytics_event() {
        let ev = subtitle_to_analytics_event(200, 50, 140, 0x3333, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.sync_error_ms, 50);
        assert_eq!(ev.wpm, 140);
    }

    #[test]
    fn test_subtitle_to_cdn_delivery() {
        let d = subtitle_to_cdn_delivery(8_192, 86_400, 0x4444, 0x5555);
        assert_ne!(d.content_hash, 0);
        assert_eq!(d.text_bytes, 8_192);
        assert_eq!(d.edge_ttl_secs, 86_400);
    }

    #[test]
    fn test_subtitle_to_search_index() {
        let idx = subtitle_to_search_index(300, 4_500, 0x6666, 7);
        assert_ne!(idx.content_hash, 0);
        assert_eq!(idx.word_count, 4_500);
        assert_eq!(idx.shard_id, 7);
    }
}
