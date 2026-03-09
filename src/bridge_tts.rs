//! TTS bridges — ALICE-TTS ↔ Cache, DB, Streaming, Analytics, CDN
//!
//! 5 bridges connecting text-to-speech synthesis to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// Bridge 1: TTS → Cache (synthesized audio cache)
pub struct TtsCacheEntry {
    pub content_hash: u64,
    pub text_len: usize,
    pub ttl_secs: u32,
    pub sample_count: usize,
}

#[inline]
#[must_use]
pub fn tts_to_cache(text: &str, sample_count: usize, ttl_secs: u32) -> TtsCacheEntry {
    TtsCacheEntry {
        content_hash: fnv1a(b"tts_cache") ^ fnv1a(text.as_bytes()) ^ (sample_count as u64),
        text_len: text.len(),
        ttl_secs,
        sample_count,
    }
}

// Bridge 2: TTS → DB (utterance log)
pub struct TtsDbUtterance {
    pub content_hash: u64,
    pub text_len: usize,
    pub duration_ms: u64,
    pub sample_rate: u32,
}

#[inline]
#[must_use]
pub fn tts_to_db(text: &str, sample_count: usize, sample_rate: u32) -> TtsDbUtterance {
    let duration_ms = if sample_rate > 0 {
        (sample_count as u64 * 1000) / u64::from(sample_rate)
    } else {
        0
    };
    TtsDbUtterance {
        content_hash: fnv1a(b"tts_db") ^ fnv1a(text.as_bytes()),
        text_len: text.len(),
        duration_ms,
        sample_rate,
    }
}

// Bridge 3: TTS → Streaming (audio packet)
pub struct TtsStreamPacket {
    pub content_hash: u64,
    pub payload_bytes: usize,
    pub sample_rate: u32,
    pub is_final: bool,
}

#[inline]
#[must_use]
pub fn tts_to_stream(samples: &[f32], sample_rate: u32, is_final: bool) -> TtsStreamPacket {
    TtsStreamPacket {
        content_hash: fnv1a(b"tts_stream") ^ (samples.len() as u64),
        payload_bytes: samples.len() * 4,
        sample_rate,
        is_final,
    }
}

// Bridge 4: TTS → Analytics (synthesis metrics)
pub struct TtsAnalyticsMetric {
    pub content_hash: u64,
    pub text_len: usize,
    pub phoneme_count: usize,
    pub duration_ms: u64,
}

#[inline]
#[must_use]
pub fn tts_to_analytics(text: &str, phoneme_count: usize, duration_ms: u64) -> TtsAnalyticsMetric {
    TtsAnalyticsMetric {
        content_hash: fnv1a(b"tts_analytics") ^ fnv1a(text.as_bytes()) ^ (phoneme_count as u64),
        text_len: text.len(),
        phoneme_count,
        duration_ms,
    }
}

// Bridge 5: TTS → CDN (audio asset delivery)
pub struct TtsCdnAsset {
    pub content_hash: u64,
    pub asset_bytes: usize,
    pub sample_rate: u32,
    pub cache_control_secs: u32,
}

#[inline]
#[must_use]
pub fn tts_to_cdn(samples: &[f32], sample_rate: u32, cache_control_secs: u32) -> TtsCdnAsset {
    TtsCdnAsset {
        content_hash: fnv1a(b"tts_cdn") ^ (samples.len() as u64),
        asset_bytes: samples.len() * 4,
        sample_rate,
        cache_control_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_cache_hash_nonzero() {
        let c = tts_to_cache("hello world", 22050, 300);
        assert_ne!(c.content_hash, 0);
        assert_eq!(c.text_len, 11);
    }

    #[test]
    fn test_tts_cache_ttl() {
        let c = tts_to_cache("test", 8000, 60);
        assert_eq!(c.ttl_secs, 60);
        assert_eq!(c.sample_count, 8000);
    }

    #[test]
    fn test_tts_db_duration() {
        let r = tts_to_db("hello", 22050, 22050);
        assert_eq!(r.duration_ms, 1000);
        assert_ne!(r.content_hash, 0);
    }

    #[test]
    fn test_tts_db_zero_rate() {
        let r = tts_to_db("test", 100, 0);
        assert_eq!(r.duration_ms, 0);
    }

    #[test]
    fn test_tts_stream_packet() {
        let samples = vec![0.0f32; 512];
        let p = tts_to_stream(&samples, 22050, true);
        assert!(p.is_final);
        assert_eq!(p.payload_bytes, 512 * 4);
        assert_ne!(p.content_hash, 0);
    }

    #[test]
    fn test_tts_analytics_phoneme_count() {
        let m = tts_to_analytics("hello world", 8, 450);
        assert_eq!(m.phoneme_count, 8);
        assert_eq!(m.duration_ms, 450);
        assert_ne!(m.content_hash, 0);
    }

    #[test]
    fn test_tts_cdn_asset_bytes() {
        let samples = vec![0.0f32; 1024];
        let a = tts_to_cdn(&samples, 22050, 86400);
        assert_eq!(a.asset_bytes, 1024 * 4);
        assert_eq!(a.cache_control_secs, 86400);
    }

    #[test]
    fn test_tts_hash_determinism() {
        let m1 = tts_to_analytics("hello", 5, 200);
        let m2 = tts_to_analytics("hello", 5, 200);
        assert_eq!(m1.content_hash, m2.content_hash);
    }
}
