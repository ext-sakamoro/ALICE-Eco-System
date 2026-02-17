//! Voice bridges — ALICE-Voice ↔ Synth, Animation, Font, Edge
//!
//! 4 bridges connecting parametric voice codec to the ALICE ecosystem.

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

/// Convert ParametricParams to FM synthesis parameters for ALICE-Synth.
pub fn voice_to_synth_params(params: &ParametricParams) -> VoiceSynthParams {
    let pitch = params.pitch.f0;
    let voiced = params.pitch.is_voiced;
    let f1 = if !params.formants.is_empty() { params.formants[0].frequency } else { 500.0 };
    let f2 = if params.formants.len() > 1 { params.formants[1].frequency } else { 1500.0 };
    let f3 = if params.formants.len() > 2 { params.formants[2].frequency } else { 2500.0 };
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

/// Convert ParametricParams to lip sync cue for ALICE-Animation.
pub fn voice_to_animation_lipsync(params: &ParametricParams) -> VoiceLipSyncCue {
    let f1 = if !params.formants.is_empty() { params.formants[0].frequency } else { 500.0 };
    let f2 = if params.formants.len() > 1 { params.formants[1].frequency } else { 1500.0 };
    let f3 = if params.formants.len() > 2 { params.formants[2].frequency } else { 2500.0 };
    VoiceLipSyncCue {
        mouth_open: (f1 / 1000.0).min(1.0),
        mouth_width: ((f2 - 800.0) / 1700.0).clamp(0.0, 1.0),
        tongue_pos: ((f3 - 1500.0) / 2000.0).clamp(0.0, 1.0),
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
pub fn voice_to_font_overlay(params: &ParametricParams) -> VoiceTextOverlay {
    let emphasis = (params.lpc.gain * 2.0).min(1.0);
    let pitch = params.pitch.f0;
    let pitch_trend = if pitch > 250.0 { 1i8 } else if pitch < 100.0 { -1 } else { 0 };
    VoiceTextOverlay {
        speaking_rate: 4.0,
        pitch_trend,
        emphasis,
        font_weight_scale: 1.0 + emphasis * 0.3,
    }
}

// ── Bridge 4: Voice → Edge (LPC → compressed IoT payload) ───────────────

/// Compressed voice payload for ALICE-Edge IoT streaming.
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

/// Package voice parameters for ALICE-Edge IoT transport.
pub fn voice_to_edge_payload(params: &ParametricParams, raw_pcm_bytes: usize) -> VoiceEdgePayload {
    let lpc_order = params.lpc.coeffs.len().min(255) as u8;
    let pitch_period = if params.pitch.f0 > 0.0 { (16000.0 / params.pitch.f0) as u16 } else { 0 };
    let payload_bytes = 4 + (lpc_order as usize) * 4 + 2 + 4; // gain + coeffs + pitch + header
    VoiceEdgePayload {
        lpc_order,
        pitch_period,
        gain: params.lpc.gain,
        payload_bytes,
        compression_ratio: if payload_bytes > 0 { raw_pcm_bytes as f32 / payload_bytes as f32 } else { 0.0 },
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
                Formant { frequency: 700.0, bandwidth: 80.0, amplitude: 1.0 },
                Formant { frequency: 1200.0, bandwidth: 90.0, amplitude: 0.8 },
                Formant { frequency: 2600.0, bandwidth: 120.0, amplitude: 0.5 },
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
}
