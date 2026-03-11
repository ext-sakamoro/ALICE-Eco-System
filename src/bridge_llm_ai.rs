//! LLM AI bridges — ALICE-LLM ↔ Neural, ML, Embedding, RAG, NLP
//!
//! 5 bridges connecting LLM inference to AI/ML subsystems.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LLM → Neural (neural network primitives) ─────────────────

/// Neural network layer descriptor extracted from LLM model.
pub struct LlmNeuralLayer {
    /// Content hash over the layer descriptor.
    pub content_hash: u64,
    /// Layer index in the model.
    pub layer_idx: u32,
    /// Layer type (0=attention, 1=ffn, 2=norm, 3=embedding).
    pub layer_type: u8,
    /// Number of parameters in this layer.
    pub param_count: u64,
    /// Quantization bits per parameter.
    pub quant_bits: u8,
    /// Output activation L2 norm (for diagnostics).
    pub activation_l2: f32,
}

/// Build a neural layer descriptor from LLM layer metadata.
#[inline]
#[must_use]
pub fn llm_to_neural_layer(
    layer_idx: u32,
    layer_type: u8,
    param_count: u64,
    quant_bits: u8,
    activation_l2: f32,
) -> LlmNeuralLayer {
    let mut buf = [0u8; 18];
    buf[0..4].copy_from_slice(&layer_idx.to_le_bytes());
    buf[4] = layer_type;
    buf[5..13].copy_from_slice(&param_count.to_le_bytes());
    buf[13] = quant_bits;
    buf[14..18].copy_from_slice(&activation_l2.to_bits().to_le_bytes());
    LlmNeuralLayer {
        content_hash: fnv1a(&buf),
        layer_idx,
        layer_type,
        param_count,
        quant_bits,
        activation_l2,
    }
}

// ── Bridge 2: LLM → ML (training pipeline) ─────────────────────────────

/// Training job descriptor for fine-tuning an LLM via ALICE-ML.
pub struct LlmMlTrainingJob {
    /// Content hash over the training job descriptor.
    pub content_hash: u64,
    /// Model parameter count (millions).
    pub model_params_m: u64,
    /// Number of trainable layers (LoRA/QLoRA subset).
    pub trainable_layers: u32,
    /// Training dataset size in tokens.
    pub dataset_tokens: u64,
    /// Estimated training FLOPS (tera).
    pub estimated_tflops: f64,
    /// Whether quantization-aware training is enabled.
    pub qat_enabled: bool,
}

/// Build a training job descriptor from LLM model and dataset metadata.
///
/// FLOPS estimate: ~6 * params * tokens (Chinchilla scaling).
#[inline]
#[must_use]
pub fn llm_to_ml_training_job(
    model_params_m: u64,
    trainable_layers: u32,
    dataset_tokens: u64,
    qat_enabled: bool,
) -> LlmMlTrainingJob {
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&model_params_m.to_le_bytes());
    buf[8..12].copy_from_slice(&trainable_layers.to_le_bytes());
    buf[12..20].copy_from_slice(&dataset_tokens.to_le_bytes());
    buf[20] = qat_enabled as u8;
    // 6 * params_M * 1e6 * tokens / 1e12 = 6 * params_M * tokens * 1e-6
    let estimated_tflops = 6.0 * model_params_m as f64 * dataset_tokens as f64 * 1e-6;
    LlmMlTrainingJob {
        content_hash: fnv1a(&buf),
        model_params_m,
        trainable_layers,
        dataset_tokens,
        estimated_tflops,
        qat_enabled,
    }
}

// ── Bridge 3: LLM → Embedding (vector generation) ──────────────────────

/// Embedding vector descriptor from LLM hidden states.
pub struct LlmEmbeddingVector {
    /// Content hash over the embedding descriptor.
    pub content_hash: u64,
    /// Embedding dimensionality.
    pub dim: u32,
    /// Source token count (input length).
    pub token_count: u32,
    /// Pooling method (0=last_token, 1=mean, 2=cls).
    pub pooling: u8,
    /// L2 norm of the embedding vector.
    pub l2_norm: f32,
    /// Model hidden dimension.
    pub hidden_dim: u32,
}

/// Build an embedding vector descriptor from LLM hidden state output.
#[inline]
#[must_use]
pub fn llm_to_embedding_vector(
    dim: u32,
    token_count: u32,
    pooling: u8,
    l2_norm: f32,
    hidden_dim: u32,
) -> LlmEmbeddingVector {
    let mut buf = [0u8; 17];
    buf[0..4].copy_from_slice(&dim.to_le_bytes());
    buf[4..8].copy_from_slice(&token_count.to_le_bytes());
    buf[8] = pooling;
    buf[9..13].copy_from_slice(&l2_norm.to_bits().to_le_bytes());
    buf[13..17].copy_from_slice(&hidden_dim.to_le_bytes());
    LlmEmbeddingVector {
        content_hash: fnv1a(&buf),
        dim,
        token_count,
        pooling,
        l2_norm,
        hidden_dim,
    }
}

// ── Bridge 4: LLM → RAG (retrieval-augmented generation) ────────────────

