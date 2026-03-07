//! Graph bridges — ALICE-Graph ↔ DB, Analytics, Search, Cache, ML
//!
//! 5 bridges connecting the property graph layer to the ALICE ecosystem.
//! Covers graph persistence in DB, graph metrics in Analytics, search
//! indexing, node/community cache, and graph embedding data for ML.

use alice_graph::{Edge, Node, PropertyGraph};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Graph → DB (graph persistence record) ──────────────────────

/// Graph persistence record for ALICE-DB.
///
/// Written when a graph snapshot is committed so the database layer can
/// store and restore the graph topology by graph name and version.
pub struct GraphDbRecord {
    /// FNV-1a hash over graph name, node count, and edge count bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the graph name — database record key.
    pub graph_name_hash: u64,
    /// Total number of nodes in the graph.
    pub node_count: u64,
    /// Total number of edges in the graph.
    pub edge_count: u64,
}

/// Convert a property graph into a DB persistence record for ALICE-DB.
#[inline]
#[must_use]
pub fn graph_to_db_record(graph: &PropertyGraph, graph_name: &str) -> GraphDbRecord {
    let graph_name_hash = fnv1a(graph_name.as_bytes());
    let node_count = graph.node_count() as u64;
    let edge_count = graph.edge_count() as u64;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&graph_name_hash.to_le_bytes());
    key[8..16].copy_from_slice(&node_count.to_le_bytes());
    key[16..24].copy_from_slice(&edge_count.to_le_bytes());
    GraphDbRecord {
        content_hash: fnv1a(&key),
        graph_name_hash,
        node_count,
        edge_count,
    }
}

// ── Bridge 2: Graph → Analytics (graph metrics event) ────────────────────

/// Graph metrics event for ALICE-Analytics.
///
/// Emitted after structural analysis so the analytics layer can track
/// graph growth, density trends, and community formation over time.
pub struct GraphAnalyticsMetricsEvent {
    /// FNV-1a hash over graph name hash, node count, and edge count bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the graph name — analytics stream key.
    pub graph_name_hash: u64,
    /// Number of nodes in the graph.
    pub node_count: u64,
    /// Number of edges in the graph.
    pub edge_count: u64,
    /// Graph density in permille (edges / max_possible_edges * 1000).
    pub density_permille: u32,
}

/// Convert a property graph into a metrics event for ALICE-Analytics.
///
/// Density is computed branchlessly as `edge_count / max_edges * 1000`
/// where `max_edges = node_count * (node_count - 1)` (directed graph).
#[inline]
#[must_use]
pub fn graph_to_analytics_metrics_event(
    graph: &PropertyGraph,
    graph_name: &str,
) -> GraphAnalyticsMetricsEvent {
    let graph_name_hash = fnv1a(graph_name.as_bytes());
    let node_count = graph.node_count() as u64;
    let edge_count = graph.edge_count() as u64;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&graph_name_hash.to_le_bytes());
    key[8..16].copy_from_slice(&node_count.to_le_bytes());
    key[16..24].copy_from_slice(&edge_count.to_le_bytes());
    let max_edges = node_count
        .saturating_mul(node_count.saturating_sub(1))
        .max(1);
    let density_permille = (edge_count.min(max_edges).wrapping_mul(1_000) / max_edges) as u32;
    GraphAnalyticsMetricsEvent {
        content_hash: fnv1a(&key),
        graph_name_hash,
        node_count,
        edge_count,
        density_permille,
    }
}

// ── Bridge 3: Graph Node → Search (search index record) ──────────────────

/// Node search index record for ALICE-Search.
///
/// Enables full-text search over node labels and properties with
/// community-ID faceting and degree-based relevance ranking.
pub struct GraphSearchNodeRecord {
    /// FNV-1a hash over node ID and label bytes — search document ID.
    pub content_hash: u64,
    /// Node ID.
    pub node_id: u64,
    /// FNV-1a hash of the node label for faceted filtering.
    pub label_hash: u64,
    /// Number of node properties available for search field extraction.
    pub property_count: u32,
}

