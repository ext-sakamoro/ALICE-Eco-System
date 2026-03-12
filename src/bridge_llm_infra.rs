//! LLM infrastructure bridges — ALICE-LLM ↔ TRT, Queue, Auth, Crypto, Container
//!
//! 5 bridges connecting LLM inference to infrastructure services.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LLM → TRT (TensorRT optimization) ────────────────────────

/// TensorRT optimization profile for LLM inference.
pub struct LlmTrtProfile {
    /// Content hash over the TRT profile.
    pub content_hash: u64,
    /// Number of model layers.
    pub num_layers: u32,
    /// Hidden dimension.
    pub hidden_dim: u32,
    /// Maximum batch size for TRT engine.
    pub max_batch_size: u32,
    /// Precision mode (0=FP32, 1=FP16, 2=INT8, 3=INT4).
    pub precision: u8,
    /// Estimated speedup over CPU inference.
    pub estimated_speedup: f32,
    /// Whether wgpu-native GPU inference is available (ALICE-LLM built-in).
    pub wgpu_native: bool,
}

/// Build a TRT optimization profile from LLM model config.
///
/// Speedup estimate: FP32=2x, FP16=8x, INT8=16x, INT4=24x (rough GPU vs CPU).
/// `wgpu_native`: true when ALICE-LLM's built-in wgpu GPU engine is available.
#[inline]
#[must_use]
pub fn llm_to_trt_profile(
    num_layers: u32,
    hidden_dim: u32,
    max_batch_size: u32,
    precision: u8,
    wgpu_native: bool,
) -> LlmTrtProfile {
    let mut buf = [0u8; 14];
    buf[0..4].copy_from_slice(&num_layers.to_le_bytes());
    buf[4..8].copy_from_slice(&hidden_dim.to_le_bytes());
    buf[8..12].copy_from_slice(&max_batch_size.to_le_bytes());
    buf[12] = precision;
    buf[13] = wgpu_native as u8;
    let estimated_speedup = match precision {
        0 => 2.0,
        1 => 8.0,
        2 => 16.0,
        3 => 24.0,
        _ => 1.0,
    };
    LlmTrtProfile {
        content_hash: fnv1a(&buf),
        num_layers,
        hidden_dim,
        max_batch_size,
        precision,
        estimated_speedup,
        wgpu_native,
    }
}

// ── Bridge 2: LLM → Queue (request queuing) ────────────────────────────

/// Inference request for ALICE-Queue scheduling.
pub struct LlmQueueRequest {
    /// Content hash over the queue request.
    pub content_hash: u64,
    /// Request priority (0=low, 1=normal, 2=high, 3=critical).
    pub priority: u8,
    /// Estimated token count for this request.
    pub estimated_tokens: u32,
    /// Maximum wait time in milliseconds.
    pub max_wait_ms: u32,
    /// Model identifier hash.
    pub model_id_hash: u64,
    /// Whether this is a streaming request.
    pub streaming: bool,
}

/// Build a queue request from LLM inference parameters.
#[inline]
#[must_use]
pub fn llm_to_queue_request(
    priority: u8,
    estimated_tokens: u32,
    max_wait_ms: u32,
    model_id_hash: u64,
    streaming: bool,
) -> LlmQueueRequest {
    let mut buf = [0u8; 18];
    buf[0] = priority;
    buf[1..5].copy_from_slice(&estimated_tokens.to_le_bytes());
    buf[5..9].copy_from_slice(&max_wait_ms.to_le_bytes());
    buf[9..17].copy_from_slice(&model_id_hash.to_le_bytes());
    buf[17] = streaming as u8;
    LlmQueueRequest {
        content_hash: fnv1a(&buf),
        priority,
        estimated_tokens,
        max_wait_ms,
        model_id_hash,
        streaming,
    }
}

// ── Bridge 3: LLM → Auth (access control) ──────────────────────────────

/// Access control decision for LLM inference request.
pub struct LlmAuthDecision {
    /// Content hash over the auth decision.
    pub content_hash: u64,
    /// User identifier hash.
    pub user_hash: u64,
    /// Model identifier hash.
    pub model_hash: u64,
    /// Token quota remaining.
    pub quota_remaining: u64,
    /// Whether access is granted.
    pub granted: bool,
    /// Rate limit tokens per minute.
    pub rate_limit_tpm: u32,
}

/// Build an auth decision for LLM inference access control.
///
/// Access is granted when quota_remaining > 0.
#[inline]
#[must_use]
pub fn llm_to_auth_decision(
    user_hash: u64,
    model_hash: u64,
    quota_remaining: u64,
    rate_limit_tpm: u32,
) -> LlmAuthDecision {
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&user_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&model_hash.to_le_bytes());
    buf[16..24].copy_from_slice(&quota_remaining.to_le_bytes());
    buf[24..28].copy_from_slice(&rate_limit_tpm.to_le_bytes());
    let granted = quota_remaining > 0;
    LlmAuthDecision {
        content_hash: fnv1a(&buf),
        user_hash,
        model_hash,
        quota_remaining,
        granted,
        rate_limit_tpm,
    }
}

// ── Bridge 4: LLM → Crypto (model encryption) ──────────────────────────