/// RAG query descriptor for retrieval-augmented generation.
pub struct LlmRagQuery {
    /// Content hash over the RAG query.
    pub content_hash: u64,
    /// Query embedding dimension.
    pub query_dim: u32,
    /// Number of retrieved context chunks.
    pub num_chunks: u32,
    /// Total context token count (query + retrieved).
    pub context_tokens: u64,
    /// Maximum generation tokens.
    pub max_gen_tokens: u32,
    /// Whether re-ranking is applied to retrieved chunks.
    pub rerank_enabled: bool,
}

/// Build a RAG query descriptor from query and retrieval metadata.
#[inline]
#[must_use]
pub fn llm_to_rag_query(
    query_dim: u32,
    num_chunks: u32,
    context_tokens: u64,
    max_gen_tokens: u32,
    rerank_enabled: bool,
) -> LlmRagQuery {
    let mut buf = [0u8; 21];
    buf[0..4].copy_from_slice(&query_dim.to_le_bytes());
    buf[4..8].copy_from_slice(&num_chunks.to_le_bytes());
    buf[8..16].copy_from_slice(&context_tokens.to_le_bytes());
    buf[16..20].copy_from_slice(&max_gen_tokens.to_le_bytes());
    buf[20] = rerank_enabled as u8;
    LlmRagQuery {
        content_hash: fnv1a(&buf),
        query_dim,
        num_chunks,
        context_tokens,
        max_gen_tokens,
        rerank_enabled,
    }
}

// ── Bridge 5: LLM → NLP (natural language preprocessing) ───────────────

/// NLP preprocessing result for LLM input.
pub struct LlmNlpPreprocess {
    /// Content hash over the preprocessing result.
    pub content_hash: u64,
    /// Input text length in characters.
    pub char_count: u32,
    /// Token count after tokenization.
    pub token_count: u32,
    /// Detected language code hash.
    pub lang_hash: u64,
    /// Compression ratio (tokens / chars).
    pub compression_ratio: f32,
    /// Whether the text was truncated to fit context window.
    pub truncated: bool,
}

/// Build an NLP preprocessing result from tokenizer output.
#[inline]
#[must_use]
pub fn llm_to_nlp_preprocess(
    char_count: u32,
    token_count: u32,
    lang_hash: u64,
    truncated: bool,
) -> LlmNlpPreprocess {
    let mut buf = [0u8; 17];
    buf[0..4].copy_from_slice(&char_count.to_le_bytes());
    buf[4..8].copy_from_slice(&token_count.to_le_bytes());
    buf[8..16].copy_from_slice(&lang_hash.to_le_bytes());
    buf[16] = truncated as u8;
    let compression_ratio = if char_count > 0 {
        token_count as f32 / char_count as f32
    } else {
        0.0
    };
    LlmNlpPreprocess {
        content_hash: fnv1a(&buf),
        char_count,
        token_count,
        lang_hash,
        compression_ratio,
        truncated,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neural_layer_hash() {
        let l = llm_to_neural_layer(0, 0, 16_777_216, 4, 36.4);
        assert_ne!(l.content_hash, 0);
        assert_eq!(l.layer_idx, 0);
        assert_eq!(l.layer_type, 0);
    }

    #[test]
    fn test_neural_layer_determinism() {
        let a = llm_to_neural_layer(5, 1, 1_000_000, 8, 10.0);
        let b = llm_to_neural_layer(5, 1, 1_000_000, 8, 10.0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_ml_training_job_flops() {
        let j = llm_to_ml_training_job(7_000, 32, 1_000_000_000, false);
        assert_ne!(j.content_hash, 0);
        // 6 * 7000 * 1e9 * 1e-6 = 42_000_000 TFLOPS
        assert!((j.estimated_tflops - 42_000_000.0).abs() < 1.0);
        assert!(!j.qat_enabled);
    }

    #[test]
    fn test_embedding_vector_fields() {
        let e = llm_to_embedding_vector(2048, 128, 1, 45.2, 2048);
        assert_ne!(e.content_hash, 0);
        assert_eq!(e.dim, 2048);
        assert_eq!(e.pooling, 1);
    }

    #[test]
    fn test_rag_query_rerank() {
        let q = llm_to_rag_query(2048, 5, 4096, 512, true);
        assert_ne!(q.content_hash, 0);
        assert!(q.rerank_enabled);
        assert_eq!(q.num_chunks, 5);
    }

    #[test]
    fn test_rag_query_no_rerank() {
        let q = llm_to_rag_query(768, 3, 2048, 256, false);
        assert!(!q.rerank_enabled);
    }

    #[test]
    fn test_nlp_preprocess_ratio() {
        let p = llm_to_nlp_preprocess(1000, 250, 0xaabb, false);
        assert_ne!(p.content_hash, 0);
        assert!((p.compression_ratio - 0.25).abs() < 0.01);
        assert!(!p.truncated);
    }

    #[test]
    fn test_nlp_preprocess_truncated() {
        let p = llm_to_nlp_preprocess(50000, 8192, 0x1234, true);
        assert!(p.truncated);
    }
}
