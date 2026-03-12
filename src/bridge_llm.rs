//! LLM bridges — ALICE-LLM ↔ DB, Cache, Analytics, API, Monitor
//!
//! 5 bridges connecting large-language-model inference to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LLM → DB (inference log) ──────────────────────────────────

/// Inference log record for ALICE-DB persistence.
pub struct LlmDbRecord {
    /// Content hash over the log entry.
    pub content_hash: u64,
    /// Total tokens generated in this inference.
    pub token_count: u64,
    /// Vocabulary size of the model.
    pub vocab_size: u32,
    /// Number of model parameters (millions).
    pub model_params_m: u64,
    /// End-to-end inference latency in microseconds.
    pub latency_us: u64,
    /// Model identifier hash.
    pub model_id_hash: u64,
}

/// Serialize an LLM inference result for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn llm_to_db_record(
    token_count: u64,
    vocab_size: u32,
    model_params_m: u64,
    latency_us: u64,
    model_id_hash: u64,
) -> LlmDbRecord {
    let mut buf = [0u8; 36];
    buf[0..8].copy_from_slice(&token_count.to_le_bytes());
    buf[8..12].copy_from_slice(&vocab_size.to_le_bytes());
    buf[12..20].copy_from_slice(&model_params_m.to_le_bytes());
    buf[20..28].copy_from_slice(&latency_us.to_le_bytes());
    buf[28..36].copy_from_slice(&model_id_hash.to_le_bytes());
    LlmDbRecord {
        content_hash: fnv1a(&buf),
        token_count,
        vocab_size,
        model_params_m,
        latency_us,
        model_id_hash,
    }
}

// ── Bridge 2: LLM → Cache (KV cache) ────────────────────────────────────

/// KV cache entry for ALICE-Cache.
pub struct LlmCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of tokens in the cached context.
    pub token_count: u64,
    /// Number of key-value head pairs cached.
    pub kv_head_count: u32,
    /// TTL for this KV cache entry in seconds.
    pub ttl_secs: u32,
    /// Whether the entry is a prefix-cache hit.
    pub is_prefix_hit: bool,
}

/// Build a KV cache entry for ALICE-Cache.
///
/// Prefix-cache hits receive a longer TTL (300 s vs 60 s) because they
/// are more likely to be reused by subsequent requests sharing the same prompt.
#[inline]
#[must_use]
pub fn llm_to_cache_entry(
    token_count: u64,
    kv_head_count: u32,
    is_prefix_hit: bool,
) -> LlmCacheEntry {
    let mut buf = [0u8; 13];
    buf[0..8].copy_from_slice(&token_count.to_le_bytes());
    buf[8..12].copy_from_slice(&kv_head_count.to_le_bytes());
    buf[12] = is_prefix_hit as u8;
    let is_hit = is_prefix_hit as u32;
    let ttl_secs = 60 + is_hit * 240;
    LlmCacheEntry {
        content_hash: fnv1a(&buf),
        token_count,
        kv_head_count,
        ttl_secs,
        is_prefix_hit,
    }
}

// ── Bridge 3: LLM → Analytics (inference metrics) ───────────────────────

/// Inference metrics for ALICE-Analytics ingestion.
pub struct LlmAnalyticsMetrics {
    /// Content hash over the metric tuple.
    pub content_hash: u64,
    /// Total tokens generated in the reporting period.
    pub token_count: u64,
    /// Throughput in tokens per second.
    pub throughput_tps: f64,
    /// Average inference latency in microseconds.
    pub avg_latency_us: f64,
    /// Number of requests served.
    pub request_count: u64,
    /// Number of model parameters (millions).
    pub model_params_m: u64,
}

/// Build inference metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn llm_to_analytics_metrics(
    token_count: u64,
    request_count: u64,
    total_latency_us: u64,
    model_params_m: u64,
) -> LlmAnalyticsMetrics {
    let rcp = 1.0 / request_count.max(1) as f64;
    let avg_latency_us = total_latency_us as f64 * rcp;
    let total_secs = total_latency_us as f64 * 1e-6;
    let throughput_tps = token_count as f64 / total_secs.max(1e-9);
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&token_count.to_le_bytes());
    buf[8..16].copy_from_slice(&request_count.to_le_bytes());
    buf[16..24].copy_from_slice(&model_params_m.to_le_bytes());
    LlmAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        token_count,
        throughput_tps,
        avg_latency_us,
        request_count,
        model_params_m,
    }
}

// ── Bridge 4: LLM → API (serving) ───────────────────────────────────────

/// Serving response for ALICE-API.
pub struct LlmApiResponse {
    /// Content hash over the response payload.
    pub content_hash: u64,
    /// Number of tokens in the generated output.
    pub token_count: u64,
    /// Inference latency in microseconds.
    pub latency_us: u64,
    /// HTTP status code.
    pub status_code: u16,
    /// Whether the output was truncated by the max-token limit.
    pub truncated: bool,
    /// Model identifier hash.
    pub model_id_hash: u64,
}

