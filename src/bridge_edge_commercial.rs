//! EdgeCommercial bridges — ALICE-Edge-Enterprise ↔ DB, Analytics, Cache
//!
//! 3 bridges connecting the enterprise edge semantic-telemetry crate to the
//! ALICE ecosystem. Gated behind the `edge-commercial` feature flag.

#[cfg(feature = "edge-commercial")]
use alice_edge_enterprise::{Embedding512, SemanticState, StateTransition};

// ── Bridge 1: EdgeCommercial → DB (enterprise inference results persistence) ──

/// Persistent record of a semantic inference result for ALICE-DB.
///
/// Captures the embedding fingerprint, the inferred state, and the
/// confidence score so that downstream queries can filter and index
/// inference history without storing the full 2 KB embedding on every row.
#[cfg(feature = "edge-commercial")]
pub struct EdgeCommercialDbRecord {
    /// FNV-1a fingerprint of the 2048-byte embedding (dedup key).
    pub embedding_hash: u64,
    /// Inferred semantic state discriminant (0–7).
    pub state_discriminant: u8,
    /// Human-readable state label (static str, zero allocation).
    pub state_label: &'static str,
    /// Inference confidence in [0.0, 1.0].
    pub confidence: f32,
    /// State priority level (0 = background, 10 = hazard).
    pub priority: u8,
    /// Is this a high-urgency event that should trigger an alert?
    pub is_alert: bool,
    /// Content hash over (embedding_hash, state_discriminant, confidence_pct)
    /// for deduplication across DB writes.
    pub content_hash: u64,
}

/// Build a DB persistence record from a semantic inference result.
///
/// # Optimization notes
/// - Embedding hash: FNV-1a over the 2048-byte f32 payload (byte-level).
/// - Confidence pct: `(confidence * 100.0) as u8` — single multiply, no
///   division.
/// - `is_alert`: branchless threshold check (`priority >= 7`).
#[cfg(feature = "edge-commercial")]
#[inline]
pub fn edge_commercial_to_db_record(
    embedding: &Embedding512,
    state: SemanticState,
    confidence: f32,
) -> EdgeCommercialDbRecord {
    // Hash the raw embedding bytes (2048 bytes = 512 × 4).
    let embedding_bytes = embedding.to_bytes();
    let embedding_hash = crate::hash::fnv1a(&embedding_bytes);

    let state_discriminant = state as u8;
    let state_label = state.label();
    let priority = state.priority();

    // is_alert: branchless — true when priority >= 7 (Missing or Hazard).
    let is_alert = priority >= 7;

    // Confidence percent packed into key (0..100 as u8, avoids f32 in hash).
    let confidence_clamped = confidence.max(0.0).min(1.0);
    let confidence_pct = (confidence_clamped * 100.0) as u8;

    // Content hash: embedding_hash bytes + state discriminant + confidence pct.
    let mut key = [0u8; 10];
    key[0..8].copy_from_slice(&embedding_hash.to_le_bytes());
    key[8] = state_discriminant;
    key[9] = confidence_pct;
    let content_hash = crate::hash::fnv1a(&key);

    EdgeCommercialDbRecord {
        embedding_hash,
        state_discriminant,
        state_label,
        confidence: confidence_clamped,
        priority,
        is_alert,
        content_hash,
    }
}

// ── Bridge 2: EdgeCommercial → Analytics (enterprise edge metrics) ────────

/// Analytics snapshot for enterprise edge device performance.
///
/// Aggregates inference batch statistics into a flat record suitable for
/// time-series ingestion by ALICE-Analytics.
#[cfg(feature = "edge-commercial")]
pub struct EdgeCommercialAnalyticsMetrics {
    /// Total inferences processed in this batch.
    pub batch_size: u32,
    /// Count of alert-level events (priority >= 7) in the batch.
    pub alert_count: u32,
    /// Alert rate in [0.0, 1.0] (alert_count / batch_size).
    pub alert_rate: f32,
    /// Average confidence over the batch.
    pub avg_confidence: f32,
    /// Estimated embedding bandwidth saved vs raw frames.
    ///
    /// 1 embedding = 2048 bytes; 1 raw JPEG frame ≈ 153600 bytes (150 KB).
    /// savings_ratio = 153600 / 2048 = 75.0 (constant).
    pub bandwidth_savings_ratio: f32,
    /// State distribution: count per state discriminant (index = discriminant).
    pub state_counts: [u32; 8],
    /// Content hash for dedup / time-series keying.
    pub content_hash: u64,
}

