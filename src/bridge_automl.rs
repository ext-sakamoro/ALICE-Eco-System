//! AutoML bridges — ALICE-AutoML ↔ DB, Cache, Analytics, ML, API
//!
//! 5 bridges connecting automated machine learning search to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: AutoML → DB (trial log) ───────────────────────────────────

/// Trial log record for ALICE-DB persistence.
pub struct AutoMlDbRecord {
    /// Content hash over the trial snapshot.
    pub content_hash: u64,
    /// Total number of trials completed so far.
    pub trial_count: u64,
    /// Best objective score observed across all trials.
    pub best_score: f64,
    /// Total number of hyperparameters in the search space.
    pub search_space_size: u32,
    /// Elapsed wall-clock time in seconds since the search started.
    pub elapsed_secs: u64,
    /// Number of hyperparameters in the best trial configuration.
    pub param_count: u32,
}

/// Serialize a completed AutoML trial for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn automl_to_db_record(
    trial_count: u64,
    best_score: f64,
    search_space_size: u32,
    elapsed_secs: u64,
    param_count: u32,
) -> AutoMlDbRecord {
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&trial_count.to_le_bytes());
    buf[8..16].copy_from_slice(&best_score.to_bits().to_le_bytes());
    buf[16..20].copy_from_slice(&search_space_size.to_le_bytes());
    buf[20..28].copy_from_slice(&elapsed_secs.to_le_bytes());
    AutoMlDbRecord {
        content_hash: fnv1a(&buf),
        trial_count,
        best_score,
        search_space_size,
        elapsed_secs,
        param_count,
    }
}

// ── Bridge 2: AutoML → Cache (trial cache) ──────────────────────────────

/// Trial result cache entry for ALICE-Cache.
pub struct AutoMlCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Trial identifier.
    pub trial_count: u64,
    /// Objective score for this trial.
    pub best_score: f64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Whether this trial holds the current best score.
    pub is_best_trial: bool,
}

/// Build a trial cache entry for ALICE-Cache.
///
/// Best-trial entries receive a longer TTL (1800 s vs 300 s) because they
/// are the candidates for final model export.
#[inline]
#[must_use]
pub fn automl_to_cache_entry(
    trial_count: u64,
    best_score: f64,
    is_best_trial: bool,
) -> AutoMlCacheEntry {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&trial_count.to_le_bytes());
    buf[8..16].copy_from_slice(&best_score.to_bits().to_le_bytes());
    buf[16] = is_best_trial as u8;
    let best_flag = is_best_trial as u32;
    let ttl_secs = 300 + best_flag * 1500;
    AutoMlCacheEntry {
        content_hash: fnv1a(&buf),
        trial_count,
        best_score,
        ttl_secs,
        is_best_trial,
    }
}

// ── Bridge 3: AutoML → Analytics (search metrics) ───────────────────────

/// Search progress metrics for ALICE-Analytics ingestion.
pub struct AutoMlAnalyticsMetrics {
    /// Content hash over the metric tuple.
    pub content_hash: u64,
    /// Total trials completed.
    pub trial_count: u64,
    /// Best objective score observed.
    pub best_score: f64,
    /// Average objective score across all trials.
    pub avg_score: f64,
    /// Size of the hyperparameter search space.
    pub search_space_size: u32,
    /// Elapsed search time in seconds.
    pub elapsed_secs: u64,
}

/// Build search progress metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn automl_to_analytics_metrics(
    trial_count: u64,
    best_score: f64,
    total_score: f64,
    search_space_size: u32,
    elapsed_secs: u64,
) -> AutoMlAnalyticsMetrics {
    let rcp = 1.0 / trial_count.max(1) as f64;
    let avg_score = total_score * rcp;
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&trial_count.to_le_bytes());
    buf[8..16].copy_from_slice(&best_score.to_bits().to_le_bytes());
    buf[16..20].copy_from_slice(&search_space_size.to_le_bytes());
    buf[20..28].copy_from_slice(&elapsed_secs.to_le_bytes());
    AutoMlAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        trial_count,
        best_score,
        avg_score,
        search_space_size,
        elapsed_secs,
    }
}

// ── Bridge 4: AutoML → ML (best model) ──────────────────────────────────

/// Best-model descriptor for ALICE-ML export.
pub struct AutoMlMlModel {
    /// Content hash over the model descriptor.
    pub content_hash: u64,
    /// Trial index of the best model.
    pub trial_count: u64,
    /// Objective score of the best model.
    pub best_score: f64,
    /// Number of hyperparameters in the best configuration.
    pub param_count: u32,
    /// Search space size that produced this model.
    pub search_space_size: u32,
    /// Elapsed seconds at the time the best model was found.
    pub elapsed_secs: u64,
}

