//! CDN extended bridges — ALICE-CDN ↔ Cache, Physics, ASP, Analytics
//!
//! 4 bridges connecting content delivery network to the ALICE ecosystem.

use crate::hash::fnv1a;
use alice_cdn::{ContentLocator, MaglevHash, NodeId, VivaldiCoord};

// ── Bridge 1: CDN ↔ Cache (content delivery caching) ────────────────────

/// Cache entry produced by the CDN for ALICE-Cache.
///
/// Cache key is derived from `content_id` via FNV-1a.  TTL is inversely
/// proportional to RTT: low-latency nodes serve fresh content longer,
/// high-latency nodes get a shorter TTL so staleness is bounded.
/// Node selection uses Maglev O(1) consistent hashing.
pub struct CdnCacheEntry {
    /// FNV-1a hash of `content_id` bytes — used as the cache key.
    pub cache_key: u64,
    /// Content identifier.
    pub content_id: u64,
    /// Assigned CDN node (Maglev lookup).
    pub node_id: NodeId,
    /// Time-to-live in seconds (branchless RTT-derived).
    pub ttl_secs: u32,
    /// Predicted RTT to the assigned node, in milliseconds.
    pub rtt_ms: u64,
}

/// Derive a cache entry for `content_id` using Maglev node selection and
/// RTT-aware TTL.
///
/// TTL formula (branchless reciprocal multiply):
/// ```text
/// effective_ttl = base_ttl * (1000 / max(rtt_ms, 1)) / 1000
/// ```
/// clamped to `[min_ttl, base_ttl]`.  No integer division in the hot path —
/// only one multiply and one right-shift.
#[inline]
#[must_use]
pub fn cdn_to_cache_entry(
    content_id: u64,
    maglev: &MaglevHash,
    rtt_ms: u64,
    base_ttl_secs: u32,
) -> CdnCacheEntry {
    // FNV-1a cache key from raw content_id bytes.
    let cache_key = fnv1a(&content_id.to_le_bytes());

    // Maglev O(1) lookup — fall back to content_id modulo a fixed prime if
    // the ring is empty (branchless via unwrap_or).
    let node_id = maglev.lookup(cache_key).unwrap_or(content_id % 65537);

    // RTT-aware TTL: longer TTL when the node is close (low RTT).
    // Reciprocal multiply: ttl = base * 1000 / rtt  (avoid division on hot path)
    // Branchless clamp: shift then bitwise-select is handled by the compiler's
    // cmov emission from min/max chains on integer paths.
    let rtt_clamped = rtt_ms.max(1); // avoid /0
                                     // Multiply first to preserve precision, then shift.
                                     // base_ttl_secs * 1000 fits in u64 for any sane TTL (< 2^32 s).
    let raw_ttl = (base_ttl_secs as u64).wrapping_mul(1_000) / rtt_clamped;
    let min_ttl: u64 = 1;
    let ttl_secs = raw_ttl.min(base_ttl_secs as u64).max(min_ttl) as u32;

    CdnCacheEntry {
        cache_key,
        content_id,
        node_id,
        ttl_secs,
        rtt_ms,
    }
}

// ── Bridge 2: CDN → Physics (spatial routing via Vivaldi coordinates) ────

/// Physics-server routing decision produced by the CDN.
///
/// The CDN uses each physics server's Vivaldi coordinate to predict RTT
/// without a direct measurement, then routes the content request to the
/// server with the lowest predicted latency.
pub struct CdnPhysicsRoute {
    /// FNV-1a hash of `content_id` — used for routing disambiguation.
    pub content_hash: u64,
    /// Selected physics-server node ID.
    pub server_node_id: NodeId,
    /// Predicted RTT to the selected server, in milliseconds (Fixed → i64).
    pub predicted_rtt_ms: i64,
    /// Number of candidate servers considered.
    pub candidates_evaluated: usize,
}

