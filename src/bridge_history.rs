//! History bridges — ALICE-History ↔ Analytics, DB, Cache
//!
//! 5 bridges connecting inverse entropy restoration to the ALICE ecosystem.

use alice_history::{Fragment, FragmentKind, RestorationResult, EntropyMeasurement};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Fragment → Analytics (degradation metrics) ─────────────

/// Fragment degradation metrics for ALICE-Analytics ingestion.
pub struct HistoryAnalyticsDegradationEvent {
    /// Content hash over fragment id, kind, known fraction, and missing count bytes.
    pub content_hash: u64,
    /// Fragment identifier.
    pub fragment_id: u64,
    /// Fragment kind discriminant: 0=Text, 1=Image, 2=Artifact, 3=Inscription, 4=Audio.
    pub kind: u8,
    /// Fraction of data that is known (0.0 to 1.0).
    pub known_fraction: f64,
    /// Number of missing elements.
    pub missing_count: usize,
    /// Total data length.
    pub data_length: usize,
}

/// Convert a fragment into a degradation analytics event.
#[inline]
pub fn history_fragment_to_analytics(fragment: &Fragment) -> HistoryAnalyticsDegradationEvent {
    let known_frac = fragment.known_fraction();
    let missing = fragment.missing_count();
    let kind_byte = fragment.kind as u8;

    let mut key = [0u8; 25];
    key[0..8].copy_from_slice(&fragment.id.to_le_bytes());
    key[8] = kind_byte;
    key[9..17].copy_from_slice(&known_frac.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&(missing as u64).to_le_bytes());

    HistoryAnalyticsDegradationEvent {
        content_hash: fnv1a(&key),
        fragment_id: fragment.id,
        kind: kind_byte,
        known_fraction: known_frac,
        missing_count: missing,
        data_length: fragment.data.len(),
    }
}

// ── Bridge 2: RestorationResult → Analytics (quality metrics) ────────

/// Restoration quality metrics for ALICE-Analytics ingestion.
pub struct HistoryAnalyticsQualityEvent {
    /// Content hash over fragment_id, entropy_before, entropy_after, iterations bytes.
    pub content_hash: u64,
    /// Fragment identifier.
    pub fragment_id: u64,
    /// Shannon entropy before restoration.
    pub entropy_before: f64,
    /// Shannon entropy after restoration.
    pub entropy_after: f64,
    /// Entropy reduction ratio (1 - after/before), clamped to [0, 1].
    pub entropy_reduction: f64,
    /// Number of solver iterations performed.
    pub iterations: u32,
    /// Mean confidence of the restored field.
    pub mean_confidence: f64,
}

/// Convert a restoration result into a quality analytics event.
#[inline]
pub fn history_restoration_to_analytics(result: &RestorationResult) -> HistoryAnalyticsQualityEvent {
    let eb = result.field.entropy_before;
    let ea = result.field.entropy_after;
    let reduction = if eb > 0.0 { (1.0 - ea / eb).max(0.0).min(1.0) } else { 0.0 };

    let mut key = [0u8; 28];
    key[0..8].copy_from_slice(&result.fragment_id.to_le_bytes());
    key[8..16].copy_from_slice(&eb.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&ea.to_bits().to_le_bytes());
    key[24..28].copy_from_slice(&result.field.iterations.to_le_bytes());

    HistoryAnalyticsQualityEvent {
        content_hash: fnv1a(&key),
        fragment_id: result.fragment_id,
        entropy_before: eb,
        entropy_after: ea,
        entropy_reduction: reduction,
        iterations: result.field.iterations,
        mean_confidence: result.field.confidence.mean_confidence,
    }
}

// ── Bridge 3: RestorationResult → DB (restoration record) ────────────

/// Restoration record for ALICE-DB persistence.
pub struct HistoryDbRestorationRecord {
    /// Content hash over fragment_id, iterations, elapsed_ns, field content_hash bytes.
    pub content_hash: u64,
    /// Fragment identifier.
    pub fragment_id: u64,
    /// Number of restored values.
    pub value_count: usize,
    /// Solver iterations.
    pub iterations: u32,
    /// Wall-clock elapsed nanoseconds.
    pub elapsed_ns: u64,
    /// Minimum confidence across all restored elements.
    pub min_confidence: f64,
    /// Mean confidence across all restored elements.
    pub mean_confidence: f64,
}

/// Convert a restoration result into a DB record.
#[inline]
pub fn history_restoration_to_db(result: &RestorationResult) -> HistoryDbRestorationRecord {
    let mut key = [0u8; 28];
    key[0..8].copy_from_slice(&result.fragment_id.to_le_bytes());
    key[8..12].copy_from_slice(&result.field.iterations.to_le_bytes());
    key[12..20].copy_from_slice(&result.elapsed_ns.to_le_bytes());
    key[20..28].copy_from_slice(&result.field.content_hash.to_le_bytes());

    HistoryDbRestorationRecord {
        content_hash: fnv1a(&key),
        fragment_id: result.fragment_id,
        value_count: result.field.values.len(),
        iterations: result.field.iterations,
        elapsed_ns: result.elapsed_ns,
        min_confidence: result.field.confidence.min_confidence,
        mean_confidence: result.field.confidence.mean_confidence,
    }
}

// ── Bridge 4: EntropyMeasurement → Analytics (entropy event) ─────────