/// Export the best AutoML model descriptor for ALICE-ML.
#[inline]
#[must_use]
pub fn automl_to_ml_model(
    trial_count: u64,
    best_score: f64,
    param_count: u32,
    search_space_size: u32,
    elapsed_secs: u64,
) -> AutoMlMlModel {
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&trial_count.to_le_bytes());
    buf[8..16].copy_from_slice(&best_score.to_bits().to_le_bytes());
    buf[16..20].copy_from_slice(&param_count.to_le_bytes());
    buf[20..24].copy_from_slice(&search_space_size.to_le_bytes());
    buf[24..28].copy_from_slice(&elapsed_secs.to_le_bytes()[..4]);
    AutoMlMlModel {
        content_hash: fnv1a(&buf),
        trial_count,
        best_score,
        param_count,
        search_space_size,
        elapsed_secs,
    }
}

// ── Bridge 5: AutoML → API (search service) ─────────────────────────────

/// Search service response for ALICE-API.
pub struct AutoMlApiResponse {
    /// Content hash over the response payload.
    pub content_hash: u64,
    /// Number of trials completed at response time.
    pub trial_count: u64,
    /// Best score at response time.
    pub best_score: f64,
    /// Whether the search has converged.
    pub converged: bool,
    /// HTTP status code.
    pub status_code: u16,
    /// Elapsed search time in seconds.
    pub elapsed_secs: u64,
}

/// Build a search service response for ALICE-API.
#[inline]
#[must_use]
pub fn automl_to_api_response(
    trial_count: u64,
    best_score: f64,
    converged: bool,
    elapsed_secs: u64,
) -> AutoMlApiResponse {
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&trial_count.to_le_bytes());
    buf[8..16].copy_from_slice(&best_score.to_bits().to_le_bytes());
    buf[16..24].copy_from_slice(&elapsed_secs.to_le_bytes());
    buf[24] = converged as u8;
    let status_code = if converged { 200 } else { 202 };
    AutoMlApiResponse {
        content_hash: fnv1a(&buf),
        trial_count,
        best_score,
        converged,
        status_code,
        elapsed_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automl_to_db_record_hash_nonzero() {
        let rec = automl_to_db_record(50, 0.92, 1000, 3600, 12);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_automl_to_db_record_fields() {
        let rec = automl_to_db_record(10, 0.85, 500, 1800, 8);
        assert_eq!(rec.trial_count, 10);
        assert!((rec.best_score - 0.85).abs() < 1e-9);
        assert_eq!(rec.search_space_size, 500);
        assert_eq!(rec.elapsed_secs, 1800);
        assert_eq!(rec.param_count, 8);
    }

    #[test]
    fn test_automl_to_cache_entry_normal_ttl() {
        let entry = automl_to_cache_entry(5, 0.7, false);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
        assert!(!entry.is_best_trial);
    }

    #[test]
    fn test_automl_to_cache_entry_best_ttl() {
        let entry = automl_to_cache_entry(20, 0.95, true);
        assert_eq!(entry.ttl_secs, 1800);
        assert!(entry.is_best_trial);
    }

    #[test]
    fn test_automl_to_analytics_metrics_avg() {
        // 10 trials, total score = 8.0 → avg = 0.8.
        let m = automl_to_analytics_metrics(10, 0.95, 8.0, 200, 7200);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.trial_count, 10);
        assert!((m.avg_score - 0.8).abs() < 1e-9);
        assert_eq!(m.search_space_size, 200);
    }

    #[test]
    fn test_automl_to_analytics_metrics_zero_trials() {
        let m = automl_to_analytics_metrics(0, 0.0, 0.0, 100, 0);
        assert_eq!(m.avg_score, 0.0);
    }

    #[test]
    fn test_automl_to_ml_model() {
        let model = automl_to_ml_model(30, 0.97, 15, 500, 5400);
        assert_ne!(model.content_hash, 0);
        assert_eq!(model.param_count, 15);
        assert!((model.best_score - 0.97).abs() < 1e-9);
    }

    #[test]
    fn test_automl_to_api_response_converged() {
        let resp = automl_to_api_response(100, 0.99, true, 10_800);
        assert_ne!(resp.content_hash, 0);
        assert_eq!(resp.status_code, 200);
        assert!(resp.converged);
    }
}