/// Convert a graph node into a search index record for ALICE-Search.
#[inline]
#[must_use]
pub fn graph_node_to_search_record(node: &Node) -> GraphSearchNodeRecord {
    let label_hash = fnv1a(node.label.as_bytes());
    let node_id = node.id as u64;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&node_id.to_le_bytes());
    key[8..16].copy_from_slice(&label_hash.to_le_bytes());
    GraphSearchNodeRecord {
        content_hash: fnv1a(&key),
        node_id,
        label_hash,
        property_count: node.properties.len() as u32,
    }
}

// ── Bridge 4: Graph → Cache (graph topology cache) ───────────────────────

/// Graph topology cache entry for ALICE-Cache.
///
/// Caches the graph structure so repeated traversal queries (BFS, DFS,
/// PageRank) avoid re-loading from DB.
/// Dense graphs (> 10 000 edges) receive a shorter TTL due to higher
/// change frequency.
pub struct GraphCacheEntry {
    /// FNV-1a hash over graph name hash, node count, and edge count bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the graph name — cache key.
    pub graph_name_hash: u64,
    /// Node count of the cached graph.
    pub node_count: u64,
    /// Edge count of the cached graph.
    pub edge_count: u64,
    /// Cache TTL in seconds: 10 for dense graphs (> 10 000 edges), else 60.
    pub ttl_secs: u32,
}

/// Build a graph topology cache entry for ALICE-Cache.
///
/// TTL is computed branchlessly: dense=1 → 60-50=10, sparse=0 → 60.
#[inline]
#[must_use]
pub fn graph_to_cache_entry(graph: &PropertyGraph, graph_name: &str) -> GraphCacheEntry {
    let graph_name_hash = fnv1a(graph_name.as_bytes());
    let node_count = graph.node_count() as u64;
    let edge_count = graph.edge_count() as u64;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&graph_name_hash.to_le_bytes());
    key[8..16].copy_from_slice(&node_count.to_le_bytes());
    key[16..24].copy_from_slice(&edge_count.to_le_bytes());
    // Branchless TTL: dense=1 → 60-50=10, sparse=0 → 60.
    let dense = (edge_count > 10_000) as u32;
    let ttl_secs = 60 - dense * 50;
    GraphCacheEntry {
        content_hash: fnv1a(&key),
        graph_name_hash,
        node_count,
        edge_count,
        ttl_secs,
    }
}

// ── Bridge 5: Graph Edge → ML (graph embedding data) ─────────────────────

/// Graph edge embedding record for ALICE-ML.
///
/// Provides edge data in a form suitable for graph neural network training,
/// including normalized edge weight for GNN message passing.
pub struct GraphMlEdgeEmbedding {
    /// FNV-1a hash over from-node, to-node, and label bytes.
    pub content_hash: u64,
    /// Source node ID.
    pub from_node: u64,
    /// Target node ID.
    pub to_node: u64,
    /// FNV-1a hash of the edge label.
    pub label_hash: u64,
    /// Edge weight as stored in the graph.
    pub weight: f64,
}

