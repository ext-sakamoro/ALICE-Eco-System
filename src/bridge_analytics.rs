//! Analytics bridges — ALICE-Analytics ↔ DB, Cache, CDN, ML, Search, View, Edge
//!
//! 7 bridges connecting streaming analytics sketches to the ALICE ecosystem.

use alice_analytics::prelude::*;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Analytics → DB (metric snapshot persistence) ──────────────

/// Metric snapshot record for ALICE-DB persistence.
pub struct AnalyticsDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// P50 latency (quantile).
    pub p50: f64,
    /// P99 latency (quantile).
    pub p99: f64,
    /// Total observation count.
    pub count: u64,
}

/// Serialize analytics snapshot for ALICE-DB persistence.
#[inline]
pub fn analytics_to_db_record(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsDbRecord {
    let card = hll.cardinality();
    let count = dd.count();
    let data = [card.to_le_bytes().as_slice(), &count.to_le_bytes()].concat();
    AnalyticsDbRecord {
        content_hash: fnv1a(&data),
        cardinality: card,
        p50: dd.quantile(0.50),
        p99: dd.quantile(0.99),
        count,
    }
}

// ── Bridge 2: Analytics → Cache (sketch caching) ────────────────────────

/// Sketch cache entry for ALICE-Cache.
pub struct AnalyticsCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// Observation count.
    pub count: u64,
}

