//! Voice bridges — ALICE-Voice ↔ Synth, Animation, Font, Edge, DB, Cache
//!
//! 6 bridges connecting parametric voice codec to the ALICE ecosystem.

use alice_voice::ParametricParams;

// ── Bridge 1: Voice → Synth (parametric → FM synthesis) ─────────────────

/// Voice parameters mapped to FM synthesis inputs.
pub struct VoiceSynthParams {
    /// Fundamental frequency (Hz) for FM carrier.
    pub carrier_freq: f32,
    /// Formant frequencies (F1-F3) for FM modulator ratios.
    pub formant_ratios: [f32; 3],
    /// Amplitude envelope from LPC gain.
    pub amplitude: f32,
    /// Voiced/unvoiced flag.
    pub voiced: bool,
}

/// Convert `ParametricParams` to FM synthesis parameters for ALICE-Synth.
#[inline]
#[must_use]
pub fn voice_to_synth_params(params: &ParametricParams) -> VoiceSynthParams {
    let pitch = params.pitch.f0;
    let voiced = params.pitch.is_voiced;
    let f1 = params.formants.first().map_or(500.0, |f| f.frequency);
    let f2 = params.formants.get(1).map_or(1500.0, |f| f.frequency);
    let f3 = params.formants.get(2).map_or(2500.0, |f| f.frequency);
    let base = if pitch > 0.0 { pitch } else { 200.0 };
    VoiceSynthParams {
        carrier_freq: pitch,
        formant_ratios: [f1 / base, f2 / base, f3 / base],
        amplitude: params.lpc.gain,
        voiced,
    }
}

// ── Bridge 2: Voice → Animation (formants → lip sync) ───────────────────

/// Lip sync cue for ALICE-Animation.
pub struct VoiceLipSyncCue {
    /// Mouth openness (0.0-1.0) derived from F1.
    pub mouth_open: f32,
    /// Mouth width (0.0-1.0) derived from F2.
    pub mouth_width: f32,
    /// Tongue position (0.0-1.0) derived from F3.
    pub tongue_pos: f32,
    /// Pitch for expression intensity.
    pub pitch_hz: f32,
    /// Voiced flag.
    pub voiced: bool,
}

/// Convert `ParametricParams` to lip sync cue for ALICE-Animation.
#[inline]
#[must_use]
pub fn voice_to_animation_lipsync(params: &ParametricParams) -> VoiceLipSyncCue {
    let f1 = params.formants.first().map_or(500.0, |f| f.frequency);
    let f2 = params.formants.get(1).map_or(1500.0, |f| f.frequency);
    let f3 = params.formants.get(2).map_or(2500.0, |f| f.frequency);
    VoiceLipSyncCue {
        mouth_open: (f1 * 0.001).min(1.0),
        mouth_width: ((f2 - 800.0) * (1.0 / 1700.0)).clamp(0.0, 1.0),
        tongue_pos: ((f3 - 1500.0) * 0.0005).clamp(0.0, 1.0),
        pitch_hz: params.pitch.f0,
        voiced: params.pitch.is_voiced,
    }
}

// ── Bridge 3: Voice → Font (pitch → text overlay timing) ────────────────

/// Voice-driven text overlay for ALICE-Font subtitle rendering.
pub struct VoiceTextOverlay {
    /// Estimated speaking rate (syllables/sec).
    pub speaking_rate: f32,
    /// Pitch contour indicator (rising/falling/flat).
    pub pitch_trend: i8,
    /// Emphasis level (0.0-1.0) from amplitude.
    pub emphasis: f32,
    /// Recommended font weight scaling.
    pub font_weight_scale: f32,
}

/// Derive text overlay parameters from voice analysis for ALICE-Font.
#[inline]
#[must_use]
pub fn voice_to_font_overlay(params: &ParametricParams) -> VoiceTextOverlay {
    let emphasis = (params.lpc.gain * 2.0).min(1.0);
    let pitch = params.pitch.f0;
    let pitch_trend = if pitch > 250.0 {
        1i8
    } else if pitch < 100.0 {
        -1
    } else {
        0
    };
    VoiceTextOverlay {
        speaking_rate: 4.0,
        pitch_trend,
        emphasis,
        font_weight_scale: 1.0 + emphasis * 0.3,
    }
}

// ── Bridge 4: Voice → Edge (LPC → compressed IoT payload) ───────────────

/// Compressed voice payload for ALICE-Edge `IoT` streaming.
pub struct VoiceEdgePayload {
    /// LPC order (number of coefficients).
    pub lpc_order: u8,
    /// Pitch period in samples.
    pub pitch_period: u16,
    /// Gain (amplitude).
    pub gain: f32,
    /// Payload size in bytes.
    pub payload_bytes: usize,
    /// Compression ratio vs raw PCM.
    pub compression_ratio: f32,
}