/// Derive analytics metrics from a batch of enterprise edge inferences.
///
/// `states` and `confidences` must have the same length.
///
/// # Optimization notes
/// - Alert rate: `alert_count * RCP_BATCH`, reciprocal multiply.
/// - Avg confidence: accumulated as f64 then divided once at the end.
/// - Bandwidth savings: compile-time constant (no runtime division).
/// - State distribution: single-pass scan, branchless index via discriminant cast.
#[cfg(feature = "edge-commercial")]
#[inline]
pub fn edge_commercial_to_analytics_metrics(
    states: &[SemanticState],
    confidences: &[f32],
) -> EdgeCommercialAnalyticsMetrics {
    let n = states.len().max(1);
    let rcp_n = 1.0 / n as f32;

    // Compile-time bandwidth savings ratio: 150 KB JPEG / 2 KB embedding.
    // 153600 / 2048 = 75.0 — exact integer ratio, no runtime division.
    const BANDWIDTH_SAVINGS_RATIO: f32 = 75.0;

    let mut alert_count: u32 = 0;
    let mut confidence_sum: f64 = 0.0;
    let mut state_counts = [0u32; 8];

    for (i, &state) in states.iter().enumerate() {
        let priority = state.priority();
        // Branchless alert accumulation: cast bool to u32.
        alert_count += (priority >= 7) as u32;

        // Confidence accumulation (saturate missing entries at 0).
        let conf = confidences.get(i).copied().unwrap_or(0.0);
        confidence_sum += conf as f64;

        // Branchless state histogram: index by discriminant (0–7).
        let idx = (state as u8) as usize;
        state_counts[idx] += 1;
    }

    let alert_rate = alert_count as f32 * rcp_n;
    let avg_confidence = (confidence_sum * rcp_n as f64) as f32;

    // Content hash over batch_size, alert_count, avg_confidence_pct.
    let avg_conf_pct = (avg_confidence * 100.0) as u8;
    let mut key = [0u8; 9];
    key[0..4].copy_from_slice(&(n as u32).to_le_bytes());
    key[4..8].copy_from_slice(&alert_count.to_le_bytes());
    key[8] = avg_conf_pct;
    let content_hash = crate::hash::fnv1a(&key);

    EdgeCommercialAnalyticsMetrics {
        batch_size: n as u32,
        alert_count,
        alert_rate,
        avg_confidence,
        bandwidth_savings_ratio: BANDWIDTH_SAVINGS_RATIO,
        state_counts,
        content_hash,
    }
}

// ── Bridge 3: EdgeCommercial → Cache (model caching metadata) ─────────────

/// Cache entry descriptor for a semantic inference model snapshot.
///
/// Carries the embedding fingerprint and state transition metadata so that
/// ALICE-Cache can key model checkpoints for fast edge re-load.
#[cfg(feature = "edge-commercial")]
pub struct EdgeCommercialCacheEntry {
    /// FNV-1a fingerprint of the embedding (primary cache key).
    pub embedding_hash: u64,
    /// Previous state discriminant (for transition-aware cache invalidation).
    pub prev_state: u8,
    /// Current state discriminant.
    pub current_state: u8,
    /// Did a state transition occur? (cache invalidation hint)
    pub state_changed: bool,
    /// Transition timestamp in milliseconds (for time-based cache expiry).
    pub timestamp_ms: u64,
    /// Confidence of the new state in [0.0, 1.0].
    pub confidence: f32,
    /// Debounce count: frames that confirmed this transition.
    pub debounce_count: u32,
    /// Combined cache key (embedding_hash XOR rotated timestamp_ms).
    pub cache_key: u64,
}

