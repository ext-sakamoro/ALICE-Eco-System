//! Cross-domain bridges — ALICE-Graph ↔ VectorDB, ML, Analytics, Search, Cache
//!
//! 5 bridges connecting graph structures to vector embedding, ML feature
//! extraction, analytics metrics, search indexing, and cache.

use alice_graph::{detect_communities, dijkstra, pagerank, Edge, Node, PropertyGraph, ShortestPath};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Graph Node → VectorDB vector record for embedding ──────

/// A graph node converted into a VectorDB-compatible vector record.
///
/// Encodes node id, label hash, and property count into a fixed-dimension
/// embedding seed so VectorDB can store and retrieve graph node embeddings.
pub struct GraphNodeVectorRecord {
    /// FNV-1a hash over node id, label, property count.
    pub content_hash: u64,
    /// Original node id.
    pub node_id: usize,
    /// Hash of the node label (used as a categorical embedding seed).
    pub label_hash: u64,
    /// Number of properties on the node.
    pub property_count: usize,
    /// VectorDB record id string: "graph_node_{id}".
    pub record_id: [u8; 32],
    /// Length of the record_id string.
    pub record_id_len: usize,
    /// Embedding dimension (fixed at 4: node_id_f32, label_hash_f32, prop_count, degree).
    pub embed_dim: usize,
}

/// Convert a graph node into a VectorDB vector record descriptor.
#[inline]
#[must_use]
pub fn graph_node_to_vectordb_record(node: &Node, graph: &PropertyGraph) -> GraphNodeVectorRecord {
    let degree = graph.out_degree(node.id);
    let label_hash = fnv1a(node.label.as_bytes());

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&(node.id as u64).to_le_bytes());
    key[8..16].copy_from_slice(&label_hash.to_le_bytes());
    key[16..24].copy_from_slice(&(node.properties.len() as u64).to_le_bytes());
    key[24..32].copy_from_slice(&(degree as u64).to_le_bytes());

    // Build record id string
    let mut record_id = [0u8; 32];
    let prefix = b"graph_node_";
    let id_str = itoa_small(node.id);
    let total_len = (prefix.len() + id_str.1).min(32);
    record_id[..prefix.len().min(32)].copy_from_slice(&prefix[..prefix.len().min(32)]);
    let remaining = 32 - prefix.len().min(32);
    let copy_len = id_str.1.min(remaining);
    record_id[prefix.len()..prefix.len() + copy_len].copy_from_slice(&id_str.0[..copy_len]);

    GraphNodeVectorRecord {
        content_hash: fnv1a(&key),
        node_id: node.id,
        label_hash,
        property_count: node.properties.len(),
        record_id,
        record_id_len: total_len,
        embed_dim: 4,
    }
}

/// Helper: convert usize to decimal bytes (max 20 digits).
fn itoa_small(mut v: usize) -> ([u8; 20], usize) {
    let mut buf = [0u8; 20];
    if v == 0 {
        buf[0] = b'0';
        return (buf, 1);
    }
    let mut i = 20;
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    let len = 20 - i;
    let mut out = [0u8; 20];
    out[..len].copy_from_slice(&buf[i..]);
    (out, len)
}

// ── Bridge 2: Graph Edge → ML feature vector ──────────────────────────

/// A graph edge converted into an ML feature vector.
///
/// Extracts numeric features from an edge (weight, src/dst degree, label hash)
/// so the ML layer can use graph edge data for link prediction or
/// classification without accessing the graph directly.
pub struct GraphEdgeMlFeature {
    /// FNV-1a hash over edge id, weight, src, dst, label hash.
    pub content_hash: u64,
    /// Original edge id.
    pub edge_id: usize,
    /// Edge weight as raw f64.
    pub weight: f64,
    /// Source node out-degree.
    pub src_degree: usize,
    /// Destination node out-degree.
    pub dst_degree: usize,
    /// Hash of the edge label.
    pub label_hash: u64,
    /// Feature dimension: always 5 (weight, src_degree, dst_degree, label_hash_lo, label_hash_hi).
    pub feature_dim: usize,
}