/// Cache analytics sketch for ALICE-Cache.
#[inline]
pub fn analytics_to_cache_entry(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsCacheEntry {
    let card = hll.cardinality();
    let count = dd.count();
    let data = card.to_le_bytes();
    AnalyticsCacheEntry {
        content_hash: fnv1a(&data),
        cardinality: card,
        count,
    }
}

// ── Bridge 3: Analytics → CDN (aggregated report delivery) ──────────────

/// Aggregated report for ALICE-CDN delivery.
pub struct AnalyticsCdnReport {
    /// Content hash.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// P50.
    pub p50: f64,
    /// P95.
    pub p95: f64,
    /// P99.
    pub p99: f64,
    /// MIME type.
    pub content_type: &'static str,
}

/// Package analytics report for ALICE-CDN delivery.
#[inline]
pub fn analytics_to_cdn_report(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsCdnReport {
    let card = hll.cardinality();
    let p50 = dd.quantile(0.50);
    let p99 = dd.quantile(0.99);
    let data = [card.to_le_bytes().as_slice(), &p50.to_le_bytes(), &p99.to_le_bytes()].concat();
    AnalyticsCdnReport {
        content_hash: fnv1a(&data),
        cardinality: card,
        p50,
        p95: dd.quantile(0.95),
        p99,
        content_type: "application/x-alice-analytics",
    }
}

// ── Bridge 4: Analytics → ML (feature extraction for anomaly detection) ──

/// ML feature vector from analytics for anomaly detection.
pub struct AnalyticsMlFeatures {
    /// Cardinality (normalized).
    pub cardinality: f64,
    /// Mean value.
    pub mean: f64,
    /// P50.
    pub p50: f64,
    /// P99.
    pub p99: f64,
    /// Count.
    pub count: u64,
    /// Feature vector for ML input.
    pub feature_vec: Vec<f64>,
}

/// Extract ML features from analytics for ALICE-ML anomaly detection.
#[inline]
pub fn analytics_to_ml_features(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsMlFeatures {
    let card = hll.cardinality();
    let mean = dd.mean();
    let p50 = dd.quantile(0.50);
    let p99 = dd.quantile(0.99);
    let count = dd.count();
    AnalyticsMlFeatures {
        cardinality: card,
        mean,
        p50,
        p99,
        count,
        feature_vec: vec![card, mean, p50, p99, count as f64],
    }
}

// ── Bridge 5: Analytics → Search (metric index) ─────────────────────────

/// Metric search index for ALICE-Search.
pub struct AnalyticsSearchIndex {
    /// Content hash for search key.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// Count.
    pub count: u64,
    /// Mean value.
    pub mean: f64,
}

/// Index analytics metrics for ALICE-Search.
#[inline]
pub fn analytics_to_search_index(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsSearchIndex {
    let card = hll.cardinality();
    let data = card.to_le_bytes();
    AnalyticsSearchIndex {
        content_hash: fnv1a(&data),
        cardinality: card,
        count: dd.count(),
        mean: dd.mean(),
    }
}

// ── Bridge 6: Analytics → View (dashboard visualization) ─────────────────

/// Dashboard visualization config for ALICE-View.
pub struct AnalyticsViewConfig {
    /// Cardinality estimate.
    pub cardinality: f64,
    /// P50.
    pub p50: f64,
    /// P95.
    pub p95: f64,
    /// P99.
    pub p99: f64,
    /// Count.
    pub count: u64,
    /// Mean.
    pub mean: f64,
}

/// Configure dashboard visualization for ALICE-View.
#[inline]
pub fn analytics_to_view_config(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsViewConfig {
    AnalyticsViewConfig {
        cardinality: hll.cardinality(),
        p50: dd.quantile(0.50),
        p95: dd.quantile(0.95),
        p99: dd.quantile(0.99),
        count: dd.count(),
        mean: dd.mean(),
    }
}

// ── Bridge 7: Analytics → Edge (lightweight telemetry) ───────────────────

/// Lightweight telemetry payload for ALICE-Edge.
pub struct AnalyticsEdgePayload {
    /// Content hash.
    pub content_hash: u64,
    /// Cardinality estimate.
    pub cardinality: f64,
    /// Count.
    pub count: u64,
    /// Payload size (estimated bytes).
    pub estimated_bytes: usize,
}

/// Prepare lightweight telemetry for ALICE-Edge devices.
#[inline]
pub fn analytics_to_edge_payload(hll: &HyperLogLog12, dd: &DDSketch256) -> AnalyticsEdgePayload {
    let card = hll.cardinality();
    let count = dd.count();
    let data = [card.to_le_bytes().as_slice(), &count.to_le_bytes()].concat();
    // Compact payload: 8 bytes cardinality + 8 bytes count = 16 bytes
    AnalyticsEdgePayload {
        content_hash: fnv1a(&data),
        cardinality: card,
        count,
        estimated_bytes: 16,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_sketches() -> (HyperLogLog12, DDSketch256) {
        let mut hll = HyperLogLog12::new();
        let mut dd = DDSketch256::new(0.01);
        for i in 0..100u64 {
            hll.insert_hash(FnvHasher::hash_u64(i));
            dd.insert(i as f64);
        }
        (hll, dd)
    }

    #[test]
    fn test_analytics_to_db_record() {
        let (hll, dd) = test_sketches();
        let rec = analytics_to_db_record(&hll, &dd);
        assert_ne!(rec.content_hash, 0);
        assert!(rec.cardinality > 0.0);
        assert_eq!(rec.count, 100);
    }

    #[test]
    fn test_analytics_to_cache_entry() {
        let (hll, dd) = test_sketches();
        let entry = analytics_to_cache_entry(&hll, &dd);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.count, 100);
    }

    #[test]
    fn test_analytics_to_cdn_report() {
        let (hll, dd) = test_sketches();
        let rpt = analytics_to_cdn_report(&hll, &dd);
        assert_ne!(rpt.content_hash, 0);
        assert_eq!(rpt.content_type, "application/x-alice-analytics");
    }

    #[test]
    fn test_analytics_to_ml_features() {
        let (hll, dd) = test_sketches();
        let f = analytics_to_ml_features(&hll, &dd);
        assert_eq!(f.feature_vec.len(), 5);
        assert_eq!(f.count, 100);
    }

    #[test]
    fn test_analytics_to_search_index() {
        let (hll, dd) = test_sketches();
        let idx = analytics_to_search_index(&hll, &dd);
        assert_ne!(idx.content_hash, 0);
        assert_eq!(idx.count, 100);
    }

    #[test]
    fn test_analytics_to_view_config() {
        let (hll, dd) = test_sketches();
        let cfg = analytics_to_view_config(&hll, &dd);
        assert!(cfg.cardinality > 0.0);
        assert_eq!(cfg.count, 100);
    }

    #[test]
    fn test_analytics_to_edge_payload() {
        let (hll, dd) = test_sketches();
        let payload = analytics_to_edge_payload(&hll, &dd);
        assert_ne!(payload.content_hash, 0);
        assert_eq!(payload.estimated_bytes, 16);
    }
}
