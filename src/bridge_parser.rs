//! Parser bridges — Parser ↔ DB, Cache, Analytics, Compiler, API
//!
//! 5 bridges connecting the parser to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Parser → DB (AST persistence) ──────────────────────────────

/// AST persistence record for ALICE-DB.
pub struct ParserDbRecord {
    /// Content hash (FNV-1a of token_count + ast_depth + input_bytes).
    pub content_hash: u64,
    /// Number of tokens produced by the lexer.
    pub token_count: u32,
    /// Maximum depth of the AST tree.
    pub ast_depth: u32,
    /// Number of parse errors encountered.
    pub error_count: u16,
    /// Parse time in microseconds.
    pub parse_time_us: u64,
    /// Input source size in bytes.
    pub input_bytes: u64,
}

/// Serialize parser output for ALICE-DB AST persistence.
#[inline]
#[must_use]
pub fn parser_to_db_record(
    token_count: u32,
    ast_depth: u32,
    error_count: u16,
    parse_time_us: u64,
    input_bytes: u64,
) -> ParserDbRecord {
    let mut buf = [0u8; 4 + 4 + 2 + 8 + 8];
    buf[..4].copy_from_slice(&token_count.to_le_bytes());
    buf[4..8].copy_from_slice(&ast_depth.to_le_bytes());
    buf[8..10].copy_from_slice(&error_count.to_le_bytes());
    buf[10..18].copy_from_slice(&parse_time_us.to_le_bytes());
    buf[18..26].copy_from_slice(&input_bytes.to_le_bytes());
    ParserDbRecord {
        content_hash: fnv1a(&buf),
        token_count,
        ast_depth,
        error_count,
        parse_time_us,
        input_bytes,
    }
}

// ── Bridge 2: Parser → Cache (parse cache) ───────────────────────────────

/// Parse cache entry for ALICE-Cache.
pub struct ParserCacheEntry {
    /// Content hash (FNV-1a of input_bytes + token_count).
    pub content_hash: u64,
    /// Input size in bytes (cache key component).
    pub input_bytes: u64,
    /// Token count.
    pub token_count: u32,
    /// Cache TTL in seconds (branchless: 120 if error_count == 0, else 0).
    pub ttl_secs: u32,
    /// Parse error count.
    pub error_count: u16,
}

/// Build a parse cache entry for ALICE-Cache.
///
/// `ttl_secs` is computed branchlessly: 120 when `error_count == 0`, else 0.
#[inline]
#[must_use]
pub fn parser_to_cache_entry(
    input_bytes: u64,
    token_count: u32,
    error_count: u16,
) -> ParserCacheEntry {
    let mut buf = [0u8; 8 + 4 + 2];
    buf[..8].copy_from_slice(&input_bytes.to_le_bytes());
    buf[8..12].copy_from_slice(&token_count.to_le_bytes());
    buf[12..14].copy_from_slice(&error_count.to_le_bytes());
    // ブランチレスTTL: エラーなしなら120秒、エラーありなら0秒
    let no_error = (error_count == 0) as u32;
    let ttl_secs = no_error * 120;
    ParserCacheEntry {
        content_hash: fnv1a(&buf),
        input_bytes,
        token_count,
        ttl_secs,
        error_count,
    }
}

// ── Bridge 3: Parser → Analytics (parse metrics) ─────────────────────────

/// Parse metrics for ALICE-Analytics.
pub struct ParserAnalyticsMetrics {
    /// Content hash (FNV-1a of all metric fields).
    pub content_hash: u64,
    /// Number of tokens produced.
    pub token_count: u32,
    /// Maximum AST depth.
    pub ast_depth: u32,
    /// Number of parse errors.
    pub error_count: u16,
    /// Parse time in microseconds.
    pub parse_time_us: u64,
    /// Input size in bytes.
    pub input_bytes: u64,
    /// Number of AST nodes produced.
    pub node_count: u32,
}

/// Extract parse metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn parser_to_analytics_metrics(
    token_count: u32,
    ast_depth: u32,
    error_count: u16,
    parse_time_us: u64,
    input_bytes: u64,
    node_count: u32,
) -> ParserAnalyticsMetrics {
    let mut buf = [0u8; 4 + 4 + 2 + 8 + 8 + 4];
    buf[..4].copy_from_slice(&token_count.to_le_bytes());
    buf[4..8].copy_from_slice(&ast_depth.to_le_bytes());
    buf[8..10].copy_from_slice(&error_count.to_le_bytes());
    buf[10..18].copy_from_slice(&parse_time_us.to_le_bytes());
    buf[18..26].copy_from_slice(&input_bytes.to_le_bytes());
    buf[26..30].copy_from_slice(&node_count.to_le_bytes());
    ParserAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        token_count,
        ast_depth,
        error_count,
        parse_time_us,
        input_bytes,
        node_count,
    }
}

// ── Bridge 4: Parser → Compiler (AST handoff) ────────────────────────────

