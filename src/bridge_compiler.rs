//! Compiler bridges — ALICE-Compiler ↔ DB, Cache, Analytics, ML, API
//!
//! 5 bridges connecting the compiler pipeline to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Compiler → DB (AST storage) ────────────────────────────────

/// AST storage record for ALICE-DB persistence.
pub struct CompilerDbRecord {
    /// Content hash over compilation-unit identifier + AST digest.
    pub content_hash: u64,
    /// FNV-1a hash of the compilation-unit identifier (e.g. source path).
    pub unit_id_hash: u64,
    /// FNV-1a hash of the serialised AST (for change detection).
    pub ast_digest: u64,
    /// Number of nodes in the AST.
    pub ast_node_count: u32,
    /// Number of syntax errors encountered.
    pub syntax_error_count: u16,
    /// Source size in bytes.
    pub source_bytes: u32,
    /// Compilation timestamp in nanoseconds.
    pub timestamp_ns: u64,
}

/// Serialize an AST for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn compiler_to_db_record(
    unit_id: &[u8],
    ast_bytes: &[u8],
    ast_node_count: u32,
    syntax_error_count: u16,
    source_bytes: u32,
    timestamp_ns: u64,
) -> CompilerDbRecord {
    let unit_id_hash = fnv1a(unit_id);
    let ast_digest = fnv1a(ast_bytes);
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&unit_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&ast_digest.to_le_bytes());
    key[16..24].copy_from_slice(&timestamp_ns.to_le_bytes());
    CompilerDbRecord {
        content_hash: fnv1a(&key),
        unit_id_hash,
        ast_digest,
        ast_node_count,
        syntax_error_count,
        source_bytes,
        timestamp_ns,
    }
}

// ── Bridge 2: Compiler → Cache (IR cache) ────────────────────────────────

/// Intermediate representation cache entry for ALICE-Cache.
pub struct CompilerCacheEntry {
    /// Content hash over unit + IR digest.
    pub content_hash: u64,
    /// FNV-1a hash of the compilation-unit identifier.
    pub unit_id_hash: u64,
    /// FNV-1a hash of the IR bytes.
    pub ir_digest: u64,
    /// Number of IR instructions.
    pub ir_instruction_count: u32,
    /// Cache TTL in seconds (shorter for units with errors).
    pub ttl_secs: u32,
}

/// Build an IR cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 60 s when the unit has syntax errors
/// so stale erroneous IR is not served for long.
#[inline]
#[must_use]
pub fn compiler_to_cache_entry(
    unit_id: &[u8],
    ir_bytes: &[u8],
    ir_instruction_count: u32,
    has_errors: bool,
) -> CompilerCacheEntry {
    let unit_id_hash = fnv1a(unit_id);
    let ir_digest = fnv1a(ir_bytes);
    // Branchless TTL: 3600 s for clean units, 60 s for units with errors.
    let error_flag = has_errors as u32;
    let ttl_secs = 3_600_u32 - error_flag * 3_540_u32;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&unit_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&ir_digest.to_le_bytes());
    CompilerCacheEntry {
        content_hash: fnv1a(&key),
        unit_id_hash,
        ir_digest,
        ir_instruction_count,
        ttl_secs,
    }
}

// ── Bridge 3: Compiler → Analytics (compile metrics) ─────────────────────

/// Compilation metrics for ALICE-Analytics ingestion.
pub struct CompilerAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total compilation units processed in the reporting window.
    pub units_compiled: u64,
    /// Total compile time across all units in microseconds.
    pub total_compile_us: u64,
    /// Average compile time per unit in microseconds.
    pub avg_compile_us: f64,
    /// Average number of optimisation passes applied.
    pub avg_optimization_passes: f32,
    /// Total output size in bytes across all units.
    pub total_output_bytes: u64,
    /// Window start timestamp in nanoseconds.
    pub window_start_ns: u64,
}

/// Build compilation metrics for ALICE-Analytics ingestion.
///
/// Averages use reciprocal multiply against `units_compiled`.
#[inline]
#[must_use]
pub fn compiler_to_analytics_metrics(
    units_compiled: u64,
    total_compile_us: u64,
    sum_optimization_passes: u64,
    total_output_bytes: u64,
    window_start_ns: u64,
) -> CompilerAnalyticsMetrics {
    let rcp = 1.0 / units_compiled.max(1) as f64;
    let avg_compile_us = total_compile_us as f64 * rcp;
    let avg_optimization_passes = (sum_optimization_passes as f64 * rcp) as f32;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&units_compiled.to_le_bytes());
    key[8..16].copy_from_slice(&total_compile_us.to_le_bytes());
    key[16..24].copy_from_slice(&window_start_ns.to_le_bytes());
    CompilerAnalyticsMetrics {
        content_hash: fnv1a(&key),
        units_compiled,
        total_compile_us,
        avg_compile_us,
        avg_optimization_passes,
        total_output_bytes,
        window_start_ns,
    }
}

// ── Bridge 4: Compiler → ML (code feature extraction) ────────────────────

/// Code feature vector for ALICE-ML (e.g. complexity prediction, bug detection).
pub struct CompilerMlFeatures {
    /// Content hash over the feature values.
    pub content_hash: u64,
    /// FNV-1a hash of the compilation unit (for model lookup).
    pub unit_id_hash: u64,
    /// Number of AST nodes (raw feature).
    pub ast_node_count: u32,
    /// Number of IR instructions (raw feature).
    pub ir_instruction_count: u32,
    /// Number of optimisation passes applied.
    pub optimization_passes: u16,
    /// Compile time in microseconds (raw feature).
    pub compile_time_us: u32,
    /// Output binary size in bytes.
    pub output_size: u32,
    /// Cyclomatic complexity estimate (IR branch count).
    pub cyclomatic_complexity: u32,
}

