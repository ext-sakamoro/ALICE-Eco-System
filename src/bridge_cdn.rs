//! CDN core bridges — ALICE-CDN ↔ Analytics, DB, Auth
//!
//! 3 bridges connecting the core CDN routing engine to the ALICE ecosystem.
//! Covers routing-decision metrics, node health persistence, and content
//! access control metadata.

use alice_cdn::{MaglevHash, NodeId, VivaldiCoord};
use crate::hash::fnv1a;

// ── Bridge 1: CDN → Analytics (routing decision metrics) ─────────────────

/// Routing decision metrics for ALICE-Analytics ingestion.
///
/// Emitted after each routing pass so the analytics pipeline can build
/// latency histograms, cache-hit distributions, and node-utilization reports.
pub struct CdnRoutingMetrics {
    /// FNV-1a hash of `content_id` — analytics stream key.
    pub content_hash: u64,
    /// Number of CDN nodes considered during routing.
    pub node_count: u32,
    /// Average Vivaldi-predicted RTT across all candidate nodes, in milliseconds.
    pub avg_latency_ms: u64,
    /// Cache hit rate in permille [0, 1000] (fixed-point, no float).
    pub cache_hit_rate_permille: u32,
    /// Node ID selected by the routing pass (Maglev lookup result).
    pub selected_node_id: NodeId,
}

/// Compute CDN routing decision metrics for ALICE-Analytics.
///
/// `hits` and `total` feed the hit-rate permille using a branchless
/// reciprocal multiply (no branch on `total == 0`).  `rtt_samples_ms`
/// is the slice of per-node Vivaldi RTT predictions; average is computed
/// with a single linear fold, no heap allocation.
#[inline]
pub fn cdn_to_routing_metrics(
    content_id: u64,
    rtt_samples_ms: &[u64],
    hits: u64,
    total: u64,
    maglev: &MaglevHash,
) -> CdnRoutingMetrics {
    let content_hash = fnv1a(&content_id.to_le_bytes());

    // Node count: length of the RTT sample slice.
    let node_count = rtt_samples_ms.len() as u32;

    // Average RTT: single linear fold, no heap allocation.
    // Guard against empty slice using saturating denominator.
    let count_safe = (rtt_samples_ms.len() as u64).max(1);
    let sum_rtt: u64 = rtt_samples_ms.iter().fold(0u64, |acc, &r| acc.wrapping_add(r));
    let avg_latency_ms = sum_rtt / count_safe;

    // Cache hit rate permille: hits * 1000 / total.
    // Branchless: max(total, 1) guard compiled to cmov.
    let total_safe = total.max(1);
    let cache_hit_rate_permille = (hits.min(total_safe).wrapping_mul(1_000) / total_safe) as u32;

    // Maglev O(1) lookup — deterministic node assignment.
    let selected_node_id = maglev.lookup(content_hash).unwrap_or(content_id % 65537);

    CdnRoutingMetrics {
        content_hash,
        node_count,
        avg_latency_ms,
        cache_hit_rate_permille,
        selected_node_id,
    }
}

// ── Bridge 2: CDN → DB (node health records for persistence) ─────────────

/// Node health record for ALICE-DB persistence.
///
/// Written after each health probe so the database layer can maintain
/// time-series node state, detect failures, and trigger rebalancing.
pub struct CdnDbNodeHealth {
    /// FNV-1a hash over `(node_id, rtt_ms)` bytes — row deduplication key.
    pub content_hash: u64,
    /// CDN node identifier.
    pub node_id: NodeId,
    /// Observed RTT to this node in milliseconds (Vivaldi prediction).
    pub rtt_ms: u64,
    /// True when the node responded within the acceptable RTT threshold.
    pub is_healthy: bool,
    /// Maglev slot count for this node (load distribution indicator).
    pub slot_count: u64,
}

