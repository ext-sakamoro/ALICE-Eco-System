//! Signal bridges — ALICE-Signal ↔ DB, Cache, Analytics, ML, Audio
//!
//! 5 bridges connecting DSP / signal processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Signal → DB (signal log) ───────────────────────────────────

/// Signal capture log record for ALICE-DB persistence.
pub struct SignalDbRecord {
    /// Content hash over the capture fields.
    pub content_hash: u64,
    /// Total number of samples captured.
    pub sample_count: u64,
    /// Sample rate in Hz.
    pub sample_rate_hz: u32,
    /// FFT window size in samples.
    pub fft_size: u32,
    /// Number of frequency bins.
    pub frequency_bins: u32,
    /// RMS signal level (linear, full-scale normalised to 1.0).
    pub rms_level: f32,
    /// Signal-to-noise ratio in dB.
    pub snr_db: f32,
    /// Capture start timestamp in nanoseconds (Unix epoch).
    pub capture_start_ns: u64,
}

/// Serialize signal capture for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn signal_to_db_record(
    sample_count: u64,
    sample_rate_hz: u32,
    fft_size: u32,
    frequency_bins: u32,
    rms_level: f32,
    snr_db: f32,
    capture_start_ns: u64,
) -> SignalDbRecord {
    let mut key = [0u8; 28];
    key[0..8].copy_from_slice(&sample_count.to_le_bytes());
    key[8..12].copy_from_slice(&sample_rate_hz.to_le_bytes());
    key[12..16].copy_from_slice(&fft_size.to_le_bytes());
    key[16..20].copy_from_slice(&rms_level.to_le_bytes());
    key[20..28].copy_from_slice(&capture_start_ns.to_le_bytes());
    SignalDbRecord {
        content_hash: fnv1a(&key),
        sample_count,
        sample_rate_hz,
        fft_size,
        frequency_bins,
        rms_level,
        snr_db,
        capture_start_ns,
    }
}

// ── Bridge 2: Signal → Cache (FFT cache) ─────────────────────────────────

/// FFT result cache entry for ALICE-Cache.
pub struct SignalCacheEntry {
    /// Content hash for cache key derivation.
    pub content_hash: u64,
    /// FFT window size in samples.
    pub fft_size: u32,
    /// Sample rate in Hz.
    pub sample_rate_hz: u32,
    /// Frequency resolution in Hz per bin.
    pub bin_resolution_hz: f32,
    /// Peak frequency bin index.
    pub peak_bin: u32,
    /// Peak magnitude (linear).
    pub peak_magnitude: f32,
    /// TTL in seconds (branchless: shorter when SNR is low).
    pub ttl_secs: u32,
}

/// Cache FFT result for ALICE-Cache.
#[inline]
#[must_use]
pub fn signal_to_cache_entry(
    fft_size: u32,
    sample_rate_hz: u32,
    peak_bin: u32,
    peak_magnitude: f32,
    snr_db: f32,
) -> SignalCacheEntry {
    let rcp_fft = 1.0 / fft_size.max(1) as f32;
    let bin_resolution_hz = sample_rate_hz as f32 * rcp_fft;

    // Branchless TTL: 60 s normally, 15 s when SNR < 10 dB (noisy signal).
    let low_snr = (snr_db < 10.0) as u32;
    let ttl_secs = 60_u32 - low_snr * 45;

    let mut key = [0u8; 12];
    key[0..4].copy_from_slice(&fft_size.to_le_bytes());
    key[4..8].copy_from_slice(&sample_rate_hz.to_le_bytes());
    key[8..12].copy_from_slice(&peak_bin.to_le_bytes());
    SignalCacheEntry {
        content_hash: fnv1a(&key),
        fft_size,
        sample_rate_hz,
        bin_resolution_hz,
        peak_bin,
        peak_magnitude,
        ttl_secs,
    }
}

// ── Bridge 3: Signal → Analytics (spectral metrics) ──────────────────────

/// Spectral metrics for ALICE-Analytics ingestion.
pub struct SignalAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total samples processed in the reporting period.
    pub sample_count: u64,
    /// Average RMS level across all frames.
    pub avg_rms: f32,
    /// Average SNR in dB across all frames.
    pub avg_snr_db: f32,
    /// Peak SNR observed in dB.
    pub peak_snr_db: f32,
    /// Number of FFT frames processed.
    pub frame_count: u64,
    /// Dominant frequency in Hz (bin with highest average magnitude).
    pub dominant_freq_hz: f32,
}

/// Build spectral metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn signal_to_analytics_metrics(
    sample_count: u64,
    total_rms: f32,
    total_snr_db: f32,
    peak_snr_db: f32,
    frame_count: u64,
    dominant_freq_hz: f32,
) -> SignalAnalyticsMetrics {
    let rcp_frames = 1.0 / frame_count.max(1) as f32;
    let avg_rms = total_rms * rcp_frames;
    let avg_snr_db = total_snr_db * rcp_frames;

    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&sample_count.to_le_bytes());
    key[8..16].copy_from_slice(&frame_count.to_le_bytes());
    SignalAnalyticsMetrics {
        content_hash: fnv1a(&key),
        sample_count,
        avg_rms,
        avg_snr_db,
        peak_snr_db,
        frame_count,
        dominant_freq_hz,
    }
}

// ── Bridge 4: Signal → ML (feature extraction) ────────────────────────────

/// Spectral feature vector for ALICE-ML training and inference.
pub struct SignalMlFeatures {
    /// Content hash for feature deduplication.
    pub content_hash: u64,
    /// RMS energy level (linear).
    pub rms_level: f32,
    /// SNR in dB.
    pub snr_db: f32,
    /// Spectral centroid frequency in Hz.
    pub spectral_centroid_hz: f32,
    /// Spectral bandwidth (standard deviation around centroid) in Hz.
    pub spectral_bandwidth_hz: f32,
    /// Zero-crossing rate (crossings per sample).
    pub zero_crossing_rate: f32,
    /// Number of frequency bins used for feature computation.
    pub frequency_bins: u32,
}

