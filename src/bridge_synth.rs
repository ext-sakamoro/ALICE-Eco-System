//! Synth bridges — ALICE-Synth ↔ Streaming-Protocol, Animation, Codec, DB, View
//!
//! 5 bridges connecting procedural audio to the ALICE ecosystem.

use alice_synth::{
    FmPatch, NoteEventKind, Patch, Score, Synthesizer,
};

// ── Bridge 1: Synth → Streaming-Protocol (Score → ASP audio frames) ────

/// ASP audio frame generated from ALICE-Synth.
pub struct SynthAspFrame {
    /// PCM i16 samples.
    pub pcm_i16: Vec<i16>,
    /// Sample rate (Hz).
    pub sample_rate: u32,
    /// Frame duration in seconds.
    pub duration_secs: f32,
    /// Score bytes for transport (compact wire format).
    pub score_bytes: Vec<u8>,
}

/// Render Score to PCM and package for ALICE-Streaming-Protocol (ASP).
pub fn synth_to_asp_frame(score: &Score, sample_rate: u32) -> SynthAspFrame {
    let duration = score.duration_secs();
    let num_samples = (duration * sample_rate as f32) as usize;
    let mut synth = Synthesizer::new(sample_rate);
    synth.load_patch(0, Patch::Fm(FmPatch::electric_piano()));
    synth.load_score(score);
    let mut pcm = vec![0i16; num_samples.max(1)];
    synth.render_i16(&mut pcm);
    SynthAspFrame {
        pcm_i16: pcm,
        sample_rate,
        duration_secs: duration,
        score_bytes: score.to_bytes(),
    }
}

// ── Bridge 2: Synth → Animation (BGM / SFX timing) ─────────────────────

/// Audio event timeline for ALICE-Animation lip-sync / SFX cues.
pub struct AnimAudioCue {
    /// Time offset in seconds.
    pub time_secs: f32,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// MIDI note number (pitch indicator).
    pub note: u8,
    /// Velocity (0-127, intensity).
    pub velocity: u8,
    /// Channel / track.
    pub channel: u8,
}

/// Extract audio cue timeline from Score for ALICE-Animation.
pub fn synth_to_animation_cues(score: &Score) -> Vec<AnimAudioCue> {
    let tempo_bpm = score.header.tempo_bpm as f32;
    let ticks_per_beat = 96.0f32;
    let secs_per_tick = 60.0 / (tempo_bpm * ticks_per_beat);
    let mut cues = Vec::new();
    let mut note_on_times: Vec<(u8, u8, f32)> = Vec::new(); // (channel, note, time)
    let mut abs_tick: u32 = 0;

    for evt in &score.events {
        abs_tick += evt.delta_tick as u32;
        let t = abs_tick as f32 * secs_per_tick;
        match evt.kind {
            NoteEventKind::NoteOn => {
                note_on_times.push((evt.channel, evt.note, t));
            }
            NoteEventKind::NoteOff => {
                if let Some(pos) = note_on_times
                    .iter()
                    .position(|&(c, n, _)| c == evt.channel && n == evt.note)
                {
                    let (ch, note, start) = note_on_times.remove(pos);
                    cues.push(AnimAudioCue {
                        time_secs: start,
                        duration_secs: t - start,
                        note,
                        velocity: evt.velocity,
                        channel: ch,
                    });
                }
            }
            _ => {}
        }
    }
    // Close any remaining notes
    let end = score.duration_secs();
    for (ch, note, start) in note_on_times {
        cues.push(AnimAudioCue {
            time_secs: start,
            duration_secs: end - start,
            note,
            velocity: 100,
            channel: ch,
        });
    }
    cues
}

// ── Bridge 3: Synth → Codec (PCM → wavelet input) ──────────────────────

/// Audio payload ready for ALICE-Codec wavelet compression.
pub struct SynthCodecPayload {
    /// PCM f32 samples (mono).
    pub pcm_f32: Vec<f32>,
    /// Sample rate.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u8,
    /// Original Score size in bytes.
    pub score_size: usize,
}

/// Render Score to f32 PCM for ALICE-Codec wavelet compression.
pub fn synth_to_codec_payload(score: &Score, sample_rate: u32) -> SynthCodecPayload {
    let duration = score.duration_secs();
    let num_samples = (duration * sample_rate as f32) as usize;
    let mut synth = Synthesizer::new(sample_rate);
    synth.load_patch(0, Patch::Fm(FmPatch::electric_piano()));
    synth.load_score(score);
    let mut pcm = vec![0.0f32; num_samples.max(1)];
    synth.render(&mut pcm);
    SynthCodecPayload {
        pcm_f32: pcm,
        sample_rate,
        channels: 1,
        score_size: score.to_bytes().len(),
    }
}