/// AST handoff descriptor for ALICE-Compiler.
pub struct ParserCompilerHandoff {
    /// Content hash (FNV-1a of node_count + ast_depth + token_count).
    pub content_hash: u64,
    /// Number of AST nodes to hand off.
    pub node_count: u32,
    /// Maximum AST depth.
    pub ast_depth: u32,
    /// Token count used during parse.
    pub token_count: u32,
    /// Parse error count (compiler rejects if > 0).
    pub error_count: u16,
    /// Input size in bytes.
    pub input_bytes: u64,
}

/// Build an AST handoff descriptor for ALICE-Compiler.
#[inline]
#[must_use]
pub fn parser_to_compiler_handoff(
    node_count: u32,
    ast_depth: u32,
    token_count: u32,
    error_count: u16,
    input_bytes: u64,
) -> ParserCompilerHandoff {
    let mut buf = [0u8; 4 + 4 + 4 + 2 + 8];
    buf[..4].copy_from_slice(&node_count.to_le_bytes());
    buf[4..8].copy_from_slice(&ast_depth.to_le_bytes());
    buf[8..12].copy_from_slice(&token_count.to_le_bytes());
    buf[12..14].copy_from_slice(&error_count.to_le_bytes());
    buf[14..22].copy_from_slice(&input_bytes.to_le_bytes());
    ParserCompilerHandoff {
        content_hash: fnv1a(&buf),
        node_count,
        ast_depth,
        token_count,
        error_count,
        input_bytes,
    }
}

// ── Bridge 5: Parser → API (parse service) ───────────────────────────────

/// Parse service response for ALICE-API.
pub struct ParserApiResponse {
    /// Content hash (FNV-1a of token_count + error_count + parse_time_us).
    pub content_hash: u64,
    /// Token count.
    pub token_count: u32,
    /// Number of parse errors.
    pub error_count: u16,
    /// Parse time in microseconds.
    pub parse_time_us: u64,
    /// AST depth.
    pub ast_depth: u32,
    /// Input bytes processed.
    pub input_bytes: u64,
}

/// Build a parse service response for ALICE-API.
#[inline]
#[must_use]
pub fn parser_to_api_response(
    token_count: u32,
    error_count: u16,
    parse_time_us: u64,
    ast_depth: u32,
    input_bytes: u64,
) -> ParserApiResponse {
    let mut buf = [0u8; 4 + 2 + 8 + 4 + 8];
    buf[..4].copy_from_slice(&token_count.to_le_bytes());
    buf[4..6].copy_from_slice(&error_count.to_le_bytes());
    buf[6..14].copy_from_slice(&parse_time_us.to_le_bytes());
    buf[14..18].copy_from_slice(&ast_depth.to_le_bytes());
    buf[18..26].copy_from_slice(&input_bytes.to_le_bytes());
    ParserApiResponse {
        content_hash: fnv1a(&buf),
        token_count,
        error_count,
        parse_time_us,
        ast_depth,
        input_bytes,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_to_db_record_basic() {
        let rec = parser_to_db_record(256, 12, 0, 3500, 4096);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.token_count, 256);
        assert_eq!(rec.ast_depth, 12);
        assert_eq!(rec.error_count, 0);
        assert_eq!(rec.parse_time_us, 3500);
        assert_eq!(rec.input_bytes, 4096);
    }

    #[test]
    fn test_parser_to_db_record_determinism() {
        let r1 = parser_to_db_record(100, 8, 2, 1000, 2048);
        let r2 = parser_to_db_record(100, 8, 2, 1000, 2048);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_parser_to_cache_entry_no_error_ttl() {
        // error_count == 0 → ttl_secs = 120
        let e = parser_to_cache_entry(1024, 128, 0);
        assert_eq!(e.ttl_secs, 120);
        assert_ne!(e.content_hash, 0);
    }

    #[test]
    fn test_parser_to_cache_entry_with_error_ttl() {
        // error_count > 0 → ttl_secs = 0
        let e = parser_to_cache_entry(1024, 128, 3);
        assert_eq!(e.ttl_secs, 0);
    }

    #[test]
    fn test_parser_to_analytics_metrics_basic() {
        let m = parser_to_analytics_metrics(512, 16, 0, 7000, 8192, 480);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.token_count, 512);
        assert_eq!(m.node_count, 480);
    }

    #[test]
    fn test_parser_to_compiler_handoff_basic() {
        let h = parser_to_compiler_handoff(480, 16, 512, 0, 8192);
        assert_ne!(h.content_hash, 0);
        assert_eq!(h.node_count, 480);
        assert_eq!(h.error_count, 0);
    }

    #[test]
    fn test_parser_to_compiler_handoff_with_errors() {
        let h = parser_to_compiler_handoff(200, 5, 210, 4, 1024);
        assert_ne!(h.content_hash, 0);
        assert_eq!(h.error_count, 4);
    }

    #[test]
    fn test_parser_to_api_response_basic() {
        let r = parser_to_api_response(300, 0, 2500, 10, 3000);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.token_count, 300);
        assert_eq!(r.error_count, 0);
        assert_eq!(r.parse_time_us, 2500);
    }
}