/// Encrypted model descriptor for secure LLM deployment.
pub struct LlmCryptoDescriptor {
    /// Content hash over the crypto descriptor.
    pub content_hash: u64,
    /// Original model size in bytes.
    pub model_size_bytes: u64,
    /// Encryption algorithm (0=AES-256-GCM, 1=ChaCha20-Poly1305).
    pub algorithm: u8,
    /// Key derivation hash.
    pub key_hash: u64,
    /// Encrypted payload overhead in bytes.
    pub overhead_bytes: u64,
    /// Whether hardware-accelerated encryption is used.
    pub hw_accelerated: bool,
}

/// Build a crypto descriptor for encrypted model storage.
///
/// Overhead: AES-256-GCM = 28 bytes/chunk, ChaCha20 = 40 bytes/chunk.
#[inline]
#[must_use]
pub fn llm_to_crypto_descriptor(
    model_size_bytes: u64,
    algorithm: u8,
    key_hash: u64,
    hw_accelerated: bool,
) -> LlmCryptoDescriptor {
    let mut buf = [0u8; 18];
    buf[0..8].copy_from_slice(&model_size_bytes.to_le_bytes());
    buf[8] = algorithm;
    buf[9..17].copy_from_slice(&key_hash.to_le_bytes());
    buf[17] = hw_accelerated as u8;
    let chunk_count = (model_size_bytes + 65535) / 65536;
    let per_chunk = if algorithm == 0 { 28u64 } else { 40 };
    let overhead_bytes = chunk_count * per_chunk;
    LlmCryptoDescriptor {
        content_hash: fnv1a(&buf),
        model_size_bytes,
        algorithm,
        key_hash,
        overhead_bytes,
        hw_accelerated,
    }
}

// ── Bridge 5: LLM → Container (containerised deployment) ───────────────

/// Container deployment descriptor for LLM serving.
pub struct LlmContainerSpec {
    /// Content hash over the container spec.
    pub content_hash: u64,
    /// Model size in bytes.
    pub model_size_bytes: u64,
    /// Required memory in bytes (model + KV cache + overhead).
    pub required_memory_bytes: u64,
    /// Number of CPU cores required.
    pub cpu_cores: u32,
    /// Whether GPU is required.
    pub gpu_required: bool,
    /// Container image size estimate in bytes.
    pub image_size_bytes: u64,
}

/// Build a container spec from LLM model requirements.
///
/// Memory estimate: model_size * 1.3 (KV cache + runtime overhead).
/// Image size: model_size + 200MB (runtime + dependencies).
#[inline]
#[must_use]
pub fn llm_to_container_spec(
    model_size_bytes: u64,
    cpu_cores: u32,
    gpu_required: bool,
) -> LlmContainerSpec {
    let mut buf = [0u8; 13];
    buf[0..8].copy_from_slice(&model_size_bytes.to_le_bytes());
    buf[8..12].copy_from_slice(&cpu_cores.to_le_bytes());
    buf[12] = gpu_required as u8;
    let required_memory_bytes = (model_size_bytes as f64 * 1.3) as u64;
    let image_size_bytes = model_size_bytes + 200_000_000;
    LlmContainerSpec {
        content_hash: fnv1a(&buf),
        model_size_bytes,
        required_memory_bytes,
        cpu_cores,
        gpu_required,
        image_size_bytes,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trt_profile_int8() {
        let p = llm_to_trt_profile(32, 4096, 8, 2, false);
        assert_ne!(p.content_hash, 0);
        assert_eq!(p.precision, 2);
        assert!((p.estimated_speedup - 16.0).abs() < 0.01);
        assert!(!p.wgpu_native);
    }

    #[test]
    fn test_trt_profile_wgpu_native() {
        let p = llm_to_trt_profile(16, 2048, 4, 3, true);
        assert!(p.wgpu_native);
        assert!((p.estimated_speedup - 24.0).abs() < 0.01);
    }

    #[test]
    fn test_trt_profile_determinism() {
        let a = llm_to_trt_profile(16, 2048, 1, 3, true);
        let b = llm_to_trt_profile(16, 2048, 1, 3, true);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_queue_request_streaming() {
        let r = llm_to_queue_request(2, 512, 5000, 0xbeef, true);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.priority, 2);
        assert!(r.streaming);
    }

    #[test]
    fn test_auth_decision_granted() {
        let d = llm_to_auth_decision(0x1234, 0x5678, 1000, 60000);
        assert_ne!(d.content_hash, 0);
        assert!(d.granted);
    }

    #[test]
    fn test_auth_decision_denied() {
        let d = llm_to_auth_decision(0x1234, 0x5678, 0, 60000);
        assert!(!d.granted);
    }

    #[test]
    fn test_crypto_aes_overhead() {
        // 770MB model, 65536-byte chunks → ~11750 chunks * 28 bytes
        let c = llm_to_crypto_descriptor(770_000_000, 0, 0xaaaa, true);
        assert_ne!(c.content_hash, 0);
        assert!(c.overhead_bytes > 0);
        assert!(c.hw_accelerated);
    }

    #[test]
    fn test_container_spec_memory() {
        let s = llm_to_container_spec(5_000_000_000, 8, true);
        assert_ne!(s.content_hash, 0);
        assert!(s.required_memory_bytes > 5_000_000_000);
        assert!(s.gpu_required);
        assert_eq!(s.image_size_bytes, 5_200_000_000);
    }

    #[test]
    fn test_container_spec_cpu_only() {
        let s = llm_to_container_spec(770_000_000, 4, false);
        assert!(!s.gpu_required);
    }
}