// ── Bridge 4: Synth → DB (Score / Patch persistence) ────────────────────

/// Serialized Score record for ALICE-DB storage.
pub struct ScoreDbRecord {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Serialized Score bytes.
    pub data: Vec<u8>,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// Number of events.
    pub event_count: usize,
    /// Tempo BPM.
    pub tempo_bpm: u16,
}

/// Serialize Score for ALICE-DB persistence.
pub fn synth_to_db_record(score: &Score) -> ScoreDbRecord {
    let data = score.to_bytes();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    ScoreDbRecord {
        content_hash: hash,
        data,
        duration_secs: score.duration_secs(),
        event_count: score.events.len(),
        tempo_bpm: score.header.tempo_bpm,
    }
}

// ── Bridge 5: Synth → View (waveform visualization) ─────────────────────

/// Waveform data for ALICE-View visualization.
pub struct WaveformView {
    /// Downsampled waveform peaks (min, max) per column.
    pub peaks: Vec<(f32, f32)>,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// RMS level.
    pub rms: f32,
}

/// Render Score to downsampled waveform for ALICE-View display.
pub fn synth_to_view_waveform(score: &Score, sample_rate: u32, columns: usize) -> WaveformView {
    let duration = score.duration_secs();
    let num_samples = (duration * sample_rate as f32) as usize;
    let mut synth = Synthesizer::new(sample_rate);
    synth.load_patch(0, Patch::Fm(FmPatch::electric_piano()));
    synth.load_score(score);
    let mut pcm = vec![0.0f32; num_samples.max(1)];
    synth.render(&mut pcm);

    let cols = columns.max(1);
    let samples_per_col = pcm.len() / cols;
    let mut peaks = Vec::with_capacity(cols);
    let mut sum_sq = 0.0f32;

    for c in 0..cols {
        let start = c * samples_per_col;
        let end = ((c + 1) * samples_per_col).min(pcm.len());
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for &s in &pcm[start..end] {
            if s < lo { lo = s; }
            if s > hi { hi = s; }
            sum_sq += s * s;
        }
        peaks.push((lo, hi));
    }

    let rms = if pcm.is_empty() { 0.0 } else { (sum_sq / pcm.len() as f32).sqrt() };
    WaveformView { peaks, duration_secs: duration, rms }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_synth::NoteEvent;

    fn test_score() -> Score {
        let mut s = Score::new(120, 1);
        s.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 60, velocity: 100, kind: NoteEventKind::NoteOn });
        s.add_event(NoteEvent { delta_tick: 96, channel: 0, note: 60, velocity: 0, kind: NoteEventKind::NoteOff });
        s.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 64, velocity: 100, kind: NoteEventKind::NoteOn });
        s.add_event(NoteEvent { delta_tick: 96, channel: 0, note: 64, velocity: 0, kind: NoteEventKind::NoteOff });
        s
    }

    #[test]
    fn test_synth_to_asp_frame() {
        let score = test_score();
        let frame = synth_to_asp_frame(&score, 44100);
        assert!(!frame.pcm_i16.is_empty());
        assert_eq!(frame.sample_rate, 44100);
        assert!(!frame.score_bytes.is_empty());
    }

    #[test]
    fn test_synth_to_animation_cues() {
        let score = test_score();
        let cues = synth_to_animation_cues(&score);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].note, 60);
        assert_eq!(cues[1].note, 64);
        assert!(cues[0].duration_secs > 0.0);
    }

    #[test]
    fn test_synth_to_codec_payload() {
        let score = test_score();
        let payload = synth_to_codec_payload(&score, 22050);
        assert!(!payload.pcm_f32.is_empty());
        assert_eq!(payload.channels, 1);
        assert!(payload.score_size > 0);
    }

    #[test]
    fn test_synth_to_db_record() {
        let score = test_score();
        let rec = synth_to_db_record(&score);
        assert!(!rec.data.is_empty());
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.event_count, 4);
        assert_eq!(rec.tempo_bpm, 120);
    }

    #[test]
    fn test_synth_to_view_waveform() {
        let score = test_score();
        let wf = synth_to_view_waveform(&score, 44100, 100);
        assert_eq!(wf.peaks.len(), 100);
        assert!(wf.duration_secs > 0.0);
        assert!(wf.rms >= 0.0);
    }
}
