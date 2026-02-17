//! Text bridges — ALICE-Text ↔ Font, Manga, DB, Browser, Queue, Analytics
//!
//! 6 bridges connecting exception-based text compression to the ALICE ecosystem.

use alice_text::{compress_tuned, decompress_tuned, CompressionMode};

// ── Bridge 1: Text → Font (compressed text → MetaFont rendering) ────────

/// Compressed text ready for ALICE-Font rendering.
pub struct TextFontPayload {
    /// Decompressed text for rendering.
    pub text: String,
    /// Character count.
    pub char_count: usize,
    /// Original compressed size.
    pub compressed_bytes: usize,
    /// Compression ratio achieved.
    pub compression_ratio: f32,
}

/// Decompress ALICE-Text payload for ALICE-Font rendering.
#[inline]
pub fn text_to_font_payload(compressed: &[u8]) -> Option<TextFontPayload> {
    let text = decompress_tuned(compressed).ok()?;
    let char_count = text.chars().count();
    Some(TextFontPayload {
        text,
        char_count,
        compressed_bytes: compressed.len(),
        compression_ratio: if compressed.is_empty() { 0.0 } else { (char_count as f32) / compressed.len() as f32 },
    })
}

// ── Bridge 2: Text → Manga (dialogue compression → page text) ───────────

/// Compressed manga dialogue for ALICE-Manga balloon text.
pub struct TextMangaDialogue {
    /// Compressed dialogue bytes.
    pub compressed: Vec<u8>,
    /// Original text length.
    pub original_len: usize,
    /// Compressed size.
    pub compressed_len: usize,
    /// Compression ratio.
    pub compression_ratio: f32,
    /// Number of dialogue lines.
    pub line_count: usize,
}

/// Compress manga dialogue text for ALICE-Manga page embedding.
#[inline]
pub fn text_to_manga_dialogue(dialogue: &str) -> TextMangaDialogue {
    let compressed = compress_tuned(dialogue, CompressionMode::Balanced).unwrap_or_else(|_| dialogue.as_bytes().to_vec());
    let line_count = dialogue.lines().count();
    TextMangaDialogue {
        compressed_len: compressed.len(),
        original_len: dialogue.len(),
        compression_ratio: if compressed.is_empty() { 0.0 } else { dialogue.len() as f32 / compressed.len() as f32 },
        line_count,
        compressed,
    }
}

// ── Bridge 3: Text → DB (columnar log → DB storage) ────────────────────

/// Compressed log record for ALICE-DB persistence.
pub struct TextDbLogRecord {
    /// Compressed payload bytes.
    pub compressed: Vec<u8>,
    /// Original log size.
    pub original_bytes: usize,
    /// Compressed size.
    pub compressed_bytes: usize,
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Number of log entries.
    pub entry_count: usize,
}

