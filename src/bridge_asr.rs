//! ASR bridges — ALICE-ASR ↔ Analytics, DB, Cache, ML, Streaming
//!
//! 5 bridges connecting speech recognition to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// Bridge 1: ASR → Analytics (recognition metrics)
pub struct AsrAnalyticsMetric {
    pub content_hash: u64,
    pub sample_rate: u32,
    pub frame_count: usize,
    pub feature_dim: usize,
}

#[inline]
#[must_use]
pub fn asr_to_analytics(samples: &[f32], sample_rate: u32) -> AsrAnalyticsMetric {
    let hash = fnv1a(b"asr_analytics");
    AsrAnalyticsMetric {
        content_hash: hash ^ (samples.len() as u64),
        sample_rate,
        frame_count: samples.len() / 160,
        feature_dim: 13,
    }
}

// Bridge 2: ASR → DB (transcript persistence)
pub struct AsrDbTranscript {
    pub content_hash: u64,
    pub audio_len_samples: usize,
    pub duration_ms: u64,
    pub channel_count: u8,
}

#[inline]
#[must_use]
pub fn asr_to_db(samples: &[f32], sample_rate: u32) -> AsrDbTranscript {
    let duration_ms = if sample_rate > 0 {
        (samples.len() as u64 * 1000) / u64::from(sample_rate)
    } else {
        0
    };
    AsrDbTranscript {
        content_hash: fnv1a(b"asr_db") ^ (samples.len() as u64),
        audio_len_samples: samples.len(),
        duration_ms,
        channel_count: 1,
    }
}

// Bridge 3: ASR → Cache (feature cache)
pub struct AsrCacheEntry {
    pub content_hash: u64,
    pub ttl_secs: u32,
    pub feature_bytes: usize,
}

#[inline]
#[must_use]
pub fn asr_to_cache(samples: &[f32], ttl_secs: u32) -> AsrCacheEntry {
    let feature_bytes = (samples.len() / 160) * 13 * 4;
    AsrCacheEntry {
        content_hash: fnv1a(b"asr_cache") ^ (samples.len() as u64),
        ttl_secs,
        feature_bytes,
    }
}

// Bridge 4: ASR → ML (feature vector for inference)
pub struct AsrMlFeatures {
    pub content_hash: u64,
    pub vector_dim: usize,
    pub frame_count: usize,
    pub normalized: bool,
}

#[inline]
#[must_use]
pub fn asr_to_ml(samples: &[f32]) -> AsrMlFeatures {
    let frame_count = samples.len() / 160;
    AsrMlFeatures {
        content_hash: fnv1a(b"asr_ml") ^ (frame_count as u64),
        vector_dim: 13,
        frame_count,
        normalized: true,
    }
}

// Bridge 5: ASR → Streaming (audio packet metadata)
pub struct AsrStreamPacket {
    pub content_hash: u64,
    pub payload_bytes: usize,
    pub sample_rate: u32,
    pub is_final: bool,
}

#[inline]
#[must_use]
pub fn asr_to_stream(samples: &[f32], sample_rate: u32, is_final: bool) -> AsrStreamPacket {
    AsrStreamPacket {
        content_hash: fnv1a(b"asr_stream") ^ (samples.len() as u64),
        payload_bytes: samples.len() * 4,
        sample_rate,
        is_final,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asr_analytics_hash() {
        let samples = vec![0.1f32; 1600];
        let m = asr_to_analytics(&samples, 16000);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.sample_rate, 16000);
        assert_eq!(m.frame_count, 10);
    }

    #[test]
    fn test_asr_db_duration() {
        let samples = vec![0.0f32; 16000];
        let r = asr_to_db(&samples, 16000);
        assert_eq!(r.duration_ms, 1000);
        assert_ne!(r.content_hash, 0);
    }

    #[test]
    fn test_asr_cache_ttl() {
        let samples = vec![0.0f32; 320];
        let c = asr_to_cache(&samples, 60);
        assert_eq!(c.ttl_secs, 60);
        assert_ne!(c.content_hash, 0);
    }

    #[test]
    fn test_asr_ml_features() {
        let samples = vec![0.0f32; 3200];
        let f = asr_to_ml(&samples);
        assert_eq!(f.frame_count, 20);
        assert!(f.normalized);
    }

    #[test]
    fn test_asr_stream_final() {
        let samples = vec![0.0f32; 480];
        let p = asr_to_stream(&samples, 16000, true);
        assert!(p.is_final);
        assert_eq!(p.payload_bytes, 480 * 4);
    }

    #[test]
    fn test_asr_hash_determinism() {
        let s1 = vec![0.0f32; 1600];
        let s2 = vec![0.0f32; 1600];
        assert_eq!(
            asr_to_analytics(&s1, 16000).content_hash,
            asr_to_analytics(&s2, 16000).content_hash
        );
    }

    #[test]
    fn test_asr_db_zero_rate() {
        let r = asr_to_db(&[0.0f32; 100], 0);
        assert_eq!(r.duration_ms, 0);
    }

    #[test]
    fn test_asr_cache_feature_bytes() {
        let samples = vec![0.0f32; 1600];
        let c = asr_to_cache(&samples, 30);
        assert_eq!(c.feature_bytes, 10 * 13 * 4);
    }
}