/// Build an API serving response from LLM inference output.
#[inline]
#[must_use]
pub fn llm_to_api_response(
    token_count: u64,
    latency_us: u64,
    truncated: bool,
    model_id_hash: u64,
) -> LlmApiResponse {
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&token_count.to_le_bytes());
    buf[8..16].copy_from_slice(&latency_us.to_le_bytes());
    buf[16..24].copy_from_slice(&model_id_hash.to_le_bytes());
    buf[24] = truncated as u8;
    let status_code = if truncated { 206 } else { 200 };
    LlmApiResponse {
        content_hash: fnv1a(&buf),
        token_count,
        latency_us,
        status_code,
        truncated,
        model_id_hash,
    }
}

// ── Bridge 5: LLM → Monitor (health) ────────────────────────────────────

/// Health snapshot from LLM serving for ALICE-Monitor.
pub struct LlmMonitorHealth {
    /// Content hash over the health snapshot.
    pub content_hash: u64,
    /// Throughput in tokens per second.
    pub throughput_tps: f64,
    /// Average inference latency in microseconds.
    pub avg_latency_us: f64,
    /// Number of failed inference requests in the window.
    pub error_count: u64,
    /// GPU memory utilisation in the range [0.0, 1.0].
    pub gpu_mem_utilisation: f32,
    /// GPU inference throughput in tokens per second (0.0 if CPU-only).
    pub gpu_inference_tps: f32,
    /// Whether the serving instance is considered healthy.
    pub is_healthy: bool,
}

/// Build a monitor health snapshot from LLM serving metrics.
///
/// `gpu_inference_tps`: GPU-accelerated inference throughput (0.0 if CPU-only).
#[inline]
#[must_use]
pub fn llm_to_monitor_health(
    throughput_tps: f64,
    avg_latency_us: f64,
    error_count: u64,
    gpu_mem_utilisation: f32,
    gpu_inference_tps: f32,
) -> LlmMonitorHealth {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&throughput_tps.to_bits().to_le_bytes());
    buf[8..16].copy_from_slice(&avg_latency_us.to_bits().to_le_bytes());
    buf[16..24].copy_from_slice(&error_count.to_le_bytes());
    buf[24..28].copy_from_slice(&gpu_mem_utilisation.to_bits().to_le_bytes());
    buf[28..32].copy_from_slice(&gpu_inference_tps.to_bits().to_le_bytes());
    let is_healthy = error_count == 0 && gpu_mem_utilisation < 0.95;
    LlmMonitorHealth {
        content_hash: fnv1a(&buf),
        throughput_tps,
        avg_latency_us,
        error_count,
        gpu_mem_utilisation,
        gpu_inference_tps,
        is_healthy,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_to_db_record_hash_nonzero() {
        let rec = llm_to_db_record(512, 32_000, 7_000, 250_000, 0xdeadbeef);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_llm_to_db_record_fields() {
        let rec = llm_to_db_record(128, 50_000, 13_000, 100_000, 0x1234);
        assert_eq!(rec.token_count, 128);
        assert_eq!(rec.vocab_size, 50_000);
        assert_eq!(rec.model_params_m, 13_000);
        assert_eq!(rec.latency_us, 100_000);
        assert_eq!(rec.model_id_hash, 0x1234);
    }

    #[test]
    fn test_llm_to_cache_entry_miss_ttl() {
        let entry = llm_to_cache_entry(256, 32, false);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 60);
        assert!(!entry.is_prefix_hit);
    }

    #[test]
    fn test_llm_to_cache_entry_hit_ttl() {
        let entry = llm_to_cache_entry(256, 32, true);
        assert_eq!(entry.ttl_secs, 300);
        assert!(entry.is_prefix_hit);
    }

    #[test]
    fn test_llm_to_analytics_metrics_throughput() {
        // 1000 tokens, 10 requests, total latency 1 s = 1_000_000 µs → tps ≈ 1000.
        let m = llm_to_analytics_metrics(1_000, 10, 1_000_000, 7_000);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.token_count, 1_000);
        assert_eq!(m.request_count, 10);
        assert!((m.avg_latency_us - 100_000.0).abs() < 1.0);
        assert!((m.throughput_tps - 1_000.0).abs() < 1.0);
    }

    #[test]
    fn test_llm_to_analytics_metrics_zero_requests() {
        let m = llm_to_analytics_metrics(0, 0, 0, 7_000);
        assert_eq!(m.avg_latency_us, 0.0);
    }

    #[test]
    fn test_llm_to_api_response_ok() {
        let resp = llm_to_api_response(64, 50_000, false, 0xbeef);
        assert_ne!(resp.content_hash, 0);
        assert_eq!(resp.status_code, 200);
        assert!(!resp.truncated);
    }

    #[test]
    fn test_llm_to_monitor_health_unhealthy() {
        let h = llm_to_monitor_health(10.0, 500_000.0, 5, 0.98, 0.0);
        assert_ne!(h.content_hash, 0);
        assert!(!h.is_healthy);
        assert_eq!(h.error_count, 5);
        assert!((h.gpu_inference_tps - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_llm_to_monitor_health_gpu_inference() {
        let h = llm_to_monitor_health(12.5, 80_000.0, 0, 0.45, 12.5);
        assert!(h.is_healthy);
        assert!((h.gpu_inference_tps - 12.5).abs() < 0.01);
    }
}