/// Compress log batch for ALICE-DB storage.
#[inline]
pub fn text_to_db_log_batch(logs: &[&str]) -> TextDbLogRecord {
    let combined = logs.join("\n");
    let compressed = compress_tuned(&combined, CompressionMode::Balanced).unwrap_or_else(|_| combined.as_bytes().to_vec());
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &compressed {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    TextDbLogRecord {
        original_bytes: combined.len(),
        compressed_bytes: compressed.len(),
        content_hash: hash,
        entry_count: logs.len(),
        compressed,
    }
}

// ── Bridge 4: Text → Browser (exception-compressed → DOM content) ───────

/// Compressed DOM text content for ALICE-Browser.
pub struct TextBrowserContent {
    /// Compressed DOM text.
    pub compressed: Vec<u8>,
    /// Original size.
    pub original_bytes: usize,
    /// Compressed size.
    pub compressed_bytes: usize,
    /// Bandwidth savings percentage.
    pub bandwidth_saving_pct: f32,
}

/// Compress browser DOM text content via ALICE-Text.
#[inline]
pub fn text_to_browser_content(dom_text: &str) -> TextBrowserContent {
    let compressed = compress_tuned(dom_text, CompressionMode::Balanced).unwrap_or_else(|_| dom_text.as_bytes().to_vec());
    let saving = if dom_text.is_empty() { 0.0 } else { (1.0 - compressed.len() as f32 / dom_text.len() as f32) * 100.0 };
    TextBrowserContent {
        original_bytes: dom_text.len(),
        compressed_bytes: compressed.len(),
        bandwidth_saving_pct: saving.max(0.0),
        compressed,
    }
}

// ── Bridge 5: Text → Queue (compressed text messages) ────────────────────

/// Compressed text message for ALICE-Queue delivery.
pub struct TextQueueMessage {
    /// Compressed payload bytes.
    pub compressed: Vec<u8>,
    /// Original text length.
    pub original_bytes: usize,
    /// Compressed size.
    pub compressed_bytes: usize,
    /// Content hash for deduplication.
    pub content_hash: u64,
}

/// Compress text for ALICE-Queue message delivery.
#[inline]
pub fn text_to_queue_message(text: &str) -> TextQueueMessage {
    let compressed = compress_tuned(text, CompressionMode::Fast).unwrap_or_else(|_| text.as_bytes().to_vec());
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &compressed {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    TextQueueMessage {
        original_bytes: text.len(),
        compressed_bytes: compressed.len(),
        content_hash: hash,
        compressed,
    }
}

// ── Bridge 6: Text → Analytics (compression metrics) ─────────────────────

/// Text compression metrics for ALICE-Analytics monitoring.
pub struct TextAnalyticsMetrics {
    /// Original size in bytes.
    pub original_bytes: usize,
    /// Compressed size in bytes.
    pub compressed_bytes: usize,
    /// Compression ratio.
    pub compression_ratio: f32,
    /// Bandwidth savings percentage.
    pub bandwidth_saving_pct: f32,
}

/// Extract compression metrics for ALICE-Analytics.
#[inline]
pub fn text_to_analytics_metrics(text: &str) -> TextAnalyticsMetrics {
    let compressed = compress_tuned(text, CompressionMode::Balanced).unwrap_or_else(|_| text.as_bytes().to_vec());
    let ratio = if compressed.is_empty() { 0.0 } else { text.len() as f32 / compressed.len() as f32 };
    let saving = if text.is_empty() { 0.0 } else { (1.0 - compressed.len() as f32 / text.len() as f32) * 100.0 };
    TextAnalyticsMetrics {
        original_bytes: text.len(),
        compressed_bytes: compressed.len(),
        compression_ratio: ratio,
        bandwidth_saving_pct: saving.max(0.0),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_to_manga_dialogue() {
        let dialogue = "Hello world!\nHow are you?\nFine, thanks.";
        let result = text_to_manga_dialogue(dialogue);
        assert_eq!(result.line_count, 3);
        assert!(result.compressed_len > 0);
        assert!(result.original_len > 0);
    }

    #[test]
    fn test_text_to_db_log_batch() {
        let logs = vec!["2024-01-01 INFO startup", "2024-01-01 WARN timeout", "2024-01-01 ERROR crash"];
        let result = text_to_db_log_batch(&logs);
        assert_eq!(result.entry_count, 3);
        assert_ne!(result.content_hash, 0);
        assert!(result.compressed_bytes > 0);
    }

    #[test]
    fn test_text_to_browser_content() {
        let content = "The quick brown fox jumps over the lazy dog. ".repeat(100);
        let result = text_to_browser_content(&content);
        assert!(result.compressed_bytes > 0);
        assert!(result.original_bytes > 0);
    }

    #[test]
    fn test_text_to_font_roundtrip() {
        let original = "Hello, ALICE!";
        let compressed = compress_tuned(original, CompressionMode::Balanced).unwrap();
        let payload = text_to_font_payload(&compressed);
        assert!(payload.is_some());
        let p = payload.unwrap();
        assert_eq!(p.text, original);
        assert_eq!(p.char_count, 13);
    }

    #[test]
    fn test_text_to_queue_message() {
        let msg = text_to_queue_message("Hello ALICE queue!");
        assert!(msg.compressed_bytes > 0);
        assert_ne!(msg.content_hash, 0);
        assert!(msg.original_bytes > 0);
    }

    #[test]
    fn test_text_to_analytics_metrics() {
        let content = "The quick brown fox jumps over the lazy dog. ".repeat(50);
        let m = text_to_analytics_metrics(&content);
        assert!(m.original_bytes > 0);
        assert!(m.compressed_bytes > 0);
        assert!(m.compression_ratio > 0.0);
    }
}
