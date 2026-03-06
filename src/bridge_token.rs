//! Token bridges — ALICE-Token ↔ Text, ML, Search, DB, Cache, Analytics
//!
//! 6 bridges connecting BPE tokenization to the ALICE ecosystem.

use alice_token::Tokenizer;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Token → Text (tokenized text for compression pipeline) ────

/// Tokenized text summary for ALICE-Text compression pipeline.
pub struct TokenTextSummary {
    /// Content hash of the token ID sequence (FNV-1a).
    pub content_hash: u64,
    /// Token IDs produced by BPE encoding.
    pub token_ids: Vec<u32>,
    /// Number of tokens.
    pub token_count: usize,
    /// Original byte length of input text.
    pub original_bytes: usize,
    /// Compression ratio (original bytes / token count).
    pub bytes_per_token: f32,
}

/// Tokenize text for ALICE-Text compression pipeline.
#[inline]
#[must_use]
pub fn token_to_text_summary(tokenizer: &Tokenizer, text: &[u8]) -> TokenTextSummary {
    let ids = tokenizer.encode(text);
    let id_bytes: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
    let content_hash = fnv1a(&id_bytes);
    let token_count = ids.len();
    let bytes_per_token = if token_count == 0 {
        0.0
    } else {
        text.len() as f32 / token_count as f32
    };
    TokenTextSummary {
        content_hash,
        token_ids: ids,
        token_count,
        original_bytes: text.len(),
        bytes_per_token,
    }
}

// ── Bridge 2: Token → ML (tokenized input for ternary inference) ────────

/// Tokenized input batch for ALICE-ML ternary inference.
pub struct TokenMlInput {
    /// Content hash of the token sequence (FNV-1a).
    pub content_hash: u64,
    /// Token IDs as inference input.
    pub token_ids: Vec<u32>,
    /// Sequence length.
    pub seq_len: usize,
    /// Vocabulary size (for embedding lookup bounds).
    pub vocab_size: usize,
    /// Maximum token ID in the sequence.
    pub max_token_id: u32,
}

/// Tokenize text for ALICE-ML inference input.
#[inline]
#[must_use]
pub fn token_to_ml_input(tokenizer: &Tokenizer, text: &[u8]) -> TokenMlInput {
    let ids = tokenizer.encode(text);
    let id_bytes: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
    let content_hash = fnv1a(&id_bytes);
    let max_token_id = ids.iter().copied().max().unwrap_or(0);
    TokenMlInput {
        content_hash,
        seq_len: ids.len(),
        vocab_size: tokenizer.vocab_size(),
        max_token_id,
        token_ids: ids,
    }
}

// ── Bridge 3: Token → Search (tokenized content for FM-Index) ───────────

/// Tokenized search document for ALICE-Search FM-Index indexing.
pub struct TokenSearchDocument {
    /// Content hash of the token sequence (FNV-1a).
    pub content_hash: u64,
    /// Document hash of the raw bytes (FNV-1a).
    pub document_hash: u64,
    /// Token IDs for index construction.
    pub token_ids: Vec<u32>,
    /// Token count (posting list size estimate).
    pub token_count: usize,
    /// Estimated index bytes (`token_count` * 4).
    pub estimated_index_bytes: usize,
}

/// Tokenize document for ALICE-Search indexing.
#[inline]
#[must_use]
pub fn token_to_search_document(tokenizer: &Tokenizer, text: &[u8]) -> TokenSearchDocument {
    let ids = tokenizer.encode(text);
    let id_bytes: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
    let content_hash = fnv1a(&id_bytes);
    let document_hash = fnv1a(text);
    let token_count = ids.len();
    TokenSearchDocument {
        content_hash,
        document_hash,
        token_count,
        estimated_index_bytes: token_count * 4,
        token_ids: ids,
    }
}

// ── Bridge 4: Token → DB (tokenized record for persistence) ─────────────

/// Tokenized record for ALICE-DB persistence.
pub struct TokenDbRecord {
    /// Content hash for deduplication (FNV-1a over token IDs).
    pub content_hash: u64,
    /// Serialized token IDs (little-endian u32 bytes).
    pub serialized: Vec<u8>,
    /// Token count.
    pub token_count: usize,
    /// Serialized size in bytes.
    pub serialized_bytes: usize,
    /// Vocabulary size at encoding time.
    pub vocab_size: usize,
}

/// Tokenize and serialize for ALICE-DB storage.
#[inline]
#[must_use]
pub fn token_to_db_record(tokenizer: &Tokenizer, text: &[u8]) -> TokenDbRecord {
    let ids = tokenizer.encode(text);
    let serialized: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
    let content_hash = fnv1a(&serialized);
    let token_count = ids.len();
    TokenDbRecord {
        content_hash,
        serialized_bytes: serialized.len(),
        token_count,
        vocab_size: tokenizer.vocab_size(),
        serialized,
    }
}

// ── Bridge 5: Token → Cache (tokenization result caching) ───────────────

/// Cached tokenization result for ALICE-Cache.
pub struct TokenCacheEntry {
    /// Content hash for cache key (FNV-1a over input bytes).
    pub content_hash: u64,
    /// Token count (result size indicator).
    pub token_count: usize,
    /// Input byte length (for cache eviction priority).
    pub input_bytes: usize,
    /// TTL in seconds (branchless: longer text → longer TTL).
    pub ttl_secs: u32,
    /// Vocabulary size used for encoding.
    pub vocab_size: usize,
}