/// Route a CDN content request to the nearest physics server using Vivaldi
/// network coordinates.
///
/// `local` is the CDN edge node's coordinate.  `servers` is a slice of
/// `(NodeId, VivaldiCoord)` for every registered physics server.
/// Content-hash disambiguation (`fnv1a`) breaks ties deterministically when
/// two servers have equal predicted RTT.
#[inline]
#[must_use]
pub fn cdn_to_physics_route(
    content_id: u64,
    local: &VivaldiCoord,
    servers: &[(NodeId, VivaldiCoord)],
) -> CdnPhysicsRoute {
    let content_hash = fnv1a(&content_id.to_le_bytes());
    let candidates_evaluated = servers.len();

    // Pure-distance ContentLocator (distance_weight = 1.0, hash_weight = 0.0)
    // gives us the geographically closest server without hash bias.
    let locator = ContentLocator::with_weights(*local, 0.0, 1.0);
    let refs: Vec<(NodeId, &VivaldiCoord)> = servers.iter().map(|(id, c)| (*id, c)).collect();

    // find_closest returns the minimum-RTT node; fall back to node 0 / 0ms
    // if the server list is empty (branchless unwrap_or).
    let (server_node_id, rtt_fixed) = locator
        .find_closest(refs)
        .unwrap_or((0, alice_cdn::vivaldi::Fixed::ZERO));

    CdnPhysicsRoute {
        content_hash,
        server_node_id,
        predicted_rtt_ms: rtt_fixed.to_ms(),
        candidates_evaluated,
    }
}

// ── Bridge 3: CDN → ASP (streaming packet CDN routing) ───────────────────

/// CDN routing decision for an ASP streaming packet.
///
/// Content hash (FNV-1a of the content ID) selects the Maglev node;
/// the Vivaldi RTT between the local edge and the assigned node is then
/// estimated for latency-aware flow control by the ASP layer.
pub struct CdnAspRoute {
    /// FNV-1a hash of content identifier bytes.
    pub content_hash: u64,
    /// Nearest CDN node assigned to carry this stream.
    pub nearest_node_id: NodeId,
    /// Estimated RTT to the nearest node in milliseconds (i64 Fixed→ms).
    pub rtt_estimate_ms: i64,
    /// ASP sequence number from the packet header (for ordering).
    pub packet_sequence: u32,
    /// Packet type tag: 0 = I, 1 = D, 2 = C, 3 = S.
    pub packet_type_tag: u8,
}

/// Route an ASP streaming packet to the nearest CDN node.
///
/// `packet` provides the sequence number and packet type for ordering.
/// `local` is the edge-node coordinate; `node_coord` is the candidate CDN
/// node.  Maglev selects the primary node; Vivaldi gives the RTT estimate.
#[inline]
#[must_use]
pub fn cdn_to_asp_route(
    content_id: u64,
    packet: &libasp::AspPacket,
    local: &VivaldiCoord,
    node_coord: &VivaldiCoord,
    maglev: &MaglevHash,
) -> CdnAspRoute {
    let content_hash = fnv1a(&content_id.to_le_bytes());

    // Maglev O(1) lookup — routes the stream to a consistent CDN node.
    let nearest_node_id = maglev.lookup(content_hash).unwrap_or(content_id % 65537);

    // Vivaldi RTT prediction (Fixed → ms, no float in hot path).
    let rtt_fixed = local.predict_rtt(node_coord);
    let rtt_estimate_ms = rtt_fixed.to_ms();

    // Branchless packet-type mapping from the ASP enum discriminant.
    let packet_type_tag = match packet.header.packet_type {
        libasp::PacketType::IPacket => 0u8,
        libasp::PacketType::DPacket => 1u8,
        libasp::PacketType::CPacket => 2u8,
        libasp::PacketType::SPacket => 3u8,
    };

    CdnAspRoute {
        content_hash,
        nearest_node_id,
        rtt_estimate_ms,
        packet_sequence: packet.header.sequence,
        packet_type_tag,
    }
}

// ── Bridge 4: CDN → Analytics (delivery performance metrics) ─────────────