/// Entropy measurement event for ALICE-Analytics ingestion.
pub struct HistoryAnalyticsEntropyEvent {
    /// Content hash over shannon_entropy, normalized_entropy, unique_symbols bytes.
    pub content_hash: u64,
    /// Shannon entropy (bits).
    pub shannon_entropy: f64,
    /// Normalized entropy (0.0 to 1.0).
    pub normalized_entropy: f64,
    /// Number of unique symbols (bins occupied).
    pub unique_symbols: usize,
    /// Total number of symbols (data length).
    pub total_symbols: usize,
}

/// Convert an entropy measurement into an analytics event.
#[inline]
pub fn history_entropy_to_analytics(measurement: &EntropyMeasurement) -> HistoryAnalyticsEntropyEvent {
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&measurement.shannon_entropy.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&measurement.normalized_entropy.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&(measurement.unique_symbols as u64).to_le_bytes());

    HistoryAnalyticsEntropyEvent {
        content_hash: fnv1a(&key),
        shannon_entropy: measurement.shannon_entropy,
        normalized_entropy: measurement.normalized_entropy,
        unique_symbols: measurement.unique_symbols,
        total_symbols: measurement.total_symbols,
    }
}

// ── Bridge 5: RestorationResult → Cache (quick lookup) ───────────────

/// Restoration cache entry for ALICE-Cache real-time lookup.
pub struct HistoryCacheRestoration {
    /// Content hash over fragment_id and field content_hash bytes.
    pub content_hash: u64,
    /// Fragment identifier.
    pub fragment_id: u64,
    /// Mean confidence of the restored field.
    pub mean_confidence: f64,
    /// Whether the restoration is high-quality (mean_confidence > 0.8).
    pub is_high_quality: bool,
    /// Cache TTL: 30s if low-quality (mean_confidence < 0.5), else 300s.
    pub ttl_secs: u32,
}

/// Convert a restoration result into a cache entry with adaptive TTL.
#[inline]
pub fn history_restoration_to_cache(result: &RestorationResult) -> HistoryCacheRestoration {
    let mean_conf = result.field.confidence.mean_confidence;

    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&result.fragment_id.to_le_bytes());
    key[8..16].copy_from_slice(&result.field.content_hash.to_le_bytes());

    // Branchless TTL: low_quality=1 → 300-270=30, high_quality=0 → 300-0=300.
    let low_quality = (mean_conf < 0.5) as u32;
    let ttl_secs = 300 - low_quality * 270;

    HistoryCacheRestoration {
        content_hash: fnv1a(&key),
        fragment_id: result.fragment_id,
        mean_confidence: mean_conf,
        is_high_quality: mean_conf > 0.8,
        ttl_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_history::{Fragment, FragmentKind, InversionConfig, restore, measure_entropy};

    fn make_fragment(id: u64, kind: FragmentKind) -> Fragment {
        Fragment::new(id, kind, vec![10.0, 0.0, 30.0], vec![1.0, 0.0, 1.0], 1000)
    }

    fn make_restoration() -> RestorationResult {
        let f = make_fragment(42, FragmentKind::Text);
        let config = InversionConfig::default();
        restore(&f, &config)
    }

    #[test]
    fn test_fragment_to_analytics() {
        let f = make_fragment(1, FragmentKind::Image);
        let ev = history_fragment_to_analytics(&f);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.fragment_id, 1);
        assert_eq!(ev.kind, 1); // Image
        assert_eq!(ev.missing_count, 1);
        assert_eq!(ev.data_length, 3);
    }

    #[test]
    fn test_fragment_to_analytics_all_known() {
        let f = Fragment::new(2, FragmentKind::Text, vec![1.0, 2.0], vec![1.0, 1.0], 0);
        let ev = history_fragment_to_analytics(&f);
        assert_eq!(ev.missing_count, 0);
        assert!((ev.known_fraction - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_restoration_to_analytics() {
        let r = make_restoration();
        let ev = history_restoration_to_analytics(&r);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.fragment_id, 42);
        assert!(ev.entropy_before >= 0.0);
        assert!(ev.entropy_after >= 0.0);
        assert!(ev.entropy_reduction >= 0.0);
        assert!(ev.entropy_reduction <= 1.0);
        assert!(ev.iterations > 0);
    }

    #[test]
    fn test_restoration_to_db() {
        let r = make_restoration();
        let rec = history_restoration_to_db(&r);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.fragment_id, 42);
        assert_eq!(rec.value_count, 3);
        assert!(rec.iterations > 0);
    }

    #[test]
    fn test_entropy_to_analytics() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let m = measure_entropy(&data, 5);
        let ev = history_entropy_to_analytics(&m);
        assert_ne!(ev.content_hash, 0);
        assert!(ev.shannon_entropy >= 0.0);
        assert_eq!(ev.total_symbols, 5);
    }

    #[test]
    fn test_restoration_to_cache_high_quality() {
        let r = make_restoration();
        let entry = history_restoration_to_cache(&r);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.fragment_id, 42);
        // All-known endpoints with single gap → high confidence
        assert!(entry.mean_confidence > 0.0);
    }

    #[test]
    fn test_hash_determinism() {
        let r = make_restoration();
        let e1 = history_restoration_to_analytics(&r);
        let e2 = history_restoration_to_analytics(&r);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    #[test]
    fn test_all_fragment_kinds_discriminant() {
        let kinds = [
            (FragmentKind::Text, 0u8),
            (FragmentKind::Image, 1),
            (FragmentKind::Artifact, 2),
            (FragmentKind::Inscription, 3),
            (FragmentKind::Audio, 4),
        ];
        for (kind, expected) in kinds {
            let f = make_fragment(0, kind);
            let ev = history_fragment_to_analytics(&f);
            assert_eq!(ev.kind, expected);
        }
    }
}