/// Build a node health record for ALICE-DB persistence.
///
/// `threshold_ms` defines the RTT budget: nodes within budget are healthy.
/// `is_healthy` is derived branchlessly via integer comparison (no if/else
/// in the critical path).  `slot_count` is extracted from `MaglevHash`
/// distribution stats so the DB can monitor load imbalance over time.
#[inline]
pub fn cdn_to_db_node_health(
    node_id: NodeId,
    rtt_ms: u64,
    threshold_ms: u64,
    maglev: &MaglevHash,
) -> CdnDbNodeHealth {
    // Branchless health flag: rtt <= threshold maps to true via integer compare.
    // The compiler emits a cmp + setle; no branch misprediction risk.
    let is_healthy = rtt_ms <= threshold_ms;

    // Slot count from distribution stats: sum of all node slot counts.
    // `node_counts` is a Vec<usize> indexed by Maglev table position; we
    // report the total as a load-distribution health indicator rather than
    // a per-node breakdown (the table does not carry NodeId → slot mapping).
    let dist = maglev.distribution_stats();
    let slot_count: u64 = dist.node_counts.iter().map(|&c| c as u64).sum();

    // Content hash over the (node_id, rtt_ms) pair for row-level dedup.
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&node_id.to_le_bytes());
    key[8..16].copy_from_slice(&rtt_ms.to_le_bytes());
    let content_hash = fnv1a(&key);

    CdnDbNodeHealth {
        content_hash,
        node_id,
        rtt_ms,
        is_healthy,
        slot_count,
    }
}

// ── Bridge 3: CDN → Auth (content access control metadata) ───────────────

/// Content access control metadata for ALICE-Auth.
///
/// Carries the serving-node assignment and predicted RTT so the Auth layer
/// can make access-control decisions that are latency-aware (e.g., rejecting
/// requests routed to degraded nodes, enforcing per-node quotas).
pub struct CdnAuthAccessMeta {
    /// FNV-1a hash of `content_id` — Auth session key.
    pub content_hash: u64,
    /// CDN node assigned to serve this content request.
    pub assigned_node_id: NodeId,
    /// Vivaldi-predicted RTT to the assigned node in milliseconds.
    pub predicted_rtt_ms: i64,
    /// True when predicted RTT is within the Auth-enforced latency budget.
    pub within_latency_budget: bool,
    /// Number of Vivaldi candidate nodes evaluated for this request.
    pub candidates_evaluated: usize,
}