/// Extract code features for ALICE-ML.
#[inline]
#[must_use]
pub fn compiler_to_ml_features(
    unit_id: &[u8],
    ast_node_count: u32,
    ir_instruction_count: u32,
    optimization_passes: u16,
    compile_time_us: u32,
    output_size: u32,
    cyclomatic_complexity: u32,
) -> CompilerMlFeatures {
    let unit_id_hash = fnv1a(unit_id);
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&unit_id_hash.to_le_bytes());
    key[8..12].copy_from_slice(&ast_node_count.to_le_bytes());
    key[12..16].copy_from_slice(&ir_instruction_count.to_le_bytes());
    key[16..20].copy_from_slice(&compile_time_us.to_le_bytes());
    CompilerMlFeatures {
        content_hash: fnv1a(&key),
        unit_id_hash,
        ast_node_count,
        ir_instruction_count,
        optimization_passes,
        compile_time_us,
        output_size,
        cyclomatic_complexity,
    }
}

// ── Bridge 5: Compiler → API (compilation service response) ──────────────

/// Compilation service response for ALICE-API.
pub struct CompilerApiResponse {
    /// Content hash over unit + output digest.
    pub content_hash: u64,
    /// FNV-1a hash of the compilation-unit identifier.
    pub unit_id_hash: u64,
    /// FNV-1a hash of the compiled output bytes.
    pub output_digest: u64,
    /// Compile time in microseconds.
    pub compile_time_us: u32,
    /// Output binary size in bytes.
    pub output_size: u32,
    /// Number of warnings emitted.
    pub warning_count: u16,
    /// Number of errors emitted (0 = success).
    pub error_count: u16,
    /// Response timestamp in nanoseconds.
    pub timestamp_ns: u64,
}

/// Build a compilation service response for ALICE-API.
#[inline]
#[must_use]
pub fn compiler_to_api_response(
    unit_id: &[u8],
    output_bytes: &[u8],
    compile_time_us: u32,
    warning_count: u16,
    error_count: u16,
    timestamp_ns: u64,
) -> CompilerApiResponse {
    let unit_id_hash = fnv1a(unit_id);
    let output_digest = fnv1a(output_bytes);
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&unit_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&output_digest.to_le_bytes());
    key[16..24].copy_from_slice(&timestamp_ns.to_le_bytes());
    CompilerApiResponse {
        content_hash: fnv1a(&key),
        unit_id_hash,
        output_digest,
        compile_time_us,
        output_size: output_bytes.len() as u32,
        warning_count,
        error_count,
        timestamp_ns,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const UNIT: &[u8] = b"src/main.alice";
    const AST: &[u8] = b"<ast-serialised-bytes>";
    const IR: &[u8] = b"<ir-serialised-bytes>";
    const OUT: &[u8] = b"\x7fELF compiled output bytes";

    #[test]
    fn test_compiler_to_db_record_hash_nonzero() {
        let rec = compiler_to_db_record(UNIT, AST, 512, 0, 4_096, 1_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.unit_id_hash, 0);
        assert_ne!(rec.ast_digest, 0);
    }

    #[test]
    fn test_compiler_to_db_record_fields() {
        let rec = compiler_to_db_record(UNIT, AST, 256, 2, 2_048, 2_000_000_000);
        assert_eq!(rec.ast_node_count, 256);
        assert_eq!(rec.syntax_error_count, 2);
        assert_eq!(rec.source_bytes, 2_048);
    }

    #[test]
    fn test_compiler_to_cache_entry_clean_ttl() {
        let entry = compiler_to_cache_entry(UNIT, IR, 1_024, false);
        assert_eq!(entry.ttl_secs, 3_600);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_compiler_to_cache_entry_error_ttl() {
        // Unit with errors → TTL = 60 s.
        let entry = compiler_to_cache_entry(UNIT, IR, 0, true);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_compiler_to_analytics_metrics_avg() {
        // 50 units, 500_000 us total → avg 10_000 us, sum_passes = 200 → avg 4.0.
        let m = compiler_to_analytics_metrics(50, 500_000, 200, 1_048_576, 0);
        assert_ne!(m.content_hash, 0);
        assert!((m.avg_compile_us - 10_000.0).abs() < 1.0);
        assert!((m.avg_optimization_passes - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_compiler_to_analytics_metrics_zero_units() {
        let m = compiler_to_analytics_metrics(0, 0, 0, 0, 0);
        assert_eq!(m.units_compiled, 0);
        assert_eq!(m.avg_compile_us, 0.0);
    }

    #[test]
    fn test_compiler_to_ml_features_fields() {
        let f = compiler_to_ml_features(UNIT, 1024, 4096, 3, 8_500, 65_536, 42);
        assert_ne!(f.content_hash, 0);
        assert_eq!(f.ast_node_count, 1024);
        assert_eq!(f.ir_instruction_count, 4096);
        assert_eq!(f.optimization_passes, 3);
        assert_eq!(f.cyclomatic_complexity, 42);
    }

    #[test]
    fn test_compiler_to_api_response_deterministic() {
        let a = compiler_to_api_response(UNIT, OUT, 7_200, 1, 0, 999_999_999);
        let b = compiler_to_api_response(UNIT, OUT, 7_200, 1, 0, 999_999_999);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.output_digest, b.output_digest);
        assert_eq!(a.output_size, OUT.len() as u32);
        assert_eq!(a.warning_count, 1);
        assert_eq!(a.error_count, 0);
    }
}