/// Build cache entry for ALICE-Cache from tokenization result.
#[inline]
#[must_use]
pub fn token_to_cache_entry(tokenizer: &Tokenizer, text: &[u8]) -> TokenCacheEntry {
    let ids = tokenizer.encode(text);
    let content_hash = fnv1a(text);
    // Branchless TTL: large inputs (>1024 bytes) get 600s, small get 300s
    let is_large = (text.len() > 1024) as u32;
    let ttl_secs = 300 + is_large * 300;
    TokenCacheEntry {
        content_hash,
        token_count: ids.len(),
        input_bytes: text.len(),
        ttl_secs,
        vocab_size: tokenizer.vocab_size(),
    }
}

// ── Bridge 6: Token → Analytics (tokenization metrics) ──────────────────

/// Tokenization metrics for ALICE-Analytics monitoring.
pub struct TokenAnalyticsMetrics {
    /// Content hash of the token sequence (FNV-1a).
    pub content_hash: u64,
    /// Number of tokens produced.
    pub token_count: usize,
    /// Original input bytes.
    pub input_bytes: usize,
    /// Average bytes per token.
    pub bytes_per_token: f32,
    /// Vocabulary utilization (unique tokens / vocab size).
    pub vocab_utilization: f32,
}

/// Extract tokenization metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn token_to_analytics_metrics(tokenizer: &Tokenizer, text: &[u8]) -> TokenAnalyticsMetrics {
    let ids = tokenizer.encode(text);
    let id_bytes: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
    let content_hash = fnv1a(&id_bytes);
    let token_count = ids.len();
    let bytes_per_token = if token_count == 0 {
        0.0
    } else {
        text.len() as f32 / token_count as f32
    };
    // Count unique tokens for utilization
    let mut seen = std::collections::HashSet::new();
    for &id in &ids {
        seen.insert(id);
    }
    let vocab_size = tokenizer.vocab_size();
    let vocab_utilization = if vocab_size == 0 {
        0.0
    } else {
        seen.len() as f32 / vocab_size as f32
    };
    TokenAnalyticsMetrics {
        content_hash,
        token_count,
        input_bytes: text.len(),
        bytes_per_token,
        vocab_utilization,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_token::VocabBuilder;

    fn make_test_tokenizer() -> Tokenizer {
        let mut builder = VocabBuilder::new();
        for b in 0u16..=255 {
            builder.add_token(vec![b as u8]);
        }
        builder.add_merge(b"h", b"e");
        builder.add_merge(b"l", b"l");
        builder.add_merge(b"o", b" ");
        let vocab = builder.build();
        Tokenizer::new(vocab)
    }

    #[test]
    fn test_token_to_text_summary() {
        let tok = make_test_tokenizer();
        let result = token_to_text_summary(&tok, b"hello world");
        assert!(result.token_count > 0);
        assert_eq!(result.original_bytes, 11);
        assert_ne!(result.content_hash, 0);
        assert!(result.bytes_per_token > 0.0);
    }

    #[test]
    fn test_token_to_text_summary_empty() {
        let tok = make_test_tokenizer();
        let result = token_to_text_summary(&tok, b"");
        assert_eq!(result.token_count, 0);
        assert_eq!(result.original_bytes, 0);
        assert!((result.bytes_per_token - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_token_to_ml_input() {
        let tok = make_test_tokenizer();
        let result = token_to_ml_input(&tok, b"hello");
        assert!(result.seq_len > 0);
        assert_eq!(result.vocab_size, tok.vocab_size());
        assert_ne!(result.content_hash, 0);
        assert!(result.max_token_id < tok.vocab_size() as u32);
    }

    #[test]
    fn test_token_to_search_document() {
        let tok = make_test_tokenizer();
        let result = token_to_search_document(&tok, b"the quick brown fox");
        assert!(result.token_count > 0);
        assert_eq!(result.estimated_index_bytes, result.token_count * 4);
        assert_ne!(result.content_hash, 0);
        assert_ne!(result.document_hash, 0);
    }

    #[test]
    fn test_token_to_search_document_deterministic() {
        let tok = make_test_tokenizer();
        let a = token_to_search_document(&tok, b"hello world");
        let b = token_to_search_document(&tok, b"hello world");
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.document_hash, b.document_hash);
        assert_eq!(a.token_count, b.token_count);
    }

    #[test]
    fn test_token_to_db_record() {
        let tok = make_test_tokenizer();
        let rec = token_to_db_record(&tok, b"hello ALICE");
        assert!(rec.token_count > 0);
        assert_eq!(rec.serialized_bytes, rec.token_count * 4);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.vocab_size, tok.vocab_size());
    }

    #[test]
    fn test_token_to_cache_entry_small() {
        let tok = make_test_tokenizer();
        let entry = token_to_cache_entry(&tok, b"short text");
        assert_eq!(entry.ttl_secs, 300); // small input → 300s
        assert_ne!(entry.content_hash, 0);
        assert!(entry.token_count > 0);
    }

    #[test]
    fn test_token_to_cache_entry_large() {
        let tok = make_test_tokenizer();
        let large = vec![b'x'; 2048];
        let entry = token_to_cache_entry(&tok, &large);
        assert_eq!(entry.ttl_secs, 600); // large input → 600s
        assert_eq!(entry.input_bytes, 2048);
    }

    #[test]
    fn test_token_to_analytics_metrics() {
        let tok = make_test_tokenizer();
        let m = token_to_analytics_metrics(&tok, b"hello world");
        assert!(m.token_count > 0);
        assert_eq!(m.input_bytes, 11);
        assert!(m.bytes_per_token > 0.0);
        assert!(m.vocab_utilization > 0.0);
        assert!(m.vocab_utilization <= 1.0);
        assert_ne!(m.content_hash, 0);
    }

    #[test]
    fn test_different_inputs_different_hashes() {
        let tok = make_test_tokenizer();
        let a = token_to_text_summary(&tok, b"hello");
        let b = token_to_text_summary(&tok, b"world");
        assert_ne!(a.content_hash, b.content_hash);
    }
}
