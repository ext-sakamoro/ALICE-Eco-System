//! LLM media bridges — ALICE-LLM ↔ Chat, ASR, TTS, Codec, Diffusion
//!
//! 5 bridges connecting LLM inference to media and communication systems.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LLM → Chat (conversational UI) ───────────────────────────

/// Chat message from LLM generation for ALICE-Chat.
pub struct LlmChatMessage {
    /// Content hash over the chat message.
    pub content_hash: u64,
    /// Message role (0=system, 1=user, 2=assistant, 3=tool).
    pub role: u8,
    /// Token count of the message.
    pub token_count: u32,
    /// Conversation turn index.
    pub turn_idx: u32,
    /// Generation latency in milliseconds.
    pub latency_ms: u32,
    /// Whether the response was stopped by EOS vs max_tokens.
    pub natural_stop: bool,
}

/// Build a chat message from LLM generation output.
#[inline]
#[must_use]
pub fn llm_to_chat_message(
    role: u8,
    token_count: u32,
    turn_idx: u32,
    latency_ms: u32,
    natural_stop: bool,
) -> LlmChatMessage {
    let mut buf = [0u8; 14];
    buf[0] = role;
    buf[1..5].copy_from_slice(&token_count.to_le_bytes());
    buf[5..9].copy_from_slice(&turn_idx.to_le_bytes());
    buf[9..13].copy_from_slice(&latency_ms.to_le_bytes());
    buf[13] = natural_stop as u8;
    LlmChatMessage {
        content_hash: fnv1a(&buf),
        role,
        token_count,
        turn_idx,
        latency_ms,
        natural_stop,
    }
}

// ── Bridge 2: ASR → LLM (speech-to-text input) ─────────────────────────

/// Speech recognition result as LLM input.
pub struct AsrLlmInput {
    /// Content hash over the ASR result.
    pub content_hash: u64,
    /// Transcribed text length in characters.
    pub char_count: u32,
    /// ASR confidence score (0.0–1.0).
    pub confidence: f32,
    /// Audio duration in milliseconds.
    pub audio_duration_ms: u32,
    /// Sample rate of the source audio (Hz).
    pub sample_rate: u32,
    /// Whether real-time streaming ASR was used.
    pub streaming: bool,
}

/// Build an LLM input descriptor from ASR transcription result.
#[inline]
#[must_use]
pub fn asr_to_llm_input(
    char_count: u32,
    confidence: f32,
    audio_duration_ms: u32,
    sample_rate: u32,
    streaming: bool,
) -> AsrLlmInput {
    let mut buf = [0u8; 17];
    buf[0..4].copy_from_slice(&char_count.to_le_bytes());
    buf[4..8].copy_from_slice(&confidence.to_bits().to_le_bytes());
    buf[8..12].copy_from_slice(&audio_duration_ms.to_le_bytes());
    buf[12..16].copy_from_slice(&sample_rate.to_le_bytes());
    buf[16] = streaming as u8;
    AsrLlmInput {
        content_hash: fnv1a(&buf),
        char_count,
        confidence,
        audio_duration_ms,
        sample_rate,
        streaming,
    }
}

// ── Bridge 3: LLM → TTS (text-to-speech output) ────────────────────────

/// TTS synthesis request from LLM-generated text.
pub struct LlmTtsRequest {
    /// Content hash over the TTS request.
    pub content_hash: u64,
    /// Text length in characters.
    pub char_count: u32,
    /// Target sample rate (Hz).
    pub sample_rate: u32,
    /// Voice profile identifier hash.
    pub voice_hash: u64,
    /// Estimated audio duration in milliseconds.
    pub estimated_duration_ms: u32,
    /// Whether sentence-level streaming is enabled.
    pub streaming: bool,
}

/// Build a TTS request from LLM output text.
///
/// Duration estimate: ~75 ms per character (natural speech ~160 wpm).
#[inline]
#[must_use]
pub fn llm_to_tts_request(
    char_count: u32,
    sample_rate: u32,
    voice_hash: u64,
    streaming: bool,
) -> LlmTtsRequest {
    let mut buf = [0u8; 17];
    buf[0..4].copy_from_slice(&char_count.to_le_bytes());
    buf[4..8].copy_from_slice(&sample_rate.to_le_bytes());
    buf[8..16].copy_from_slice(&voice_hash.to_le_bytes());
    buf[16] = streaming as u8;
    let estimated_duration_ms = char_count * 75;
    LlmTtsRequest {
        content_hash: fnv1a(&buf),
        char_count,
        sample_rate,
        voice_hash,
        estimated_duration_ms,
        streaming,
    }
}

// ── Bridge 4: LLM → Codec (token encoding/compression) ─────────────────

