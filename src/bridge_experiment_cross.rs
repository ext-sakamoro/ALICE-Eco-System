//! Cross-domain bridges — ALICE-Experiment ↔ ML, Analytics, Cache
//!
//! 5 bridges connecting A/B experiment data to ML feature extraction,
//! Analytics metrics, ML reward signals, and Cache.

use alice_experiment::{p_value_from_z, z_test_proportions, BanditArm, ConversionData, Variant};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Experiment Variant → ML feature for prediction ─────────

/// An experiment variant converted into an ML feature vector.
///
/// Encodes variant name hash and traffic percentage so the ML layer can
/// use experiment configuration as input features for outcome prediction.
pub struct ExperimentVariantMlFeature {
    /// FNV-1a hash over name hash, traffic_pct bytes.
    pub content_hash: u64,
    /// Hash of the variant name.
    pub name_hash: u64,
    /// Traffic percentage as raw f64 (0.0 - 1.0).
    pub traffic_pct: f64,
    /// Feature dimension: always 3 (name_hash_lo, name_hash_hi, traffic_pct).
    pub feature_dim: usize,
}

/// Convert an experiment variant into an ML feature descriptor.
#[inline]
#[must_use]
pub fn experiment_variant_to_ml_feature(variant: &Variant) -> ExperimentVariantMlFeature {
    let name_hash = fnv1a(variant.name.as_bytes());

    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&name_hash.to_le_bytes());
    key[8..16].copy_from_slice(&variant.traffic_pct.to_bits().to_le_bytes());

    ExperimentVariantMlFeature {
        content_hash: fnv1a(&key),
        name_hash,
        traffic_pct: variant.traffic_pct,
        feature_dim: 3,
    }
}

// ── Bridge 2: ConversionData → Analytics metrics ─────────────────────

/// Conversion data converted into Analytics-compatible metrics.
///
/// Summarizes A/B conversion statistics into metrics suitable for
/// the Analytics pipeline (counters, rates, confidence intervals).
pub struct ExperimentConversionAnalytics {
    /// FNV-1a hash over variant hash, visitors, conversions, rate bytes.
    pub content_hash: u64,
    /// Hash of the variant name.
    pub variant_hash: u64,
    /// Total visitors.
    pub visitors: u64,
    /// Total conversions.
    pub conversions: u64,
    /// Conversion rate (conversions / visitors).
    pub rate: f64,
    /// Wilson confidence interval lower bound (z=1.96).
    pub ci_lower: f64,
    /// Wilson confidence interval upper bound (z=1.96).
    pub ci_upper: f64,
    /// Metric name hash for Analytics pipeline registration.
    pub metric_name_hash: u64,
}

/// Convert conversion data into Analytics metrics.
#[inline]
#[must_use]
pub fn experiment_conversion_to_analytics(data: &ConversionData) -> ExperimentConversionAnalytics {
    let variant_hash = fnv1a(data.variant.as_bytes());
    let rate = data.rate();
    let (ci_lower, ci_upper) = data.confidence_interval(1.96);
    let metric_name_hash = fnv1a(b"experiment.conversion");

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&variant_hash.to_le_bytes());
    key[8..16].copy_from_slice(&data.visitors.to_le_bytes());
    key[16..24].copy_from_slice(&data.conversions.to_le_bytes());
    key[24..32].copy_from_slice(&rate.to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&ci_lower.to_bits().to_le_bytes());

    ExperimentConversionAnalytics {
        content_hash: fnv1a(&key),
        variant_hash,
        visitors: data.visitors,
        conversions: data.conversions,
        rate,
        ci_lower,
        ci_upper,
        metric_name_hash,
    }
}

// ── Bridge 3: BanditArm → ML reward signal ──────────────────────────

/// A bandit arm converted into an ML reward signal.
///
/// Encodes arm performance (expected reward, trial counts, success ratio)
/// so the ML layer can use bandit feedback as training reward signals
/// for reinforcement learning.
pub struct ExperimentBanditMlReward {
    /// FNV-1a hash over name hash, successes, failures, expected_reward bytes.
    pub content_hash: u64,
    /// Hash of the arm name.
    pub name_hash: u64,
    /// Total successes.
    pub successes: u64,
    /// Total failures.
    pub failures: u64,
    /// Total trials (successes + failures).
    pub total_trials: u64,
    /// Expected reward (Beta distribution mean: alpha/(alpha+beta)).
    pub expected_reward: f64,
    /// Feature dimension: always 4 (successes, failures, total, reward).
    pub feature_dim: usize,
}

