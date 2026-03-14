//! LLM science bridges — ALICE-LLM ↔ Bio, Kinematics, Quant, Graph, Geo
//!
//! 5 bridges connecting LLM inference to scientific and analytical services.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LLM → Bio (biological sequence analysis) ──────────────────

/// Biological sequence analysis request for ALICE-Bio.
pub struct LlmBioSequence {
    /// Content hash over the sequence request.
    pub content_hash: u64,
    /// Sequence length in residues/bases.
    pub seq_len: u64,
    /// Sequence type (0=protein, 1=DNA, 2=RNA, 3=multi_chain).
    pub seq_type: u8,
    /// Embedding dimension for the bio-LLM.
    pub embed_dim: u32,
    /// Estimated token count (3 chars per token for protein, 6 for nucleotide).
    pub estimated_tokens: u64,
    /// Analysis mode (0=structure_predict, 1=function_annotate, 2=variant_effect, 3=alignment).
    pub mode: u8,
}

/// Build a bio sequence analysis request from sequence metadata.
///
/// Token estimate: protein ~3 chars/token, nucleotide ~6 chars/token.
#[inline]
#[must_use]
pub fn llm_to_bio_sequence(seq_len: u64, seq_type: u8, embed_dim: u32, mode: u8) -> LlmBioSequence {
    let mut buf = [0u8; 14];
    buf[0..8].copy_from_slice(&seq_len.to_le_bytes());
    buf[8] = seq_type;
    buf[9..13].copy_from_slice(&embed_dim.to_le_bytes());
    buf[13] = mode;
    // Protein: ~3 chars/token, nucleotide: ~6 chars/token
    let chars_per_tok = if seq_type == 0 { 3u64 } else { 6 };
    let estimated_tokens = seq_len.div_ceil(chars_per_tok);
    LlmBioSequence {
        content_hash: fnv1a(&buf),
        seq_len,
        seq_type,
        embed_dim,
        estimated_tokens,
        mode,
    }
}

// ── Bridge 2: LLM → Kinematics (motion planning description) ───────────

/// Motion planning request for ALICE-Kinematics.
pub struct LlmKinematicsRequest {
    /// Content hash over the kinematics request.
    pub content_hash: u64,
    /// Number of joints in the kinematic chain.
    pub joint_count: u32,
    /// Number of trajectory waypoints.
    pub waypoint_count: u32,
    /// Degrees of freedom.
    pub dof: u32,
    /// Text description token count for the motion command.
    pub command_tokens: u32,
    /// Estimated planning time in milliseconds.
    pub estimated_plan_ms: u32,
}

/// Build a kinematics request from motion description.
///
/// Planning time estimate: ~2ms per joint × waypoint × DOF.
#[inline]
#[must_use]
pub fn llm_to_kinematics_request(
    joint_count: u32,
    waypoint_count: u32,
    dof: u32,
    command_tokens: u32,
) -> LlmKinematicsRequest {
    let mut buf = [0u8; 16];
    buf[0..4].copy_from_slice(&joint_count.to_le_bytes());
    buf[4..8].copy_from_slice(&waypoint_count.to_le_bytes());
    buf[8..12].copy_from_slice(&dof.to_le_bytes());
    buf[12..16].copy_from_slice(&command_tokens.to_le_bytes());
    // ~2ms per joint × waypoint complexity factor
    let complexity = joint_count as u64 * waypoint_count as u64 * dof as u64;
    let estimated_plan_ms = ((complexity * 2) / 1000).max(1) as u32;
    LlmKinematicsRequest {
        content_hash: fnv1a(&buf),
        joint_count,
        waypoint_count,
        dof,
        command_tokens,
        estimated_plan_ms,
    }
}

// ── Bridge 3: LLM → Quant (quantitative model interpretation) ──────────

/// Quantitative model interpretation request for ALICE-Quant.
pub struct LlmQuantSignal {
    /// Content hash over the quant signal.
    pub content_hash: u64,
    /// Number of time-series data points.
    pub data_points: u64,
    /// Feature dimension per data point.
    pub feature_dim: u32,
    /// Signal type (0=price_action, 1=volatility, 2=sentiment, 3=correlation).
    pub signal_type: u8,
    /// LLM explanation token count.
    pub explanation_tokens: u32,
    /// Estimated memory for time-series in bytes (data_points × feature_dim × 4).
    pub memory_bytes: u64,
}

/// Build a quant signal interpretation request from market data.
#[inline]
#[must_use]
pub fn llm_to_quant_signal(
    data_points: u64,
    feature_dim: u32,
    signal_type: u8,
    explanation_tokens: u32,
) -> LlmQuantSignal {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&data_points.to_le_bytes());
    buf[8..12].copy_from_slice(&feature_dim.to_le_bytes());
    buf[12] = signal_type;
    buf[13..17].copy_from_slice(&explanation_tokens.to_le_bytes());
    let memory_bytes = data_points * feature_dim as u64 * 4;
    LlmQuantSignal {
        content_hash: fnv1a(&buf),
        data_points,
        feature_dim,
        signal_type,
        explanation_tokens,
        memory_bytes,
    }
}

// ── Bridge 4: LLM → Graph (graph query generation / reasoning) ──────────

/// Graph query request for ALICE-Graph.
pub struct LlmGraphQuery {
    /// Content hash over the graph query.
    pub content_hash: u64,
    /// Number of nodes in the target graph.
    pub node_count: u64,
    /// Number of edges in the target graph.
    pub edge_count: u64,
    /// Query type (0=path_find, 1=pattern_match, 2=subgraph_extract, 3=reasoning).
    pub query_type: u8,
    /// Natural language query token count.
    pub query_tokens: u32,
    /// Estimated query complexity (log2(nodes) × log2(edges)).
    pub complexity_score: f32,
}