/// Convert a graph edge into an ML feature vector descriptor.
#[inline]
#[must_use]
pub fn graph_edge_to_ml_feature(edge: &Edge, graph: &PropertyGraph) -> GraphEdgeMlFeature {
    let src_degree = graph.out_degree(edge.src);
    let dst_degree = graph.out_degree(edge.dst);
    let label_hash = fnv1a(edge.label.as_bytes());

    let mut key = [0u8; 48];
    key[0..8].copy_from_slice(&(edge.id as u64).to_le_bytes());
    key[8..16].copy_from_slice(&edge.weight.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&(edge.src as u64).to_le_bytes());
    key[24..32].copy_from_slice(&(edge.dst as u64).to_le_bytes());
    key[32..40].copy_from_slice(&label_hash.to_le_bytes());
    key[40..48].copy_from_slice(&(src_degree as u64).to_le_bytes());

    GraphEdgeMlFeature {
        content_hash: fnv1a(&key),
        edge_id: edge.id,
        weight: edge.weight,
        src_degree,
        dst_degree,
        label_hash,
        feature_dim: 5,
    }
}

// ── Bridge 3: PageRank scores → Analytics metrics ─────────────────────

/// PageRank scores converted into Analytics-compatible metrics.
///
/// Summarizes a PageRank result vector into aggregate statistics
/// (min, max, mean, top-k node ids) for the Analytics pipeline.
pub struct GraphPagerankAnalytics {
    /// FNV-1a hash over node_count, min, max, mean, sum bytes.
    pub content_hash: u64,
    /// Number of nodes in the graph.
    pub node_count: usize,
    /// Minimum PageRank score.
    pub min_score: f64,
    /// Maximum PageRank score.
    pub max_score: f64,
    /// Mean PageRank score.
    pub mean_score: f64,
    /// Sum of all PageRank scores.
    pub sum_score: f64,
    /// Node id with the highest PageRank.
    pub top_node_id: usize,
    /// Metric name hash for Analytics pipeline registration.
    pub metric_name_hash: u64,
}

/// Convert PageRank scores into Analytics metrics.
#[inline]
#[must_use]
pub fn graph_pagerank_to_analytics(scores: &[f64]) -> GraphPagerankAnalytics {
    let node_count = scores.len();
    let (mut min_s, mut max_s, mut sum_s, mut top_id) = (f64::INFINITY, f64::NEG_INFINITY, 0.0, 0);

    for (i, &s) in scores.iter().enumerate() {
        if s < min_s { min_s = s; }
        if s > max_s { max_s = s; top_id = i; }
        sum_s += s;
    }

    if node_count == 0 {
        min_s = 0.0;
        max_s = 0.0;
    }

    let mean_s = if node_count > 0 { sum_s / node_count as f64 } else { 0.0 };
    let metric_name_hash = fnv1a(b"graph.pagerank");

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&(node_count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&min_s.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&max_s.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&mean_s.to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&sum_s.to_bits().to_le_bytes());

    GraphPagerankAnalytics {
        content_hash: fnv1a(&key),
        node_count,
        min_score: min_s,
        max_score: max_s,
        mean_score: mean_s,
        sum_score: sum_s,
        top_node_id: top_id,
        metric_name_hash,
    }
}

// ── Bridge 4: Graph community detection → Search index ────────────────

/// Graph community membership converted into a Search index record.
///
/// Encodes community assignment and community size so the Search layer
/// can index and query by community membership.
pub struct GraphCommunitySearchRecord {
    /// FNV-1a hash over node_id, community_id, community_size, total_communities.
    pub content_hash: u64,
    /// Node id.
    pub node_id: usize,
    /// Community id this node belongs to.
    pub community_id: usize,
    /// Number of nodes in this community.
    pub community_size: usize,
    /// Total number of distinct communities.
    pub total_communities: usize,
    /// Search document bytes: "community:{community_id}:node:{node_id}".
    pub search_key: [u8; 48],
    /// Length of the search_key.
    pub search_key_len: usize,
}

