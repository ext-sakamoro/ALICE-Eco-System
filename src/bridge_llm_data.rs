//! LLM data bridges — ALICE-LLM ↔ Text, Search, CDN, VectorDB, Semantic-Telemetry
//!
//! 5 bridges connecting LLM inference to data processing and distribution systems.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LLM → Text (text compression) ────────────────────────────

/// Text compression result for LLM-generated output.
pub struct LlmTextCompressed {
    /// Content hash over the compressed output.
    pub content_hash: u64,
    /// Original text size in bytes.
    pub original_bytes: u64,
    /// Compressed size in bytes.
    pub compressed_bytes: u64,
    /// Compression ratio (compressed / original).
    pub ratio: f32,
    /// Compression mode (0=fast, 1=balanced, 2=max).
    pub mode: u8,
    /// Token count of the original text.
    pub token_count: u32,
}

/// Build a text compression result from LLM output.
#[inline]
#[must_use]
pub fn llm_to_text_compressed(
    original_bytes: u64,
    compressed_bytes: u64,
    mode: u8,
    token_count: u32,
) -> LlmTextCompressed {
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&original_bytes.to_le_bytes());
    buf[8..16].copy_from_slice(&compressed_bytes.to_le_bytes());
    buf[16] = mode;
    buf[17..21].copy_from_slice(&token_count.to_le_bytes());
    let ratio = if original_bytes > 0 {
        compressed_bytes as f32 / original_bytes as f32
    } else {
        0.0
    };
    LlmTextCompressed {
        content_hash: fnv1a(&buf),
        original_bytes,
        compressed_bytes,
        ratio,
        mode,
        token_count,
    }
}

// ── Bridge 2: LLM → Search (semantic search index) ─────────────────────

/// Search index entry from LLM embedding for ALICE-Search.
pub struct LlmSearchEntry {
    /// Content hash over the search entry.
    pub content_hash: u64,
    /// Document identifier hash.
    pub doc_hash: u64,
    /// Embedding dimension.
    pub embed_dim: u32,
    /// Number of indexed chunks from this document.
    pub chunk_count: u32,
    /// Average chunk token count.
    pub avg_chunk_tokens: u32,
    /// Whether the embedding is quantized (int8 vs f32).
    pub quantized: bool,
}

/// Build a search index entry from LLM document embedding.
#[inline]
#[must_use]
pub fn llm_to_search_entry(
    doc_hash: u64,
    embed_dim: u32,
    chunk_count: u32,
    avg_chunk_tokens: u32,
    quantized: bool,
) -> LlmSearchEntry {
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&doc_hash.to_le_bytes());
    buf[4..8].copy_from_slice(&embed_dim.to_le_bytes());
    buf[8..12].copy_from_slice(&chunk_count.to_le_bytes());
    buf[12..16].copy_from_slice(&avg_chunk_tokens.to_le_bytes());
    buf[16] = quantized as u8;
    LlmSearchEntry {
        content_hash: fnv1a(&buf),
        doc_hash,
        embed_dim,
        chunk_count,
        avg_chunk_tokens,
        quantized,
    }
}

// ── Bridge 3: LLM → CDN (model distribution) ───────────────────────────

/// Model distribution descriptor for ALICE-CDN.
pub struct LlmCdnAsset {
    /// Content hash over the CDN asset descriptor.
    pub content_hash: u64,
    /// Model file size in bytes.
    pub file_size_bytes: u64,
    /// Model identifier hash.
    pub model_hash: u64,
    /// GGUF quantization type string hash.
    pub quant_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Whether delta updates are supported.
    pub delta_supported: bool,
}

/// Build a CDN asset descriptor for model distribution.
///
/// Large models (>2GB) get longer TTL (86400s) since they change infrequently.
#[inline]
#[must_use]
pub fn llm_to_cdn_asset(
    file_size_bytes: u64,
    model_hash: u64,
    quant_hash: u64,
    delta_supported: bool,
) -> LlmCdnAsset {
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&file_size_bytes.to_le_bytes());
    buf[8..16].copy_from_slice(&model_hash.to_le_bytes());
    buf[16..24].copy_from_slice(&quant_hash.to_le_bytes());
    buf[24] = delta_supported as u8;
    let is_large = (file_size_bytes > 2_000_000_000) as u32;
    let ttl_secs = 3600 + is_large * 82800; // 3600 or 86400
    LlmCdnAsset {
        content_hash: fnv1a(&buf),
        file_size_bytes,
        model_hash,
        quant_hash,
        ttl_secs,
        delta_supported,
    }
}

// ── Bridge 4: LLM → VectorDB (embedding storage) ───────────────────────

/// Vector storage record for ALICE-VectorDB.
pub struct LlmVectorRecord {
    /// Content hash over the vector record.
    pub content_hash: u64,
    /// Vector dimension.
    pub dim: u32,
    /// Source document hash.
    pub doc_hash: u64,
    /// Chunk index within the document.
    pub chunk_idx: u32,
    /// Vector storage size in bytes (dim * element_size).
    pub storage_bytes: u64,
    /// Element type (0=f32, 1=f16, 2=int8).
    pub element_type: u8,
}