/// Build a graph query request from natural language.
///
/// Complexity score: log2(nodes) × log2(edges) as a rough BFS/DFS estimate.
#[inline]
#[must_use]
pub fn llm_to_graph_query(
    node_count: u64,
    edge_count: u64,
    query_type: u8,
    query_tokens: u32,
) -> LlmGraphQuery {
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&node_count.to_le_bytes());
    buf[8..16].copy_from_slice(&edge_count.to_le_bytes());
    buf[16] = query_type;
    buf[17..21].copy_from_slice(&query_tokens.to_le_bytes());
    let log_n = if node_count > 1 {
        (node_count as f64).log2()
    } else {
        1.0
    };
    let log_e = if edge_count > 1 {
        (edge_count as f64).log2()
    } else {
        1.0
    };
    let complexity_score = (log_n * log_e) as f32;
    LlmGraphQuery {
        content_hash: fnv1a(&buf),
        node_count,
        edge_count,
        query_type,
        query_tokens,
        complexity_score,
    }
}

// ── Bridge 5: LLM → Geo (geospatial query / location intelligence) ─────

/// Geospatial query request for ALICE-Geo.
pub struct LlmGeoQuery {
    /// Content hash over the geo query.
    pub content_hash: u64,
    /// Latitude in microdegrees (lat × 1e6).
    pub lat_micro: i32,
    /// Longitude in microdegrees (lon × 1e6).
    pub lon_micro: i32,
    /// Search radius in meters.
    pub radius_m: u32,
    /// Query type (0=poi_search, 1=route_plan, 2=area_analysis, 3=geocode).
    pub query_type: u8,
    /// Natural language query token count.
    pub query_tokens: u32,
}

/// Build a geospatial query from location and natural language.
#[inline]
#[must_use]
pub fn llm_to_geo_query(
    lat_micro: i32,
    lon_micro: i32,
    radius_m: u32,
    query_type: u8,
    query_tokens: u32,
) -> LlmGeoQuery {
    let mut buf = [0u8; 17];
    buf[0..4].copy_from_slice(&lat_micro.to_le_bytes());
    buf[4..8].copy_from_slice(&lon_micro.to_le_bytes());
    buf[8..12].copy_from_slice(&radius_m.to_le_bytes());
    buf[12] = query_type;
    buf[13..17].copy_from_slice(&query_tokens.to_le_bytes());
    LlmGeoQuery {
        content_hash: fnv1a(&buf),
        lat_micro,
        lon_micro,
        radius_m,
        query_type,
        query_tokens,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bio_sequence_protein() {
        let b = llm_to_bio_sequence(300, 0, 1280, 0);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.seq_type, 0);
        assert_eq!(b.estimated_tokens, 100); // 300 / 3
        assert_eq!(b.mode, 0);
    }

    #[test]
    fn test_bio_sequence_dna() {
        let b = llm_to_bio_sequence(6000, 1, 768, 2);
        assert_eq!(b.estimated_tokens, 1000); // 6000 / 6
        assert_eq!(b.seq_type, 1);
    }

    #[test]
    fn test_kinematics_request_planning() {
        let k = llm_to_kinematics_request(6, 10, 6, 32);
        assert_ne!(k.content_hash, 0);
        assert_eq!(k.joint_count, 6);
        assert_eq!(k.dof, 6);
        // 6 * 10 * 6 * 2 / 1000 = 0.72 → max(1) = 1
        assert!(k.estimated_plan_ms >= 1);
    }

    #[test]
    fn test_kinematics_hash_determinism() {
        let a = llm_to_kinematics_request(7, 20, 7, 64);
        let b = llm_to_kinematics_request(7, 20, 7, 64);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_quant_signal_price_action() {
        let q = llm_to_quant_signal(10_000, 64, 0, 256);
        assert_ne!(q.content_hash, 0);
        assert_eq!(q.signal_type, 0);
        assert_eq!(q.memory_bytes, 10_000 * 64 * 4); // 2.56 MB
    }

    #[test]
    fn test_graph_query_reasoning() {
        let g = llm_to_graph_query(1_000_000, 5_000_000, 3, 128);
        assert_ne!(g.content_hash, 0);
        assert_eq!(g.query_type, 3);
        assert!(g.complexity_score > 0.0);
        // log2(1M) ≈ 19.9, log2(5M) ≈ 22.3 → ~444
        assert!(g.complexity_score > 400.0);
    }

    #[test]
    fn test_graph_query_single_node() {
        let g = llm_to_graph_query(1, 0, 0, 16);
        // log2(1)=1.0 (clamped), log2(0)→1.0 (clamped)
        assert!((g.complexity_score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_geo_query_poi_search() {
        // Tokyo: 35.6762° N, 139.6503° E → 35676200, 139650300 microdegrees
        let g = llm_to_geo_query(35_676_200, 139_650_300, 500, 0, 32);
        assert_ne!(g.content_hash, 0);
        assert_eq!(g.lat_micro, 35_676_200);
        assert_eq!(g.radius_m, 500);
        assert_eq!(g.query_type, 0);
    }

    #[test]
    fn test_geo_query_hash_determinism() {
        let a = llm_to_geo_query(35_676_200, 139_650_300, 1000, 1, 64);
        let b = llm_to_geo_query(35_676_200, 139_650_300, 1000, 1, 64);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