/// Extract spectral features for ALICE-ML.
#[inline]
#[must_use]
pub fn signal_to_ml_features(
    rms_level: f32,
    snr_db: f32,
    spectral_centroid_hz: f32,
    spectral_bandwidth_hz: f32,
    zero_crossing_rate: f32,
    frequency_bins: u32,
) -> SignalMlFeatures {
    let mut key = [0u8; 20];
    key[0..4].copy_from_slice(&rms_level.to_le_bytes());
    key[4..8].copy_from_slice(&snr_db.to_le_bytes());
    key[8..12].copy_from_slice(&spectral_centroid_hz.to_le_bytes());
    key[12..16].copy_from_slice(&spectral_bandwidth_hz.to_le_bytes());
    key[16..20].copy_from_slice(&frequency_bins.to_le_bytes());
    SignalMlFeatures {
        content_hash: fnv1a(&key),
        rms_level,
        snr_db,
        spectral_centroid_hz,
        spectral_bandwidth_hz,
        zero_crossing_rate,
        frequency_bins,
    }
}

// ── Bridge 5: Signal → Audio (DSP pipeline) ───────────────────────────────

/// DSP pipeline configuration for ALICE-Audio integration.
pub struct SignalAudioPipeline {
    /// Content hash over the pipeline configuration.
    pub content_hash: u64,
    /// Sample rate in Hz.
    pub sample_rate_hz: u32,
    /// FFT size in samples.
    pub fft_size: u32,
    /// Number of frequency bins.
    pub frequency_bins: u32,
    /// Hop size between successive FFT frames in samples.
    pub hop_size: u32,
    /// Pre-emphasis coefficient (applied before windowing).
    pub pre_emphasis: f32,
    /// Window type: 0 = rectangular, 1 = Hann, 2 = Hamming, 3 = Blackman.
    pub window_type: u8,
}

/// Build DSP pipeline configuration for ALICE-Audio.
#[inline]
#[must_use]
pub fn signal_to_audio_pipeline(
    sample_rate_hz: u32,
    fft_size: u32,
    hop_size: u32,
    pre_emphasis: f32,
    window_type: u8,
) -> SignalAudioPipeline {
    // frequency_bins = fft_size / 2 + 1 (real FFT output).
    let frequency_bins = fft_size / 2 + 1;

    let mut key = [0u8; 13];
    key[0..4].copy_from_slice(&sample_rate_hz.to_le_bytes());
    key[4..8].copy_from_slice(&fft_size.to_le_bytes());
    key[8..12].copy_from_slice(&hop_size.to_le_bytes());
    key[12] = window_type;
    SignalAudioPipeline {
        content_hash: fnv1a(&key),
        sample_rate_hz,
        fft_size,
        frequency_bins,
        hop_size,
        pre_emphasis,
        window_type,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_to_db_record_hash_nonzero() {
        let rec = signal_to_db_record(
            44_100,
            44_100,
            1024,
            513,
            0.25,
            30.0,
            1_700_000_000_000_000_000,
        );
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.sample_rate_hz, 44_100);
        assert_eq!(rec.fft_size, 1024);
    }

    #[test]
    fn test_signal_to_db_record_deterministic() {
        let a = signal_to_db_record(48_000, 48_000, 2048, 1025, 0.1, 20.0, 0);
        let b = signal_to_db_record(48_000, 48_000, 2048, 1025, 0.1, 20.0, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_signal_to_cache_entry_good_snr_ttl() {
        let entry = signal_to_cache_entry(1024, 44_100, 256, 0.8, 25.0);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 60); // SNR >= 10 dB
                                        // bin_resolution = 44100 / 1024 ≈ 43.07 Hz
        assert!((entry.bin_resolution_hz - 43.066_4).abs() < 0.1);
    }

    #[test]
    fn test_signal_to_cache_entry_low_snr_ttl() {
        let entry = signal_to_cache_entry(1024, 44_100, 10, 0.05, 5.0);
        assert_eq!(entry.ttl_secs, 15); // SNR < 10 dB → short TTL
    }

    #[test]
    fn test_signal_to_analytics_metrics_averages() {
        let m = signal_to_analytics_metrics(441_000, 25.0, 300.0, 42.0, 10, 1_000.0);
        assert_ne!(m.content_hash, 0);
        assert!((m.avg_rms - 2.5).abs() < 0.001);
        assert!((m.avg_snr_db - 30.0).abs() < 0.001);
        assert_eq!(m.frame_count, 10);
    }

    #[test]
    fn test_signal_to_analytics_metrics_zero_frames() {
        let m = signal_to_analytics_metrics(0, 0.0, 0.0, 0.0, 0, 0.0);
        assert_eq!(m.frame_count, 0);
        assert_eq!(m.avg_rms, 0.0);
    }

    #[test]
    fn test_signal_to_ml_features() {
        let f = signal_to_ml_features(0.3, 22.5, 440.0, 150.0, 0.05, 513);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.frequency_bins, 513);
        assert!((f.spectral_centroid_hz - 440.0).abs() < 0.001);
    }

    #[test]
    fn test_signal_to_audio_pipeline_bins() {
        let p = signal_to_audio_pipeline(44_100, 2048, 512, 0.97, 1);
        assert_ne!(p.content_hash, 0);
        // frequency_bins = 2048/2 + 1 = 1025
        assert_eq!(p.frequency_bins, 1025);
        assert_eq!(p.window_type, 1);
        assert_eq!(p.hop_size, 512);
    }
}