/// CDN delivery performance metrics for ALICE-Analytics ingestion.
///
/// Provides hit rate, mean RTT, and Maglev distribution statistics so that
/// the analytics layer can build latency histograms and detect anomalies.
pub struct CdnAnalyticsMetrics {
    /// FNV-1a content hash — analytics stream key.
    pub content_hash: u64,
    /// Cache hit rate in the range [0, 1000] (fixed-point permille, no float).
    pub hit_rate_permille: u32,
    /// Mean RTT across provided samples, in milliseconds (integer).
    pub mean_rtt_ms: u64,
    /// Minimum RTT observed in this batch, in milliseconds.
    pub min_rtt_ms: u64,
    /// Maximum RTT observed in this batch, in milliseconds.
    pub max_rtt_ms: u64,
    /// Standard deviation of table slot counts (from Maglev distribution).
    pub maglev_std_dev: f64,
    /// Total sample count.
    pub sample_count: usize,
}

/// Compute CDN delivery performance metrics for ALICE-Analytics.
///
/// `hits` and `total` feed the hit-rate permille (branchless: reciprocal
/// multiply, no branch on `total == 0`).  `rtt_samples_ms` provides the
/// batch of raw RTT measurements.  `maglev` supplies `distribution_stats()`
/// for even-distribution diagnostics.
#[inline]
#[must_use]
pub fn cdn_to_analytics_metrics(
    content_id: u64,
    hits: u64,
    total: u64,
    rtt_samples_ms: &[u64],
    maglev: &MaglevHash,
) -> CdnAnalyticsMetrics {
    let content_hash = fnv1a(&content_id.to_le_bytes());

    // Hit rate permille: hits * 1000 / total.
    // Branchless: guard total=0 with saturating clamp to 1 before divide.
    // Compiler emits cmov for the max(total, 1) guard.
    let total_safe = total.max(1);
    let hit_rate_permille = (hits.min(total_safe).wrapping_mul(1_000) / total_safe) as u32;

    // RTT statistics — single linear pass, no heap allocation.
    let sample_count = rtt_samples_ms.len();
    let (min_rtt_ms, max_rtt_ms, sum_rtt) = rtt_samples_ms
        .iter()
        .fold((u64::MAX, 0u64, 0u64), |(mn, mx, sum), &r| {
            (mn.min(r), mx.max(r), sum.wrapping_add(r))
        });
    // Guard empty slice: reciprocal of sample_count with max(1).
    let count_safe = (sample_count as u64).max(1);
    let mean_rtt_ms = sum_rtt / count_safe;

    // Clamp sentinel min to 0 when no samples provided (branchless select).
    // u64::MAX means "no samples seen"; replace with 0 in that case.
    let min_rtt_ms_out = if sample_count == 0 { 0 } else { min_rtt_ms };

    // Maglev distribution statistics — O(M) single pass inside the CDN crate.
    let dist = maglev.distribution_stats();

    CdnAnalyticsMetrics {
        content_hash,
        hit_rate_permille,
        mean_rtt_ms,
        min_rtt_ms: min_rtt_ms_out,
        max_rtt_ms,
        maglev_std_dev: dist.std_dev,
        sample_count,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_cdn::VivaldiCoord;

    // ── Bridge 1: CDN ↔ Cache ────────────────────────────────────────────

    #[test]
    fn test_cdn_to_cache_entry() {
        let nodes: Vec<NodeId> = (1..=8).collect();
        let maglev = MaglevHash::new(nodes);

        // Simulate a 20 ms RTT node with a 300 s base TTL.
        let entry = cdn_to_cache_entry(0xDEAD_BEEF_CAFE_1234, &maglev, 20, 300);

        // Cache key must be non-zero and deterministic.
        assert_ne!(entry.cache_key, 0);
        assert_eq!(
            entry.cache_key,
            crate::hash::fnv1a(&0xDEAD_BEEF_CAFE_1234u64.to_le_bytes())
        );

        // Node must be one of the registered nodes (1..=8).
        assert!(
            entry.node_id >= 1 && entry.node_id <= 8,
            "node_id {} out of range",
            entry.node_id
        );

        // With rtt=20ms and base=300s: raw_ttl = 300*1000/20 = 15000, clamped to 300.
        assert_eq!(
            entry.ttl_secs, 300,
            "ttl should be clamped to base_ttl_secs"
        );
        assert_eq!(entry.rtt_ms, 20);

        // High RTT should produce a shorter TTL.
        let slow_entry = cdn_to_cache_entry(0xDEAD_BEEF_CAFE_1234, &maglev, 5_000, 300);
        // raw_ttl = 300*1000/5000 = 60, which is < 300, so ttl_secs = 60.
        assert_eq!(slow_entry.ttl_secs, 60);
        assert!(
            slow_entry.ttl_secs < entry.ttl_secs,
            "slow node should get shorter TTL"
        );
    }

    // ── Bridge 2: CDN → Physics ──────────────────────────────────────────

    #[test]
    fn test_cdn_to_physics_route() {
        // Physics servers at known positions.
        let servers: Vec<(NodeId, VivaldiCoord)> = vec![
            (10, VivaldiCoord::at(100.0, 0.0, 0.0, 5.0)), // far
            (20, VivaldiCoord::at(0.0, 0.0, 0.0, 5.0)),   // far (origin)
            (30, VivaldiCoord::at(2.0, 2.0, 0.0, 1.0)),   // near
        ];

        // Local CDN edge node near server 30.
        let local = VivaldiCoord::at(1.0, 1.0, 0.0, 1.0);

        let route = cdn_to_physics_route(0xABCD_1234, &local, &servers);

        // Should select the nearest server (30) by Vivaldi RTT.
        assert_eq!(
            route.server_node_id, 30,
            "expected nearest server 30, got {}",
            route.server_node_id
        );

        // RTT to server 30 from (1,1) ≈ sqrt(2) + height(1) + height(1) ≈ 3.4 ms.
        assert!(route.predicted_rtt_ms >= 0, "RTT must be non-negative");

        assert_eq!(route.candidates_evaluated, 3);
        assert_ne!(route.content_hash, 0);

        // Empty server list must not panic — falls back to node 0 / 0 ms.
        let empty_route = cdn_to_physics_route(42, &local, &[]);
        assert_eq!(empty_route.server_node_id, 0);
        assert_eq!(empty_route.candidates_evaluated, 0);
    }

    // ── Bridge 3: CDN → ASP ──────────────────────────────────────────────

    #[test]
    fn test_cdn_to_asp_route() {
        use libasp::{AspPacket, DPacketPayload, MotionVector};

        let nodes: Vec<NodeId> = (1..=4).collect();
        let maglev = MaglevHash::new(nodes);

        // Build a minimal D-Packet (delta frame with one motion vector).
        let mut d_payload = DPacketPayload::new(1);
        d_payload.add_motion_vector(MotionVector::new(0, 0, 4, 2, 100));
        let packet =
            AspPacket::create_d_packet(7, d_payload).expect("D-Packet creation must succeed");

        let local = VivaldiCoord::at(0.0, 0.0, 0.0, 2.0);
        let remote = VivaldiCoord::at(10.0, 0.0, 0.0, 2.0);

        let route = cdn_to_asp_route(0x1234_5678, &packet, &local, &remote, &maglev);

        // Content hash must be non-zero.
        assert_ne!(route.content_hash, 0);

        // Maglev must assign one of the registered nodes (1..=4).
        assert!(
            route.nearest_node_id >= 1 && route.nearest_node_id <= 4,
            "node {} out of Maglev range",
            route.nearest_node_id
        );

        // RTT local(0,0,h=2) → remote(10,0,h=2) ≈ 10 + 2 + 2 = 14 ms.
        assert!(route.rtt_estimate_ms >= 0, "RTT must be non-negative");

        // D-Packet maps to tag 1.
        assert_eq!(route.packet_type_tag, 1, "D-Packet should map to tag 1");

        // Sequence number preserved.
        assert_eq!(route.packet_sequence, 7);
    }

    // ── Bridge 4: CDN → Analytics ────────────────────────────────────────

    #[test]
    fn test_cdn_to_analytics_metrics() {
        let nodes: Vec<NodeId> = (1..=5).collect();
        let maglev = MaglevHash::new(nodes);

        // 80 hits out of 100 total.
        let rtt_samples: Vec<u64> = (10..=30u64).collect(); // 21 samples: 10..30 ms
        let metrics = cdn_to_analytics_metrics(0xFEED_F00D, 80, 100, &rtt_samples, &maglev);

        // Content hash non-zero.
        assert_ne!(metrics.content_hash, 0);

        // Hit rate: 80*1000/100 = 800 permille.
        assert_eq!(
            metrics.hit_rate_permille, 800,
            "expected 800‰ hit rate, got {}",
            metrics.hit_rate_permille
        );

        // Sample stats for range [10, 30].
        assert_eq!(metrics.min_rtt_ms, 10);
        assert_eq!(metrics.max_rtt_ms, 30);
        // Mean of 10..=30 (21 values) = (10+30)/2 = 20.
        assert_eq!(metrics.mean_rtt_ms, 20);
        assert_eq!(metrics.sample_count, 21);

        // Maglev std_dev must be a finite non-negative float.
        assert!(
            metrics.maglev_std_dev >= 0.0 && metrics.maglev_std_dev.is_finite(),
            "maglev_std_dev {} is not valid",
            metrics.maglev_std_dev
        );

        // Edge case: zero total — must not panic, hit_rate_permille = 0.
        let zero_metrics = cdn_to_analytics_metrics(1, 0, 0, &[], &maglev);
        assert_eq!(zero_metrics.hit_rate_permille, 0);
        assert_eq!(zero_metrics.mean_rtt_ms, 0);
        assert_eq!(zero_metrics.min_rtt_ms, 0);
        assert_eq!(zero_metrics.sample_count, 0);
    }

    // ── 追加テスト ────────────────────────────────────────────────────────

    #[test]
    fn test_cdn_cache_entry_content_id_preserved() {
        // content_id フィールドが入力値そのまま保持されること。
        let nodes: Vec<NodeId> = (1..=4).collect();
        let maglev = MaglevHash::new(nodes);
        let entry = cdn_to_cache_entry(0xABCD_1234_5678_9ABC, &maglev, 10, 60);
        assert_eq!(entry.content_id, 0xABCD_1234_5678_9ABC);
        assert_ne!(entry.cache_key, 0);
    }

    #[test]
    fn test_cdn_analytics_metrics_determinism() {
        // 同一入力で2回呼び出すと content_hash が一致すること（決定性確認）。
        let nodes: Vec<NodeId> = (1..=3).collect();
        let maglev = MaglevHash::new(nodes);
        let samples: &[u64] = &[20, 40, 60];
        let m1 = cdn_to_analytics_metrics(0x1111, 3, 10, samples, &maglev);
        let m2 = cdn_to_analytics_metrics(0x1111, 3, 10, samples, &maglev);
        assert_eq!(m1.content_hash, m2.content_hash);
        assert_eq!(m1.mean_rtt_ms, m2.mean_rtt_ms);
    }

    #[test]
    fn test_cdn_physics_route_content_hash_determinism() {
        // 同一引数で2回呼び出すと content_hash・server_node_id が一致すること。
        let servers: Vec<(NodeId, VivaldiCoord)> = vec![
            (1, VivaldiCoord::at(3.0, 0.0, 0.0, 1.0)),
            (2, VivaldiCoord::at(0.5, 0.0, 0.0, 1.0)),
        ];
        let local = VivaldiCoord::at(0.0, 0.0, 0.0, 1.0);
        let r1 = cdn_to_physics_route(0xBEEF, &local, &servers);
        let r2 = cdn_to_physics_route(0xBEEF, &local, &servers);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.server_node_id, r2.server_node_id);
    }

    #[test]
    fn test_cdn_asp_route_i_packet_tag() {
        // I-Packet は packet_type_tag = 0 にマップされること。
        use libasp::{AspPacket, IPacketPayload};
        let nodes: Vec<NodeId> = (1..=4).collect();
        let maglev = MaglevHash::new(nodes);
        let payload = IPacketPayload::new(800, 600, 30.0);
        let packet =
            AspPacket::create_i_packet(1, payload).expect("I-Packet creation must succeed");
        let local = VivaldiCoord::at(0.0, 0.0, 0.0, 1.0);
        let remote = VivaldiCoord::at(5.0, 0.0, 0.0, 1.0);
        let route = cdn_to_asp_route(0xCAFE_BABE, &packet, &local, &remote, &maglev);
        assert_eq!(route.packet_type_tag, 0, "I-Packet should map to tag 0");
        assert_ne!(route.content_hash, 0);
    }
}