/// Build content access control metadata for ALICE-Auth.
///
/// `local` is the CDN edge-node coordinate.  `nodes` is the slice of
/// candidate `(NodeId, VivaldiCoord)` pairs.  The function selects the
/// nearest node by Vivaldi RTT (pure distance, hash_weight = 0.0) and
/// compares the result against `budget_ms`.  No float in the hot path —
/// only the Fixed → ms conversion is i64 arithmetic.
#[inline]
pub fn cdn_to_auth_access_meta(
    content_id: u64,
    local: &VivaldiCoord,
    nodes: &[(NodeId, VivaldiCoord)],
    budget_ms: u64,
) -> CdnAuthAccessMeta {
    let content_hash = fnv1a(&content_id.to_le_bytes());
    let candidates_evaluated = nodes.len();

    // Pure-distance locator: find the nearest node by Vivaldi RTT.
    let locator = alice_cdn::ContentLocator::with_weights(*local, 0.0, 1.0);
    let refs: Vec<(NodeId, &VivaldiCoord)> = nodes.iter().map(|(id, c)| (*id, c)).collect();

    let (assigned_node_id, rtt_fixed) = locator
        .find_closest(refs)
        .unwrap_or((0, alice_cdn::vivaldi::Fixed::ZERO));

    let predicted_rtt_ms = rtt_fixed.to_ms();

    // Branchless budget check: compare integer ms value.
    // Compiler emits cmp + setle — no branch.
    let within_latency_budget = predicted_rtt_ms >= 0
        && (predicted_rtt_ms as u64) <= budget_ms;

    CdnAuthAccessMeta {
        content_hash,
        assigned_node_id,
        predicted_rtt_ms,
        within_latency_budget,
        candidates_evaluated,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_cdn::VivaldiCoord;

    // ── Bridge 1: CDN → Analytics ────────────────────────────────────────

    #[test]
    fn test_cdn_to_routing_metrics() {
        let nodes: Vec<NodeId> = (1..=6).collect();
        let maglev = MaglevHash::new(nodes);

        // 6 RTT samples simulating a small routing batch.
        let rtt_samples: &[u64] = &[10, 20, 30, 40, 50, 60];
        // 75 hits out of 100 total requests.
        let metrics = cdn_to_routing_metrics(0xABCD_EF01_2345_6789, rtt_samples, 75, 100, &maglev);

        // Content hash must be non-zero and deterministic.
        assert_ne!(metrics.content_hash, 0);
        assert_eq!(
            metrics.content_hash,
            crate::hash::fnv1a(&0xABCD_EF01_2345_6789u64.to_le_bytes())
        );

        // Node count matches the sample slice length.
        assert_eq!(metrics.node_count, 6);

        // Average RTT: (10+20+30+40+50+60)/6 = 210/6 = 35.
        assert_eq!(metrics.avg_latency_ms, 35);

        // Hit rate: 75*1000/100 = 750 permille.
        assert_eq!(metrics.cache_hit_rate_permille, 750);

        // Selected node must be from the registered range (1..=6).
        assert!(
            metrics.selected_node_id >= 1 && metrics.selected_node_id <= 6,
            "selected_node_id {} out of range", metrics.selected_node_id
        );

        // Edge case: empty RTT samples must not panic.
        let empty = cdn_to_routing_metrics(1, &[], 0, 0, &maglev);
        assert_eq!(empty.node_count, 0);
        assert_eq!(empty.avg_latency_ms, 0);
        assert_eq!(empty.cache_hit_rate_permille, 0);
    }

    // ── Bridge 2: CDN → DB ───────────────────────────────────────────────

    #[test]
    fn test_cdn_to_db_node_health() {
        let nodes: Vec<NodeId> = (1..=4).collect();
        let maglev = MaglevHash::new(nodes);

        // Node with RTT well within the 100 ms threshold.
        let healthy = cdn_to_db_node_health(1, 20, 100, &maglev);
        assert!(healthy.is_healthy);
        assert_eq!(healthy.node_id, 1);
        assert_eq!(healthy.rtt_ms, 20);
        assert_ne!(healthy.content_hash, 0);

        // Node with RTT exceeding threshold — must be marked unhealthy.
        let sick = cdn_to_db_node_health(2, 200, 100, &maglev);
        assert!(!sick.is_healthy);
        assert_eq!(sick.rtt_ms, 200);

        // Content hashes must differ for different (node_id, rtt_ms) pairs.
        assert_ne!(healthy.content_hash, sick.content_hash);

        // Boundary: RTT exactly equal to threshold is healthy.
        let boundary = cdn_to_db_node_health(3, 100, 100, &maglev);
        assert!(boundary.is_healthy);
    }

    // ── Bridge 3: CDN → Auth ─────────────────────────────────────────────

    #[test]
    fn test_cdn_to_auth_access_meta() {
        // Two candidate nodes; the closer one is at (2, 0).
        let nodes: Vec<(NodeId, VivaldiCoord)> = vec![
            (10, VivaldiCoord::at(100.0, 0.0, 0.0, 5.0)), // far
            (20, VivaldiCoord::at(2.0,   0.0, 0.0, 1.0)), // near
        ];
        let local = VivaldiCoord::at(0.0, 0.0, 0.0, 1.0);

        // Budget of 50 ms — the near node at ~4 ms should pass.
        let meta = cdn_to_auth_access_meta(0xFEED_F00D, &local, &nodes, 50);

        assert_ne!(meta.content_hash, 0);
        assert_eq!(meta.candidates_evaluated, 2);

        // Nearest node (20) selected by Vivaldi pure-distance.
        assert_eq!(meta.assigned_node_id, 20,
            "expected node 20, got {}", meta.assigned_node_id);

        // Predicted RTT must be non-negative.
        assert!(meta.predicted_rtt_ms >= 0);

        // Within budget: near node RTT << 50 ms.
        assert!(meta.within_latency_budget,
            "expected within budget, rtt_ms={}", meta.predicted_rtt_ms);

        // Edge case: empty node list must not panic, falls back to node 0.
        let empty = cdn_to_auth_access_meta(1, &local, &[], 50);
        assert_eq!(empty.assigned_node_id, 0);
        assert_eq!(empty.candidates_evaluated, 0);

        // Tight budget of 0 ms — any positive RTT exceeds it.
        let over = cdn_to_auth_access_meta(0xFEED_F00D, &local, &nodes, 0);
        assert!(!over.within_latency_budget,
            "expected over budget, rtt_ms={}", over.predicted_rtt_ms);
    }
}