/// Convert a bandit arm into an ML reward signal.
#[inline]
#[must_use]
pub fn experiment_bandit_to_ml_reward(arm: &BanditArm) -> ExperimentBanditMlReward {
    let name_hash = fnv1a(arm.name.as_bytes());
    let expected_reward = arm.expected_reward();
    let total_trials = arm.total_trials();

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&name_hash.to_le_bytes());
    key[8..16].copy_from_slice(&arm.successes.to_le_bytes());
    key[16..24].copy_from_slice(&arm.failures.to_le_bytes());
    key[24..32].copy_from_slice(&expected_reward.to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&total_trials.to_le_bytes());

    ExperimentBanditMlReward {
        content_hash: fnv1a(&key),
        name_hash,
        successes: arm.successes,
        failures: arm.failures,
        total_trials,
        expected_reward,
        feature_dim: 4,
    }
}

// ── Bridge 4: Z-test result → Analytics event ────────────────────────

/// A z-test statistical result converted into an Analytics event.
///
/// Captures the z-score, p-value, and significance for the Analytics
/// pipeline to track experiment outcomes over time.
pub struct ExperimentZTestAnalytics {
    /// FNV-1a hash over z_score, p_value, is_significant bytes.
    pub content_hash: u64,
    /// The z-score from the proportions test.
    pub z_score: f64,
    /// Two-sided p-value.
    pub p_value: f64,
    /// Whether the result is significant at alpha=0.05.
    pub is_significant: bool,
    /// Significance level used.
    pub alpha: f64,
    /// Sample sizes: [control_n, treatment_n].
    pub sample_sizes: [u64; 2],
    /// Metric name hash for Analytics pipeline registration.
    pub metric_name_hash: u64,
}

/// Convert a z-test result into an Analytics event.
#[inline]
#[must_use]
pub fn experiment_z_test_to_analytics(
    successes1: u64,
    total1: u64,
    successes2: u64,
    total2: u64,
) -> ExperimentZTestAnalytics {
    let z = z_test_proportions(successes1, total1, successes2, total2);
    let p = p_value_from_z(z);
    let sig = p < 0.05;
    let metric_name_hash = fnv1a(b"experiment.ztest");

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&z.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&p.to_bits().to_le_bytes());
    key[16] = sig as u8;
    key[17..25].copy_from_slice(&total1.to_le_bytes());
    key[25..33].copy_from_slice(&total2.to_le_bytes());

    ExperimentZTestAnalytics {
        content_hash: fnv1a(&key),
        z_score: z,
        p_value: p,
        is_significant: sig,
        alpha: 0.05,
        sample_sizes: [total1, total2],
        metric_name_hash,
    }
}

// ── Bridge 5: Experiment result → Cache ──────────────────────────────

/// An experiment conversion result converted into a Cache entry.
///
/// Caches the conversion rate and confidence interval for a variant.
/// TTL is branchless-adjusted: low-traffic variants (< 100 visitors)
/// get shorter TTL as their statistics are less stable.
pub struct ExperimentResultCache {
    /// FNV-1a hash over variant hash, visitors, rate, ci_lower, ci_upper bytes.
    pub content_hash: u64,
    /// Hash of the variant name.
    pub variant_hash: u64,
    /// Conversion rate.
    pub rate: f64,
    /// Wilson CI lower bound.
    pub ci_lower: f64,
    /// Wilson CI upper bound.
    pub ci_upper: f64,
    /// TTL in seconds. Low-traffic variants get shorter TTL.
    pub ttl_secs: u32,
    /// Cache key hash for direct lookup.
    pub cache_key: u64,
}