/// Build a vector storage record from LLM embedding output.
#[inline]
#[must_use]
pub fn llm_to_vector_record(
    dim: u32,
    doc_hash: u64,
    chunk_idx: u32,
    element_type: u8,
) -> LlmVectorRecord {
    let mut buf = [0u8; 17];
    buf[0..4].copy_from_slice(&dim.to_le_bytes());
    buf[4..12].copy_from_slice(&doc_hash.to_le_bytes());
    buf[12..16].copy_from_slice(&chunk_idx.to_le_bytes());
    buf[16] = element_type;
    let element_size: u64 = match element_type {
        0 => 4,
        1 => 2,
        2 => 1,
        _ => 4,
    };
    let storage_bytes = dim as u64 * element_size;
    LlmVectorRecord {
        content_hash: fnv1a(&buf),
        dim,
        doc_hash,
        chunk_idx,
        storage_bytes,
        element_type,
    }
}

// ── Bridge 5: LLM → Semantic-Telemetry (inference telemetry) ────────────

/// Semantic telemetry event from LLM inference.
pub struct LlmTelemetryEvent {
    /// Content hash over the telemetry event.
    pub content_hash: u64,
    /// Model identifier hash.
    pub model_hash: u64,
    /// Tokens per second.
    pub tps: f32,
    /// Prefill latency in milliseconds.
    pub prefill_ms: u32,
    /// Decode latency in milliseconds.
    pub decode_ms: u32,
    /// Speculative accept rate (0.0 if not used).
    pub spec_accept_rate: f32,
}

/// Build a semantic telemetry event from LLM inference metrics.
#[inline]
#[must_use]
pub fn llm_to_telemetry_event(
    model_hash: u64,
    tps: f32,
    prefill_ms: u32,
    decode_ms: u32,
    spec_accept_rate: f32,
) -> LlmTelemetryEvent {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&model_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&tps.to_bits().to_le_bytes());
    buf[12..16].copy_from_slice(&prefill_ms.to_le_bytes());
    buf[16..20].copy_from_slice(&decode_ms.to_le_bytes());
    buf[20..24].copy_from_slice(&spec_accept_rate.to_bits().to_le_bytes());
    LlmTelemetryEvent {
        content_hash: fnv1a(&buf),
        model_hash,
        tps,
        prefill_ms,
        decode_ms,
        spec_accept_rate,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_compressed_ratio() {
        let t = llm_to_text_compressed(1000, 300, 1, 250);
        assert_ne!(t.content_hash, 0);
        assert!((t.ratio - 0.3).abs() < 0.01);
        assert_eq!(t.mode, 1);
    }

    #[test]
    fn test_text_compressed_zero() {
        let t = llm_to_text_compressed(0, 0, 0, 0);
        assert_eq!(t.ratio, 0.0);
    }

    #[test]
    fn test_search_entry_quantized() {
        let s = llm_to_search_entry(0xdead, 2048, 10, 128, true);
        assert_ne!(s.content_hash, 0);
        assert!(s.quantized);
        assert_eq!(s.chunk_count, 10);
    }

    #[test]
    fn test_cdn_asset_large_ttl() {
        let a = llm_to_cdn_asset(5_000_000_000, 0x1234, 0x5678, true);
        assert_ne!(a.content_hash, 0);
        assert_eq!(a.ttl_secs, 86400); // >2GB → 24h
        assert!(a.delta_supported);
    }

    #[test]
    fn test_cdn_asset_small_ttl() {
        let a = llm_to_cdn_asset(770_000_000, 0x1234, 0x5678, false);
        assert_eq!(a.ttl_secs, 3600); // <2GB → 1h
    }

    #[test]
    fn test_vector_record_f32() {
        let v = llm_to_vector_record(2048, 0xbeef, 0, 0);
        assert_ne!(v.content_hash, 0);
        assert_eq!(v.storage_bytes, 8192); // 2048 * 4
        assert_eq!(v.element_type, 0);
    }

    #[test]
    fn test_vector_record_int8() {
        let v = llm_to_vector_record(768, 0xface, 5, 2);
        assert_eq!(v.storage_bytes, 768); // 768 * 1
    }

    #[test]
    fn test_telemetry_event_speculative() {
        let e = llm_to_telemetry_event(0xaaaa, 5.7, 5548, 1229, 0.63);
        assert_ne!(e.content_hash, 0);
        assert!((e.tps - 5.7).abs() < 0.1);
        assert!((e.spec_accept_rate - 0.63).abs() < 0.01);
    }

    #[test]
    fn test_telemetry_event_determinism() {
        let a = llm_to_telemetry_event(0xbbbb, 20.2, 100, 500, 0.0);
        let b = llm_to_telemetry_event(0xbbbb, 20.2, 100, 500, 0.0);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
