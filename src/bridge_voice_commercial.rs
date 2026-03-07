//! Voice-Commercial bridges — ALICE-Voice-Commercial ↔ Voice, ML, Analytics, Cache, Edge
//!
//! 5 bridges connecting the commercial voice processing layer
//! (semantic VAD, layered analysis) to the ALICE ecosystem.

#[cfg(feature = "voice-commercial")]
use alice_voice_commercial::{VadConfig, VadDetector, VadState};

// ── Bridge 1: VoiceCommercial → Analytics (VAD telemetry) ───────────────

/// VAD processing telemetry for ALICE-Analytics.
#[cfg(feature = "voice-commercial")]
pub struct VoiceCommercialAnalyticsTelemetry {
    /// Deterministic hash of the frame data.
    pub content_hash: u64,
    /// Whether speech was detected this frame.
    pub is_speech: bool,
    /// Frame energy level (RMS).
    pub energy: f32,
    /// Zero crossing rate for the frame.
    pub zcr: f32,
    /// Current VAD state discriminant (0=Silence, 1=Speech, 2=Hangover).
    pub state: u8,
}

/// Process an audio frame through commercial VAD and emit telemetry.
#[cfg(feature = "voice-commercial")]
#[inline]
#[must_use]
pub fn voice_commercial_to_analytics_vad(
    detector: &mut VadDetector,
    frame: &[f32],
) -> VoiceCommercialAnalyticsTelemetry {
    let content_hash = fnv1a(bytemuck_f32_to_u8(frame));
    let energy = alice_voice_commercial::frame_energy(frame);
    let zcr = alice_voice_commercial::zero_crossing_rate(frame);
    let is_speech = detector.process_frame(frame);
    let state = match detector.state() {
        VadState::Silence => 0,
        VadState::Speech => 1,
        VadState::Hangover => 2,
    };
    VoiceCommercialAnalyticsTelemetry {
        content_hash,
        is_speech,
        energy,
        zcr,
        state,
    }
}

// ── Bridge 2: VoiceCommercial → ML (VAD features for model input) ───────

/// VAD feature vector for ALICE-ML ternary inference.
#[cfg(feature = "voice-commercial")]
pub struct VoiceCommercialMlFeatures {
    /// Deterministic hash of features.
    pub content_hash: u64,
    /// Feature vector: [energy, zcr, delta_energy, spectral_centroid_approx].
    pub features: [f32; 4],
    /// Feature dimensionality.
    pub dim: usize,
}

/// Extract ML-ready features from an audio frame for speech classification.
#[cfg(feature = "voice-commercial")]
#[inline]
#[must_use]
pub fn voice_commercial_to_ml_features(
    frame: &[f32],
    prev_energy: f32,
) -> VoiceCommercialMlFeatures {
    let energy = alice_voice_commercial::frame_energy(frame);
    let zcr = alice_voice_commercial::zero_crossing_rate(frame);
    let delta_energy = energy - prev_energy;
    let (num, den) = frame
        .iter()
        .enumerate()
        .fold((0.0f32, 0.0f32), |(n, d), (i, &s)| {
            let mag = s.abs();
            ((i as f32).mul_add(mag, n), mag + d)
        });
    let spectral_centroid = if den > 1e-10 { num / den } else { 0.0 };
    let features = [energy, zcr, delta_energy, spectral_centroid];
    let content_hash = fnv1a(bytemuck_f32_to_u8(&features));
    VoiceCommercialMlFeatures {
        content_hash,
        features,
        dim: 4,
    }
}

// ── Bridge 3: VoiceCommercial → Voice (config bridging) ─────────────────

/// Standardized VAD configuration for ALICE-Voice pipeline integration.
#[cfg(feature = "voice-commercial")]
pub struct VoiceCommercialVoiceConfig {
    /// Deterministic hash of config values.
    pub content_hash: u64,
    /// Energy threshold for speech detection.
    pub energy_threshold: f32,
    /// ZCR low bound.
    pub zcr_low: f32,
    /// ZCR high bound.
    pub zcr_high: f32,
    /// Hangover frames after speech ends.
    pub hangover_frames: u32,
}

/// Create a default commercial VAD config for ALICE-Voice pipeline.
#[cfg(feature = "voice-commercial")]
#[inline]
#[must_use]
pub fn voice_commercial_to_voice_config() -> VoiceCommercialVoiceConfig {
    let config = VadConfig::default();
    let mut buf = [0u8; 16];
    buf[..4].copy_from_slice(&config.energy_threshold.to_le_bytes());
    buf[4..8].copy_from_slice(&config.zcr_low.to_le_bytes());
    buf[8..12].copy_from_slice(&config.zcr_high.to_le_bytes());
    buf[12..16].copy_from_slice(&config.hangover_frames.to_le_bytes());
    VoiceCommercialVoiceConfig {
        content_hash: fnv1a(&buf),
        energy_threshold: config.energy_threshold,
        zcr_low: config.zcr_low,
        zcr_high: config.zcr_high,
        hangover_frames: config.hangover_frames,
    }
}

// ── Bridge 4: VoiceCommercial → Cache (VAD state TTL) ───────────────────