/// Convert a graph community detection result into Search index records for a single node.
#[inline]
#[must_use]
pub fn graph_community_to_search_record(
    node_id: usize,
    communities: &[usize],
) -> GraphCommunitySearchRecord {
    let community_id = communities.get(node_id).copied().unwrap_or(0);

    // Count members in this community
    let community_size = communities.iter().filter(|&&c| c == community_id).count();

    // Count distinct communities
    let mut seen = [false; 256];
    let mut total_communities = 0usize;
    for &c in communities {
        let slot = c & 0xFF;
        if !seen[slot] {
            seen[slot] = true;
            total_communities += 1;
        }
    }

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&(node_id as u64).to_le_bytes());
    key[8..16].copy_from_slice(&(community_id as u64).to_le_bytes());
    key[16..24].copy_from_slice(&(community_size as u64).to_le_bytes());
    key[24..32].copy_from_slice(&(total_communities as u64).to_le_bytes());

    // Build search key
    let mut search_key = [0u8; 48];
    let prefix = b"community:";
    let cid = itoa_small(community_id);
    let mid = b":node:";
    let nid = itoa_small(node_id);
    let mut pos = 0usize;
    for &b in prefix.iter().chain(&cid.0[..cid.1]).chain(mid.iter()).chain(&nid.0[..nid.1]) {
        if pos >= 48 { break; }
        search_key[pos] = b;
        pos += 1;
    }

    GraphCommunitySearchRecord {
        content_hash: fnv1a(&key),
        node_id,
        community_id,
        community_size,
        total_communities,
        search_key,
        search_key_len: pos,
    }
}

// ── Bridge 5: Shortest path → Cache ──────────────────────────────────

/// Shortest path result converted into a Cache entry.
///
/// Encodes a Dijkstra shortest path result so it can be cached and
/// retrieved without recomputation. TTL is branchless-adjusted based
/// on path length.
pub struct GraphPathCache {
    /// FNV-1a hash over src, dst, distance, hop_count bytes.
    pub content_hash: u64,
    /// Source node id.
    pub src: usize,
    /// Destination node id.
    pub dst: usize,
    /// Total distance.
    pub distance: f64,
    /// Number of hops (edges) in the path.
    pub hop_count: usize,
    /// TTL in seconds. Long paths (>5 hops) get shorter TTL (less stable).
    pub ttl_secs: u32,
    /// Cache key hash for direct lookup.
    pub cache_key: u64,
}