/// Package voice parameters for ALICE-Edge `IoT` transport.
#[inline]
#[must_use]
pub fn voice_to_edge_payload(params: &ParametricParams, raw_pcm_bytes: usize) -> VoiceEdgePayload {
    let lpc_order = params.lpc.coeffs.len().min(255) as u8;
    let pitch_period = if params.pitch.f0 > 0.0 {
        (16000.0 / params.pitch.f0) as u16
    } else {
        0
    };
    let payload_bytes = 4 + (lpc_order as usize) * 4 + 2 + 4; // gain + coeffs + pitch + header
    VoiceEdgePayload {
        lpc_order,
        pitch_period,
        gain: params.lpc.gain,
        payload_bytes,
        compression_ratio: if payload_bytes > 0 {
            raw_pcm_bytes as f32 / payload_bytes as f32
        } else {
            0.0
        },
    }
}

// ── Bridge 5: Voice → DB (voice parameter persistence) ─────────────────

/// Voice parameter record for ALICE-DB persistence.
pub struct VoiceDbRecord {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Pitch frequency (Hz).
    pub pitch_hz: f32,
    /// LPC order.
    pub lpc_order: u8,
    /// Number of formants.
    pub formant_count: u8,
    /// Sample rate.
    pub sample_rate: u32,
}

/// Serialize `ParametricParams` for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn voice_to_db_record(params: &ParametricParams) -> VoiceDbRecord {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &params.pitch.f0.to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for &b in &params.lpc.gain.to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    VoiceDbRecord {
        content_hash: hash,
        pitch_hz: params.pitch.f0,
        lpc_order: params.lpc.coeffs.len().min(255) as u8,
        formant_count: params.formants.len().min(255) as u8,
        sample_rate: params.sample_rate,
    }
}

// ── Bridge 6: Voice → Cache (voice parameter caching) ──────────────────

/// Voice parameter cache entry for ALICE-Cache.
pub struct VoiceCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Pitch frequency (Hz).
    pub pitch_hz: f32,
    /// Voiced flag.
    pub voiced: bool,
    /// Payload size estimate.
    pub payload_bytes: usize,
}

/// Prepare `ParametricParams` for ALICE-Cache storage.
#[inline]
#[must_use]
pub fn voice_to_cache_entry(params: &ParametricParams) -> VoiceCacheEntry {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &params.pitch.f0.to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for &b in &(params.sample_rate as u64).to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let payload_bytes = 4 + params.lpc.coeffs.len() * 4 + params.formants.len() * 12 + 8;
    VoiceCacheEntry {
        content_hash: hash,
        pitch_hz: params.pitch.f0,
        voiced: params.pitch.is_voiced,
        payload_bytes,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_voice::{Formant, LpcCoefficients, PitchInfo};

    fn test_params() -> ParametricParams {
        ParametricParams {
            pitch: PitchInfo {
                f0: 220.0,
                period: 72.7,
                voicing_prob: 0.9,
                confidence: 0.95,
                is_voiced: true,
            },
            lpc: LpcCoefficients {
                coeffs: vec![0.5, -0.3, 0.1],
                gain: 0.6,
                reflection: vec![],
                error: 0.01,
            },
            formants: vec![
                Formant {
                    frequency: 700.0,
                    bandwidth: 80.0,
                    amplitude: 1.0,
                },
                Formant {
                    frequency: 1200.0,
                    bandwidth: 90.0,
                    amplitude: 0.8,
                },
                Formant {
                    frequency: 2600.0,
                    bandwidth: 120.0,
                    amplitude: 0.5,
                },
            ],
            activity: alice_voice::VoiceActivity {
                is_voiced: true,
                confidence: 0.95,
                energy_db: -20.0,
            },
            frame_size: 320,
            sample_rate: 16000,
        }
    }

    #[test]
    fn test_voice_to_synth_params() {
        let p = test_params();
        let s = voice_to_synth_params(&p);
        assert!((s.carrier_freq - 220.0).abs() < 0.01);
        assert!(s.voiced);
        assert!(s.formant_ratios[0] > 1.0);
    }

    #[test]
    fn test_voice_to_animation_lipsync() {
        let p = test_params();
        let lip = voice_to_animation_lipsync(&p);
        assert!(lip.mouth_open > 0.0 && lip.mouth_open <= 1.0);
        assert!(lip.mouth_width >= 0.0 && lip.mouth_width <= 1.0);
        assert!(lip.voiced);
    }

    #[test]
    fn test_voice_to_font_overlay() {
        let p = test_params();
        let o = voice_to_font_overlay(&p);
        assert!(o.emphasis > 0.0);
        assert!(o.font_weight_scale >= 1.0);
    }

    #[test]
    fn test_voice_to_edge_payload() {
        let p = test_params();
        let e = voice_to_edge_payload(&p, 32000);
        assert!(e.compression_ratio > 1.0);
        assert_eq!(e.lpc_order, 3);
        assert!(e.pitch_period > 0);
    }

    #[test]
    fn test_voice_to_db_record() {
        let p = test_params();
        let rec = voice_to_db_record(&p);
        assert_ne!(rec.content_hash, 0);
        assert!((rec.pitch_hz - 220.0).abs() < 0.01);
        assert_eq!(rec.lpc_order, 3);
        assert_eq!(rec.formant_count, 3);
    }

    #[test]
    fn test_voice_to_cache_entry() {
        let p = test_params();
        let entry = voice_to_cache_entry(&p);
        assert_ne!(entry.content_hash, 0);
        assert!(entry.voiced);
        assert!(entry.payload_bytes > 0);
    }
}