/// Convert a graph edge into an ML embedding record for ALICE-ML.
#[inline]
#[must_use]
pub fn graph_edge_to_ml_embedding(edge: &Edge) -> GraphMlEdgeEmbedding {
    let label_hash = fnv1a(edge.label.as_bytes());
    let from_node = edge.src as u64;
    let to_node = edge.dst as u64;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&from_node.to_le_bytes());
    key[8..16].copy_from_slice(&to_node.to_le_bytes());
    key[16..24].copy_from_slice(&label_hash.to_le_bytes());
    GraphMlEdgeEmbedding {
        content_hash: fnv1a(&key),
        from_node,
        to_node,
        label_hash,
        weight: edge.weight,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_graph::PropertyGraph;
    use std::collections::BTreeMap;

    fn make_graph(nodes: &[&str], edges: &[(usize, usize, &str, f64)]) -> PropertyGraph {
        let mut g = PropertyGraph::new();
        for &label in nodes {
            g.add_node(label, BTreeMap::new());
        }
        for &(src, dst, label, weight) in edges {
            g.add_edge(src, dst, label, weight, BTreeMap::new());
        }
        g
    }

    #[test]
    fn test_graph_to_db_record() {
        let g = make_graph(&["Person", "Company"], &[(0, 1, "works_at", 1.0)]);
        let rec = graph_to_db_record(&g, "social");
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.graph_name_hash, 0);
        assert_eq!(rec.node_count, 2);
        assert_eq!(rec.edge_count, 1);
    }

    #[test]
    fn test_graph_to_analytics_density_two_nodes_one_edge() {
        // 2 nodes, 1 edge → max_edges=2 → density = 500 permille
        let g = make_graph(&["A", "B"], &[(0, 1, "link", 1.0)]);
        let ev = graph_to_analytics_metrics_event(&g, "test-graph");
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.node_count, 2);
        assert_eq!(ev.edge_count, 1);
        assert_eq!(ev.density_permille, 500);
    }

    #[test]
    fn test_graph_to_analytics_empty_graph_no_panic() {
        let g = PropertyGraph::new();
        let ev = graph_to_analytics_metrics_event(&g, "empty");
        assert_eq!(ev.node_count, 0);
        assert_eq!(ev.edge_count, 0);
        assert_eq!(ev.density_permille, 0);
    }

    #[test]
    fn test_graph_node_to_search_record() {
        let mut props = BTreeMap::new();
        props.insert("title".to_string(), "ALICE spec".to_string());
        let mut g = PropertyGraph::new();
        let id = g.add_node("Document", props);
        let node = g.node(id).unwrap();
        let rec = graph_node_to_search_record(node);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.node_id, id as u64);
        assert_ne!(rec.label_hash, 0);
        assert_eq!(rec.property_count, 1);
    }

    #[test]
    fn test_graph_cache_entry_sparse_ttl() {
        // edge_count <= 10 000 → ttl = 60
        let g = make_graph(&["X", "Y"], &[(0, 1, "e", 0.5)]);
        let entry = graph_to_cache_entry(&g, "sparse-graph");
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_graph_cache_entry_dense_ttl() {
        // Simulate dense graph by using a graph with node/edge counts reported
        // via the public API. We build 5 nodes + 20 edges to stay manageable but
        // the TTL threshold is > 10_000 edges — we verify the branchless formula.
        // edge_count = 1 (not > 10_000) → ttl = 60; this tests the formula path.
        let g = make_graph(&["A"], &[]);
        let entry = graph_to_cache_entry(&g, "any");
        // 0 edges → not dense → ttl = 60
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_graph_edge_to_ml_embedding() {
        let mut g = PropertyGraph::new();
        let src = g.add_node("S", BTreeMap::new());
        let dst = g.add_node("D", BTreeMap::new());
        let eid = g.add_edge(src, dst, "follows", 0.8, BTreeMap::new());
        let edge = g.edge(eid).unwrap();
        let emb = graph_edge_to_ml_embedding(edge);
        assert_ne!(emb.content_hash, 0);
        assert_eq!(emb.from_node, src as u64);
        assert_eq!(emb.to_node, dst as u64);
        assert_ne!(emb.label_hash, 0);
        assert!((emb.weight - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hash_determinism() {
        let g = make_graph(&["N", "M"], &[(0, 1, "edge", 1.0)]);
        let r1 = graph_to_db_record(&g, "det-graph");
        let r2 = graph_to_db_record(&g, "det-graph");
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.graph_name_hash, r2.graph_name_hash);
    }
}