/// Convert a shortest path result into a Cache entry.
#[inline]
#[must_use]
pub fn graph_path_to_cache(src: usize, dst: usize, path: &ShortestPath) -> GraphPathCache {
    let hop_count = if path.path.len() > 1 { path.path.len() - 1 } else { 0 };

    // Branchless TTL: long paths (>5 hops) get 60s less
    let long_path = (hop_count > 5) as u32;
    let ttl_secs: u32 = 300 - long_path * 60;

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&(src as u64).to_le_bytes());
    key[8..16].copy_from_slice(&(dst as u64).to_le_bytes());
    key[16..24].copy_from_slice(&path.distance.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&(hop_count as u64).to_le_bytes());

    let cache_key = fnv1a(&key);

    GraphPathCache {
        content_hash: fnv1a(&key),
        src,
        dst,
        distance: path.distance,
        hop_count,
        ttl_secs,
        cache_key,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_graph::{detect_communities, dijkstra, pagerank, PropertyGraph};
    use std::collections::BTreeMap;

    fn test_graph() -> PropertyGraph {
        let mut g = PropertyGraph::new();
        let a = g.add_node("Person", BTreeMap::new());
        let b = g.add_node("Person", BTreeMap::new());
        let c = g.add_node("Person", BTreeMap::new());
        let d = g.add_node("Person", BTreeMap::new());
        g.add_edge(a, b, "KNOWS", 1.0, BTreeMap::new());
        g.add_edge(b, c, "KNOWS", 2.0, BTreeMap::new());
        g.add_edge(a, c, "KNOWS", 10.0, BTreeMap::new());
        g.add_edge(c, d, "KNOWS", 1.0, BTreeMap::new());
        g
    }

    // ── Bridge 1: node → vectordb record ─────────────────────────────

    #[test]
    fn test_graph_node_to_vectordb_record() {
        let g = test_graph();
        let node = g.node(0).unwrap();
        let rec = graph_node_to_vectordb_record(node, &g);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.node_id, 0);
        assert_eq!(rec.embed_dim, 4);
        assert!(rec.record_id_len > 0);
    }

    #[test]
    fn test_graph_node_to_vectordb_record_deterministic() {
        let g = test_graph();
        let node = g.node(1).unwrap();
        let r1 = graph_node_to_vectordb_record(node, &g);
        let r2 = graph_node_to_vectordb_record(node, &g);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 2: edge → ml feature ──────────────────────────────────

    #[test]
    fn test_graph_edge_to_ml_feature() {
        let g = test_graph();
        let edge = g.edge(0).unwrap();
        let feat = graph_edge_to_ml_feature(edge, &g);
        assert_ne!(feat.content_hash, 0);
        assert_eq!(feat.edge_id, 0);
        assert!((feat.weight - 1.0).abs() < 1e-10);
        assert_eq!(feat.feature_dim, 5);
        assert_eq!(feat.src_degree, 2); // node 0 has 2 outgoing edges
    }

    #[test]
    fn test_graph_edge_to_ml_feature_deterministic() {
        let g = test_graph();
        let edge = g.edge(1).unwrap();
        let f1 = graph_edge_to_ml_feature(edge, &g);
        let f2 = graph_edge_to_ml_feature(edge, &g);
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    // ── Bridge 3: pagerank → analytics ───────────────────────────────

    #[test]
    fn test_graph_pagerank_to_analytics() {
        let g = test_graph();
        let scores = pagerank(&g, 0.85, 50);
        let analytics = graph_pagerank_to_analytics(&scores);
        assert_ne!(analytics.content_hash, 0);
        assert_eq!(analytics.node_count, 4);
        assert!(analytics.min_score > 0.0);
        assert!(analytics.max_score > analytics.min_score);
        assert!((analytics.sum_score - 1.0).abs() < 0.01);
        assert_ne!(analytics.metric_name_hash, 0);
    }

    #[test]
    fn test_graph_pagerank_to_analytics_empty() {
        let analytics = graph_pagerank_to_analytics(&[]);
        assert_eq!(analytics.node_count, 0);
        assert!((analytics.mean_score - 0.0).abs() < 1e-10);
    }

    // ── Bridge 4: community → search record ──────────────────────────

    #[test]
    fn test_graph_community_to_search_record() {
        let g = test_graph();
        let communities = detect_communities(&g);
        let rec = graph_community_to_search_record(0, &communities);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.node_id, 0);
        assert!(rec.community_size > 0);
        assert!(rec.total_communities >= 1);
        assert!(rec.search_key_len > 0);
    }

    // ── Bridge 5: shortest path → cache ──────────────────────────────

    #[test]
    fn test_graph_path_to_cache() {
        let g = test_graph();
        let sp = dijkstra(&g, 0, 3).unwrap();
        let cache = graph_path_to_cache(0, 3, &sp);
        assert_ne!(cache.content_hash, 0);
        assert_eq!(cache.src, 0);
        assert_eq!(cache.dst, 3);
        assert!((cache.distance - 4.0).abs() < 0.01);
        assert_eq!(cache.hop_count, 3); // 0→1→2→3 = 3 hops
        assert_eq!(cache.ttl_secs, 300); // <= 5 hops → full TTL
    }

    #[test]
    fn test_graph_path_to_cache_long_path_ttl() {
        // Simulate a long path with >5 hops
        let sp = ShortestPath {
            distance: 10.0,
            path: vec![0, 1, 2, 3, 4, 5, 6, 7],
        };
        let cache = graph_path_to_cache(0, 7, &sp);
        assert_eq!(cache.hop_count, 7);
        // Branchless: 300 - 1 * 60 = 240
        assert_eq!(cache.ttl_secs, 240);
    }

    #[test]
    fn test_graph_path_to_cache_deterministic() {
        let g = test_graph();
        let sp = dijkstra(&g, 0, 3).unwrap();
        let c1 = graph_path_to_cache(0, 3, &sp);
        let c2 = graph_path_to_cache(0, 3, &sp);
        assert_eq!(c1.content_hash, c2.content_hash);
        assert_eq!(c1.cache_key, c2.cache_key);
    }
}
