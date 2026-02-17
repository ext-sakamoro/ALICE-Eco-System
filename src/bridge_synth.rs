//! Synth bridges — ALICE-Synth ↔ Streaming-Protocol, Animation, Codec, DB, View, Cache, CDN, Sync, Crypto, Queue, Analytics
//!
//! 11 bridges connecting procedural audio to the ALICE ecosystem.

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
#[inline]
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
#[inline]
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
#[inline]
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
#[inline]
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
#[inline]
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
            lo = lo.min(s);
            hi = hi.max(s);
            sum_sq += s * s;
        }
        peaks.push((lo, hi));
    }

    let rms = if pcm.is_empty() { 0.0 } else { (sum_sq * (1.0 / pcm.len() as f32)).sqrt() };
    WaveformView { peaks, duration_secs: duration, rms }
}

// ── Bridge 6: Synth → Cache (Score content caching) ─────────────────────

/// Score cache entry for ALICE-Cache.
pub struct SynthCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Score bytes for cache value.
    pub data: Vec<u8>,
    /// Duration in seconds (for eviction priority).
    pub duration_secs: f32,
    /// Event count.
    pub event_count: usize,
}

/// Prepare Score for ALICE-Cache storage.
#[inline]
pub fn synth_to_cache_entry(score: &Score) -> SynthCacheEntry {
    let data = score.to_bytes();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    SynthCacheEntry {
        content_hash: hash,
        data,
        duration_secs: score.duration_secs(),
        event_count: score.events.len(),
    }
}

// ── Bridge 7: Synth → CDN (audio content distribution) ──────────────────

/// Audio content package for ALICE-CDN delivery.
pub struct SynthCdnPackage {
    /// Score bytes (compact wire format).
    pub score_bytes: Vec<u8>,
    /// Content hash for CDN routing.
    pub content_hash: u64,
    /// MIME type hint.
    pub content_type: &'static str,
    /// Duration in seconds.
    pub duration_secs: f32,
}

/// Package Score for ALICE-CDN content delivery.
#[inline]
pub fn synth_to_cdn_package(score: &Score) -> SynthCdnPackage {
    let data = score.to_bytes();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    SynthCdnPackage {
        score_bytes: data,
        content_hash: hash,
        content_type: "application/x-alice-score",
        duration_secs: score.duration_secs(),
    }
}

// ── Bridge 8: Synth → Sync (multiplayer music sync) ─────────────────────

/// Score sync packet for ALICE-Sync multiplayer music.
pub struct SynthSyncPacket {
    /// Score bytes.
    pub score_bytes: Vec<u8>,
    /// Tempo BPM for synchronization.
    pub tempo_bpm: u16,
    /// Current playback position in ticks.
    pub position_tick: u32,
    /// Event count.
    pub event_count: usize,
}

/// Prepare Score for ALICE-Sync multiplayer synchronization.
#[inline]
pub fn synth_to_sync_packet(score: &Score, position_tick: u32) -> SynthSyncPacket {
    SynthSyncPacket {
        score_bytes: score.to_bytes(),
        tempo_bpm: score.header.tempo_bpm,
        position_tick,
        event_count: score.events.len(),
    }
}

// ── Bridge 9: Synth → Crypto (encrypted audio payload) ──────────────────

/// Encrypted audio payload for secure transport.
pub struct SynthCryptoPayload {
    /// Score bytes (to be encrypted).
    pub plaintext: Vec<u8>,
    /// Content hash for integrity verification.
    pub content_hash: u64,
    /// Payload size.
    pub payload_bytes: usize,
}

/// Prepare Score bytes for ALICE-Crypto encryption.
#[inline]
pub fn synth_to_crypto_payload(score: &Score) -> SynthCryptoPayload {
    let data = score.to_bytes();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let len = data.len();
    SynthCryptoPayload {
        plaintext: data,
        content_hash: hash,
        payload_bytes: len,
    }
}

// ── Bridge 10: Synth → Queue (score delivery via message queue) ─────────

/// Score message for ALICE-Queue delivery.
pub struct SynthQueueMessage {
    /// Score bytes (compact wire format).
    pub score_bytes: Vec<u8>,
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Event count.
    pub event_count: usize,
    /// Tempo BPM.
    pub tempo_bpm: u16,
}

/// Package Score for ALICE-Queue message delivery.
#[inline]
pub fn synth_to_queue_message(score: &Score) -> SynthQueueMessage {
    let data = score.to_bytes();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    SynthQueueMessage {
        score_bytes: data,
        content_hash: hash,
        event_count: score.events.len(),
        tempo_bpm: score.header.tempo_bpm,
    }
}

// ── Bridge 11: Synth → Analytics (audio metrics) ────────────────────────

/// Audio metrics for ALICE-Analytics monitoring.
pub struct SynthAnalyticsMetrics {
    /// Score duration in seconds.
    pub duration_secs: f32,
    /// Event count.
    pub event_count: usize,
    /// Tempo BPM.
    pub tempo_bpm: u16,
    /// Events per second.
    pub events_per_sec: f32,
    /// Score bytes (wire size).
    pub score_bytes: usize,
}

/// Extract audio metrics for ALICE-Analytics monitoring.
#[inline]
pub fn synth_to_analytics_metrics(score: &Score) -> SynthAnalyticsMetrics {
    let duration = score.duration_secs();
    let events = score.events.len();
    let eps = if duration > 0.0 { events as f32 / duration } else { 0.0 };
    SynthAnalyticsMetrics {
        duration_secs: duration,
        event_count: events,
        tempo_bpm: score.header.tempo_bpm,
        events_per_sec: eps,
        score_bytes: score.to_bytes().len(),
    }
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

    #[test]
    fn test_synth_to_cache_entry() {
        let score = test_score();
        let entry = synth_to_cache_entry(&score);
        assert_ne!(entry.content_hash, 0);
        assert!(!entry.data.is_empty());
        assert_eq!(entry.event_count, 4);
    }

    #[test]
    fn test_synth_to_cdn_package() {
        let score = test_score();
        let pkg = synth_to_cdn_package(&score);
        assert!(!pkg.score_bytes.is_empty());
        assert_eq!(pkg.content_type, "application/x-alice-score");
    }

    #[test]
    fn test_synth_to_sync_packet() {
        let score = test_score();
        let pkt = synth_to_sync_packet(&score, 96);
        assert_eq!(pkt.tempo_bpm, 120);
        assert_eq!(pkt.position_tick, 96);
    }

    #[test]
    fn test_synth_to_crypto_payload() {
        let score = test_score();
        let crypto = synth_to_crypto_payload(&score);
        assert!(crypto.payload_bytes > 0);
        assert_ne!(crypto.content_hash, 0);
    }

    #[test]
    fn test_synth_to_queue_message() {
        let score = test_score();
        let msg = synth_to_queue_message(&score);
        assert!(!msg.score_bytes.is_empty());
        assert_ne!(msg.content_hash, 0);
        assert_eq!(msg.event_count, 4);
        assert_eq!(msg.tempo_bpm, 120);
    }

    #[test]
    fn test_synth_to_analytics_metrics() {
        let score = test_score();
        let m = synth_to_analytics_metrics(&score);
        assert!(m.duration_secs > 0.0);
        assert_eq!(m.event_count, 4);
        assert_eq!(m.tempo_bpm, 120);
        assert!(m.events_per_sec > 0.0);
        assert!(m.score_bytes > 0);
    }
}