/// VAD state cache entry for ALICE-Cache.
#[cfg(feature = "voice-commercial")]
pub struct VoiceCommercialCacheEntry {
    /// Deterministic hash of state.
    pub content_hash: u64,
    /// Whether speech is active.
    pub is_speech: bool,
    /// TTL in seconds (speech=10s, silence=2s).
    pub ttl_secs: u32,
    /// Whether to cache.
    pub cacheable: bool,
}

/// Create a cache entry for VAD detection result.
#[cfg(feature = "voice-commercial")]
#[inline]
#[must_use]
pub fn voice_commercial_to_cache_state(is_speech: bool) -> VoiceCommercialCacheEntry {
    let state_byte = u8::from(is_speech);
    // Branchless TTL: speech=10s, silence=2s
    let condition = u32::from(is_speech);
    let ttl_secs = 2 + condition * 8;
    VoiceCommercialCacheEntry {
        content_hash: fnv1a(&[state_byte]),
        is_speech,
        ttl_secs,
        cacheable: true,
    }
}

// ── Bridge 5: VoiceCommercial → Edge (VAD event for stream routing) ─────

/// VAD event for ALICE-Edge stream routing.
#[cfg(feature = "voice-commercial")]
pub struct VoiceCommercialEdgeEvent {
    /// Deterministic hash of (is_speech, energy).
    pub content_hash: u64,
    /// Is this a speech segment?
    pub is_speech: bool,
    /// Energy level (for threshold routing).
    pub energy: f32,
    /// Priority (speech=5, silence=1).
    pub priority: u8,
}

/// Create an edge routing event from VAD analysis.
#[cfg(feature = "voice-commercial")]
#[inline]
#[must_use]
pub fn voice_commercial_to_edge_event(is_speech: bool, energy: f32) -> VoiceCommercialEdgeEvent {
    let priority = if is_speech { 5 } else { 1 };
    let mut buf = [0u8; 5];
    buf[0] = u8::from(is_speech);
    buf[1..5].copy_from_slice(&energy.to_le_bytes());
    VoiceCommercialEdgeEvent {
        content_hash: fnv1a(&buf),
        is_speech,
        energy,
        priority,
    }
}

// ── Shared ──────────────────────────────────────────────────────────────

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Zero-copy cast from `&[f32]` to `&[u8]` for hashing.
#[cfg(feature = "voice-commercial")]
#[inline(always)]
fn bytemuck_f32_to_u8(data: &[f32]) -> &[u8] {
    // SAFETY: f32 has no alignment/validity constraints beyond those of u8.
    // The resulting slice covers exactly `len * 4` contiguous bytes.
    unsafe { core::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 4) }
}

#[cfg(test)]
#[cfg(feature = "voice-commercial")]
mod tests {
    use super::*;

    #[test]
    fn vad_telemetry_hash_nonzero() {
        let mut detector = VadDetector::with_defaults();
        let silence = vec![0.0f32; 160];
        let t = voice_commercial_to_analytics_vad(&mut detector, &silence);
        assert_ne!(t.content_hash, 0);
    }

    #[test]
    fn vad_telemetry_silence() {
        let mut detector = VadDetector::with_defaults();
        let silence = vec![0.0f32; 160];
        let t = voice_commercial_to_analytics_vad(&mut detector, &silence);
        assert!(!t.is_speech);
        assert!(t.energy < 1e-6);
    }

    #[test]
    fn ml_features_hash_nonzero() {
        let frame: Vec<f32> = (0..160).map(|i| (i as f32 * 0.01).sin()).collect();
        let f = voice_commercial_to_ml_features(&frame, 0.0);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.dim, 4);
    }

    #[test]
    fn ml_features_energy_positive() {
        let frame: Vec<f32> = (0..160).map(|i| (i as f32 * 0.01).sin()).collect();
        let f = voice_commercial_to_ml_features(&frame, 0.0);
        assert!(f.features[0] > 0.0);
    }

    #[test]
    fn voice_config_hash_nonzero() {
        let config = voice_commercial_to_voice_config();
        assert_ne!(config.content_hash, 0);
        assert!(config.energy_threshold > 0.0);
    }

    #[test]
    fn voice_config_values() {
        let config = voice_commercial_to_voice_config();
        assert!(config.hangover_frames > 0);
        assert!(config.zcr_low < config.zcr_high);
    }

    #[test]
    fn cache_speech_ttl() {
        let entry = voice_commercial_to_cache_state(true);
        assert_eq!(entry.ttl_secs, 10);
        assert!(entry.is_speech);
    }

    #[test]
    fn cache_silence_ttl() {
        let entry = voice_commercial_to_cache_state(false);
        assert_eq!(entry.ttl_secs, 2);
        assert!(!entry.is_speech);
    }

    #[test]
    fn edge_event_speech() {
        let event = voice_commercial_to_edge_event(true, 0.5);
        assert!(event.is_speech);
        assert_eq!(event.priority, 5);
        assert_ne!(event.content_hash, 0);
    }

    #[test]
    fn edge_event_silence() {
        let event = voice_commercial_to_edge_event(false, 0.01);
        assert!(!event.is_speech);
        assert_eq!(event.priority, 1);
    }
}