/// Build a cache entry from a state transition event for ALICE-Cache.
///
/// `embedding` is the current frame's CLIP embedding.
/// `transition` is the state machine transition produced by ALICE-Edge-Enterprise.
///
/// # Optimization notes
/// - `state_changed`: branchless `prev != current` bool cast.
/// - `cache_key`: XOR + rotate avoids a second FNV pass over large data.
#[cfg(feature = "edge-commercial")]
#[inline]
pub fn edge_commercial_to_cache_entry(
    embedding: &Embedding512,
    transition: &StateTransition,
) -> EdgeCommercialCacheEntry {
    let embedding_bytes = embedding.to_bytes();
    let embedding_hash = crate::hash::fnv1a(&embedding_bytes);

    let prev_state = transition.from as u8;
    let current_state = transition.to as u8;

    // Branchless state_changed: cast equality test to bool.
    let state_changed = prev_state != current_state;

    let timestamp_ms = transition.timestamp_ms;

    // Cache key: combine embedding fingerprint with timestamp.
    // Rotate by 13 bits to distribute entropy before XOR.
    let cache_key = embedding_hash ^ timestamp_ms.rotate_left(13);

    EdgeCommercialCacheEntry {
        embedding_hash,
        prev_state,
        current_state,
        state_changed,
        timestamp_ms,
        confidence: transition.confidence,
        debounce_count: transition.debounce_count,
        cache_key,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(all(test, feature = "edge-commercial"))]
mod tests {
    use super::*;
    use alice_edge_enterprise::{Embedding512, SemanticState, StateTransition};

    fn test_embedding() -> Embedding512 {
        let mut e = Embedding512::zero();
        for (i, v) in e.data.iter_mut().enumerate() {
            *v = (i as f32) * 0.01;
        }
        e
    }

    // ── Bridge 1 test ─────────────────────────────────────────────────────

    #[test]
    fn test_edge_commercial_to_db_record_normal() {
        let emb = test_embedding();
        let rec = edge_commercial_to_db_record(&emb, SemanticState::Full, 0.92);

        assert_eq!(rec.state_discriminant, SemanticState::Full as u8);
        assert_eq!(rec.state_label, "full");
        assert_eq!(rec.priority, 1);
        assert!(!rec.is_alert);
        assert!((rec.confidence - 0.92).abs() < 1e-5);
        assert_ne!(rec.embedding_hash, 0);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.content_hash, 0xcbf29ce484222325);
    }

    #[test]
    fn test_edge_commercial_to_db_record_hazard() {
        let emb = test_embedding();
        let rec = edge_commercial_to_db_record(&emb, SemanticState::Hazard, 0.88);

        assert_eq!(rec.state_discriminant, SemanticState::Hazard as u8);
        assert_eq!(rec.state_label, "hazard");
        assert_eq!(rec.priority, 10);
        assert!(rec.is_alert, "Hazard should trigger alert");
    }

    #[test]
    fn test_edge_commercial_to_db_record_missing() {
        let emb = test_embedding();
        let rec = edge_commercial_to_db_record(&emb, SemanticState::Missing, 0.75);
        assert_eq!(rec.priority, 7);
        assert!(rec.is_alert, "Missing should trigger alert");
    }

    #[test]
    fn test_edge_commercial_to_db_confidence_clamp() {
        let emb = test_embedding();
        // Confidence above 1.0 must be clamped.
        let rec = edge_commercial_to_db_record(&emb, SemanticState::Full, 1.5);
        assert_eq!(rec.confidence, 1.0);
        // Confidence below 0.0 must be clamped.
        let rec2 = edge_commercial_to_db_record(&emb, SemanticState::Full, -0.1);
        assert_eq!(rec2.confidence, 0.0);
    }

    #[test]
    fn test_edge_commercial_to_db_different_states_different_hashes() {
        let emb = test_embedding();
        let rec_full = edge_commercial_to_db_record(&emb, SemanticState::Full, 0.9);
        let rec_empty = edge_commercial_to_db_record(&emb, SemanticState::Empty, 0.9);
        // Different states with the same embedding must yield different content hashes.
        assert_ne!(rec_full.content_hash, rec_empty.content_hash);
    }

    // ── Bridge 2 test ─────────────────────────────────────────────────────

    #[test]
    fn test_edge_commercial_to_analytics_metrics_basic() {
        let states = vec![
            SemanticState::Full,
            SemanticState::Empty,
            SemanticState::Hazard,
            SemanticState::Missing,
            SemanticState::Dirty,
        ];
        let confidences = vec![0.9, 0.8, 0.95, 0.7, 0.85];

        let m = edge_commercial_to_analytics_metrics(&states, &confidences);

        assert_eq!(m.batch_size, 5);
        // Hazard + Missing = 2 alerts.
        assert_eq!(m.alert_count, 2);
        assert!((m.alert_rate - 0.4).abs() < 1e-5);
        assert!((m.avg_confidence - 0.84).abs() < 1e-4);
        // Bandwidth savings ratio is always 75.0.
        assert!((m.bandwidth_savings_ratio - 75.0).abs() < 1e-6);
        // State histogram checks.
        assert_eq!(m.state_counts[SemanticState::Full as usize], 1);
        assert_eq!(m.state_counts[SemanticState::Empty as usize], 1);
        assert_eq!(m.state_counts[SemanticState::Hazard as usize], 1);
        assert_eq!(m.state_counts[SemanticState::Missing as usize], 1);
        assert_eq!(m.state_counts[SemanticState::Dirty as usize], 1);
        assert_ne!(m.content_hash, 0);
    }

    #[test]
    fn test_edge_commercial_to_analytics_no_alerts() {
        let states = vec![SemanticState::Full, SemanticState::InUse];
        let confidences = vec![1.0, 0.8];
        let m = edge_commercial_to_analytics_metrics(&states, &confidences);
        assert_eq!(m.alert_count, 0);
        assert_eq!(m.alert_rate, 0.0);
    }

    #[test]
    fn test_edge_commercial_to_analytics_all_alerts() {
        let states = vec![SemanticState::Hazard, SemanticState::Missing];
        let confidences = vec![0.9, 0.9];
        let m = edge_commercial_to_analytics_metrics(&states, &confidences);
        assert_eq!(m.alert_count, 2);
        assert!((m.alert_rate - 1.0).abs() < 1e-5);
    }

    // ── Bridge 3 test ─────────────────────────────────────────────────────

    #[test]
    fn test_edge_commercial_to_cache_entry_transition() {
        let emb = test_embedding();
        let transition = StateTransition {
            from: SemanticState::Full,
            to: SemanticState::Empty,
            timestamp_ms: 42_000,
            confidence: 0.87,
            debounce_count: 3,
        };

        let entry = edge_commercial_to_cache_entry(&emb, &transition);

        assert_ne!(entry.embedding_hash, 0);
        assert_eq!(entry.prev_state, SemanticState::Full as u8);
        assert_eq!(entry.current_state, SemanticState::Empty as u8);
        assert!(entry.state_changed);
        assert_eq!(entry.timestamp_ms, 42_000);
        assert_eq!(entry.debounce_count, 3);
        assert!((entry.confidence - 0.87).abs() < 1e-5);
        assert_ne!(entry.cache_key, 0);
        // cache_key must differ from embedding_hash alone.
        assert_ne!(entry.cache_key, entry.embedding_hash);
    }

    #[test]
    fn test_edge_commercial_to_cache_entry_no_change() {
        let emb = test_embedding();
        let transition = StateTransition {
            from: SemanticState::Full,
            to: SemanticState::Full,
            timestamp_ms: 100_000,
            confidence: 0.95,
            debounce_count: 5,
        };

        let entry = edge_commercial_to_cache_entry(&emb, &transition);
        assert!(!entry.state_changed, "No change when prev == current");
        assert_eq!(entry.prev_state, entry.current_state);
    }

    #[test]
    fn test_edge_commercial_to_cache_entry_unique_keys_per_timestamp() {
        let emb = test_embedding();
        let t1 = StateTransition {
            from: SemanticState::Full, to: SemanticState::Empty,
            timestamp_ms: 1_000, confidence: 0.9, debounce_count: 3,
        };
        let t2 = StateTransition {
            from: SemanticState::Full, to: SemanticState::Empty,
            timestamp_ms: 2_000, confidence: 0.9, debounce_count: 3,
        };

        let e1 = edge_commercial_to_cache_entry(&emb, &t1);
        let e2 = edge_commercial_to_cache_entry(&emb, &t2);

        // Same embedding + same transition direction but different timestamp → different cache key.
        assert_ne!(e1.cache_key, e2.cache_key);
    }
}