/// Convert an experiment result into a Cache entry.
#[inline]
#[must_use]
pub fn experiment_result_to_cache(data: &ConversionData) -> ExperimentResultCache {
    let variant_hash = fnv1a(data.variant.as_bytes());
    let rate = data.rate();
    let (ci_lower, ci_upper) = data.confidence_interval(1.96);

    // Branchless TTL: low traffic (< 100 visitors) gets 120s less
    let low_traffic = (data.visitors < 100) as u32;
    let ttl_secs: u32 = 600 - low_traffic * 120;

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&variant_hash.to_le_bytes());
    key[8..16].copy_from_slice(&data.visitors.to_le_bytes());
    key[16..24].copy_from_slice(&rate.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&ci_lower.to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&ci_upper.to_bits().to_le_bytes());

    let cache_key = fnv1a(&key);

    ExperimentResultCache {
        content_hash: fnv1a(&key),
        variant_hash,
        rate,
        ci_lower,
        ci_upper,
        ttl_secs,
        cache_key,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_experiment::{BanditArm, ConversionData, Variant};

    // ── Bridge 1: variant → ml feature ───────────────────────────────

    #[test]
    fn test_experiment_variant_to_ml_feature() {
        let v = Variant {
            name: String::from("control"),
            traffic_pct: 0.5,
        };
        let feat = experiment_variant_to_ml_feature(&v);
        assert_ne!(feat.content_hash, 0);
        assert_ne!(feat.name_hash, 0);
        assert!((feat.traffic_pct - 0.5).abs() < 1e-10);
        assert_eq!(feat.feature_dim, 3);
    }

    #[test]
    fn test_experiment_variant_to_ml_feature_deterministic() {
        let v = Variant {
            name: String::from("treatment"),
            traffic_pct: 0.3,
        };
        let f1 = experiment_variant_to_ml_feature(&v);
        let f2 = experiment_variant_to_ml_feature(&v);
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    // ── Bridge 2: conversion → analytics ─────────────────────────────

    #[test]
    fn test_experiment_conversion_to_analytics() {
        let data = ConversionData {
            variant: String::from("A"),
            visitors: 1000,
            conversions: 100,
        };
        let analytics = experiment_conversion_to_analytics(&data);
        assert_ne!(analytics.content_hash, 0);
        assert!((analytics.rate - 0.1).abs() < 0.001);
        assert!(analytics.ci_lower < 0.1);
        assert!(analytics.ci_upper > 0.1);
        assert_ne!(analytics.metric_name_hash, 0);
    }

    #[test]
    fn test_experiment_conversion_to_analytics_zero_visitors() {
        let data = ConversionData {
            variant: String::from("B"),
            visitors: 0,
            conversions: 0,
        };
        let analytics = experiment_conversion_to_analytics(&data);
        assert!((analytics.rate - 0.0).abs() < 1e-10);
    }

    // ── Bridge 3: bandit → ml reward ─────────────────────────────────

    #[test]
    fn test_experiment_bandit_to_ml_reward() {
        let mut arm = BanditArm::new("arm_a");
        arm.successes = 10;
        arm.failures = 10;
        let reward = experiment_bandit_to_ml_reward(&arm);
        assert_ne!(reward.content_hash, 0);
        assert_eq!(reward.successes, 10);
        assert_eq!(reward.failures, 10);
        assert_eq!(reward.total_trials, 20);
        // Beta(11,11) mean = 0.5
        assert!((reward.expected_reward - 0.5).abs() < 0.01);
        assert_eq!(reward.feature_dim, 4);
    }

    // ── Bridge 4: z-test → analytics ─────────────────────────────────

    #[test]
    fn test_experiment_z_test_to_analytics_significant() {
        let analytics = experiment_z_test_to_analytics(100, 1000, 200, 1000);
        assert_ne!(analytics.content_hash, 0);
        assert!(analytics.z_score.abs() > 1.0);
        assert!(analytics.p_value < 0.05);
        assert!(analytics.is_significant);
        assert_eq!(analytics.sample_sizes, [1000, 1000]);
    }

    #[test]
    fn test_experiment_z_test_to_analytics_not_significant() {
        let analytics = experiment_z_test_to_analytics(100, 1000, 105, 1000);
        assert!(!analytics.is_significant);
    }

    // ── Bridge 5: result → cache ─────────────────────────────────────

    #[test]
    fn test_experiment_result_to_cache_high_traffic() {
        let data = ConversionData {
            variant: String::from("A"),
            visitors: 10000,
            conversions: 500,
        };
        let cache = experiment_result_to_cache(&data);
        assert_ne!(cache.content_hash, 0);
        // High traffic → full TTL
        assert_eq!(cache.ttl_secs, 600);
        assert_ne!(cache.cache_key, 0);
    }

    #[test]
    fn test_experiment_result_to_cache_low_traffic_ttl() {
        let data = ConversionData {
            variant: String::from("B"),
            visitors: 50,
            conversions: 5,
        };
        let cache = experiment_result_to_cache(&data);
        // Branchless: 600 - 1 * 120 = 480
        assert_eq!(cache.ttl_secs, 480);
    }

    #[test]
    fn test_experiment_result_to_cache_deterministic() {
        let data = ConversionData {
            variant: String::from("C"),
            visitors: 500,
            conversions: 50,
        };
        let c1 = experiment_result_to_cache(&data);
        let c2 = experiment_result_to_cache(&data);
        assert_eq!(c1.content_hash, c2.content_hash);
        assert_eq!(c1.cache_key, c2.cache_key);
    }
}