/// Token codec descriptor for LLM token serialization.
pub struct LlmCodecDescriptor {
    /// Content hash over the codec descriptor.
    pub content_hash: u64,
    /// Vocabulary size.
    pub vocab_size: u32,
    /// Bytes per token in the encoding.
    pub bytes_per_token: u8,
    /// Number of special tokens.
    pub special_token_count: u32,
    /// Total encoded payload size in bytes.
    pub payload_bytes: u64,
    /// Compression ratio vs UTF-8 (tokens * bpt / original_bytes).
    pub compression_ratio: f32,
}

/// Build a codec descriptor from tokenizer and text metadata.
#[inline]
#[must_use]
pub fn llm_to_codec_descriptor(
    vocab_size: u32,
    token_count: u32,
    special_token_count: u32,
    original_bytes: u64,
) -> LlmCodecDescriptor {
    let bytes_per_token: u8 = if vocab_size <= 256 {
        1
    } else if vocab_size <= 65536 {
        2
    } else {
        4
    };
    let mut buf = [0u8; 13];
    buf[0..4].copy_from_slice(&vocab_size.to_le_bytes());
    buf[4..8].copy_from_slice(&token_count.to_le_bytes());
    buf[8..12].copy_from_slice(&special_token_count.to_le_bytes());
    buf[12] = bytes_per_token;
    let payload_bytes = token_count as u64 * bytes_per_token as u64;
    let compression_ratio = if original_bytes > 0 {
        payload_bytes as f32 / original_bytes as f32
    } else {
        0.0
    };
    LlmCodecDescriptor {
        content_hash: fnv1a(&buf),
        vocab_size,
        bytes_per_token,
        special_token_count,
        payload_bytes,
        compression_ratio,
    }
}

// ── Bridge 5: LLM → Diffusion (multimodal generation) ──────────────────

/// Diffusion generation prompt from LLM text output.
pub struct LlmDiffusionPrompt {
    /// Content hash over the diffusion prompt.
    pub content_hash: u64,
    /// Prompt embedding dimension.
    pub embed_dim: u32,
    /// Number of diffusion steps.
    pub num_steps: u32,
    /// Guidance scale (CFG).
    pub guidance_scale: f32,
    /// Target image resolution (width).
    pub width: u32,
    /// Target image resolution (height).
    pub height: u32,
}

/// Build a diffusion prompt descriptor from LLM text embedding.
#[inline]
#[must_use]
pub fn llm_to_diffusion_prompt(
    embed_dim: u32,
    num_steps: u32,
    guidance_scale: f32,
    width: u32,
    height: u32,
) -> LlmDiffusionPrompt {
    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&embed_dim.to_le_bytes());
    buf[4..8].copy_from_slice(&num_steps.to_le_bytes());
    buf[8..12].copy_from_slice(&guidance_scale.to_bits().to_le_bytes());
    buf[12..16].copy_from_slice(&width.to_le_bytes());
    buf[16..20].copy_from_slice(&height.to_le_bytes());
    LlmDiffusionPrompt {
        content_hash: fnv1a(&buf),
        embed_dim,
        num_steps,
        guidance_scale,
        width,
        height,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_assistant() {
        let m = llm_to_chat_message(2, 64, 3, 1500, true);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.role, 2);
        assert!(m.natural_stop);
    }

    #[test]
    fn test_chat_message_truncated() {
        let m = llm_to_chat_message(2, 4096, 1, 30000, false);
        assert!(!m.natural_stop);
    }

    #[test]
    fn test_asr_to_llm_input() {
        let a = asr_to_llm_input(500, 0.95, 30000, 16000, true);
        assert_ne!(a.content_hash, 0);
        assert!(a.streaming);
        assert!((a.confidence - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_tts_request_duration() {
        let t = llm_to_tts_request(200, 24000, 0xface, false);
        assert_ne!(t.content_hash, 0);
        assert_eq!(t.estimated_duration_ms, 15000); // 200 * 75
        assert!(!t.streaming);
    }

    #[test]
    fn test_codec_descriptor_large_vocab() {
        let c = llm_to_codec_descriptor(128256, 1024, 256, 4096);
        assert_ne!(c.content_hash, 0);
        assert_eq!(c.bytes_per_token, 4); // >65536 → 4 bytes
        assert_eq!(c.payload_bytes, 4096); // 1024 * 4
    }

    #[test]
    fn test_codec_descriptor_small_vocab() {
        let c = llm_to_codec_descriptor(256, 100, 10, 400);
        assert_eq!(c.bytes_per_token, 1);
        assert_eq!(c.payload_bytes, 100);
    }

    #[test]
    fn test_diffusion_prompt_fields() {
        let d = llm_to_diffusion_prompt(2048, 50, 7.5, 1024, 1024);
        assert_ne!(d.content_hash, 0);
        assert_eq!(d.num_steps, 50);
        assert!((d.guidance_scale - 7.5).abs() < 0.01);
    }

    #[test]
    fn test_diffusion_prompt_determinism() {
        let a = llm_to_diffusion_prompt(768, 20, 3.0, 512, 512);
        let b = llm_to_diffusion_prompt(768, 20, 3.0, 512, 512);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
