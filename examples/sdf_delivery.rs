//! ALICE SDF Asset Delivery Pipeline Demo
//!
//! ```text
//! Client Request (asset_id)
//!     ↓
//! ALICE-CDN (Vivaldi → nearest edge node, Maglev O(1) assignment)
//!     ↓
//! ALICE-Cache (Markov prefetch, TinyLFU eviction)
//!     ↓ (cache miss → origin)
//! ALICE-SDF (ASDF binary format, ~38 bytes per sphere)
//! ```
//!
//! Run: `cargo run --example sdf_delivery`
//!
//! Optimizations:
//! - In-memory ASDF serialization (zero disk I/O)
//! - Fixed-array origin storage (zero HashMap alloc)
//! - Pre-built lookup tables (zero per-request allocation)
//! - Single-clone cache population
//!
//! Author: Moroya Sakamoto

use alice_cache::{AliceCache, CacheConfig};
use alice_cdn::{ContentLocator, MaglevHash, VivaldiCoord};
use alice_sdf::io::AsdfHeader;
use alice_sdf::{SdfNode, SdfTree};

// Estimated glTF sizes for bandwidth comparison
const GLTF_SPHERE_BYTES: usize = 15_000; // ~15 KB (vertices + normals + indices)
const GLTF_CSG_BYTES: usize = 200_000; // ~200 KB (boolean mesh result)
const GLTF_COMPLEX_BYTES: usize = 2_000_000; // ~2 MB (100+ node scene)
const GLTF_SIZES: [usize; 3] = [GLTF_SPHERE_BYTES, GLTF_CSG_BYTES, GLTF_COMPLEX_BYTES];

const NUM_ASSETS: usize = 3;
const ASSET_ID_BASE: u64 = 1001;
const ASSET_NAMES: [&str; NUM_ASSETS] = ["Sphere", "CSG", "Complex"];

const NUM_CDN_NODES: usize = 8;
const SIMULATION_REQUESTS: u64 = 100;

/// CDN edge node definition
struct CdnNode {
    id: u64,
    name: &'static str,
    coord: VivaldiCoord,
}

/// Serialize SdfTree to ASDF binary format in-memory (zero disk I/O)
///
/// Layout: [AsdfHeader: 16B] + [bincode body: variable]
/// CRC32 computed on-the-fly over body bytes.
#[inline]
fn serialize_asdf(tree: &SdfTree) -> Vec<u8> {
    let body = bincode::serialize(tree).expect("SdfTree serialization");
    let crc = crc32fast::hash(&body);
    let header = AsdfHeader::new(tree, crc);
    let mut buf = Vec::with_capacity(16 + body.len());
    buf.extend_from_slice(&header.to_bytes());
    buf.extend_from_slice(&body);
    buf
}

/// Asset ID → array index (branchless)
#[inline(always)]
fn asset_idx(id: u64) -> usize {
    (id - ASSET_ID_BASE) as usize
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       ALICE SDF ASSET DELIVERY PIPELINE DEMO                ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // ========================================================================
    // PHASE 1: CREATE SDF ASSETS & IN-MEMORY ASDF SERIALIZATION
    // ========================================================================
    println!("━━━ PHASE 1: SDF Asset Creation & ASDF Serialization ━━━");
    println!("  (In-memory serialization — zero disk I/O)");
    println!();

    // Asset 1001: Simple sphere (1 SDF node)
    let sphere_tree = SdfTree::new(SdfNode::sphere(1.0));
    let sphere_nodes = sphere_tree.node_count();
    let sphere_asdf = serialize_asdf(&sphere_tree);

    // Asset 1002: CSG hollow sphere (sphere - box = 3 nodes)
    let csg_tree = SdfTree::new(SdfNode::sphere(1.5).subtract(SdfNode::box3d(1.0, 1.0, 1.0)));
    let csg_nodes = csg_tree.node_count();
    let csg_asdf = serialize_asdf(&csg_tree);

    // Asset 1003: Complex scene (10 CSG unions = 31 nodes)
    let mut complex_shape = SdfNode::sphere(1.0);
    for i in 0..10 {
        complex_shape =
            complex_shape.union(SdfNode::box3d(0.8, 0.8, 0.8).translate(i as f32 * 2.0, 0.0, 0.0));
    }
    let complex_tree = SdfTree::new(complex_shape);
    let complex_nodes = complex_tree.node_count();
    let complex_asdf = serialize_asdf(&complex_tree);

    // Fixed-array asset storage (zero HashMap overhead)
    let assets: [&[u8]; NUM_ASSETS] = [&sphere_asdf, &csg_asdf, &complex_asdf];
    let asset_sizes: [usize; NUM_ASSETS] = [sphere_asdf.len(), csg_asdf.len(), complex_asdf.len()];
    let asset_nodes: [u32; NUM_ASSETS] = [sphere_nodes, csg_nodes, complex_nodes];

    println!("  ┌──────────────────┬────────┬──────────┬──────────┬──────────┐");
    println!("  │ Asset            │ Nodes  │ ASDF     │ glTF est │ Ratio    │");
    println!("  ├──────────────────┼────────┼──────────┼──────────┼──────────┤");
    for i in 0..NUM_ASSETS {
        println!(
            "  │ {:>16} │ {:>5}  │ {:>5} B  │ {:>5} KB │ {:>5.0}x   │",
            ASSET_NAMES[i],
            asset_nodes[i],
            asset_sizes[i],
            GLTF_SIZES[i] / 1024,
            GLTF_SIZES[i] as f64 / asset_sizes[i] as f64,
        );
    }
    println!("  └──────────────────┴────────┴──────────┴──────────┴──────────┘");
    println!();

    // ========================================================================
    // PHASE 2: SETUP CDN EDGE NODES (Vivaldi + Maglev)
    // ========================================================================
    println!(
        "━━━ PHASE 2: CDN Edge Node Setup ({} Global Nodes) ━━━",
        NUM_CDN_NODES
    );

    let nodes: [CdnNode; NUM_CDN_NODES] = [
        CdnNode {
            id: 1,
            name: "Tokyo",
            coord: VivaldiCoord::at(0.0, 0.0, 0.0, 2.0),
        },
        CdnNode {
            id: 2,
            name: "London",
            coord: VivaldiCoord::at(30.0, 20.0, 0.0, 3.0),
        },
        CdnNode {
            id: 3,
            name: "New York",
            coord: VivaldiCoord::at(50.0, 10.0, 0.0, 4.0),
        },
        CdnNode {
            id: 4,
            name: "Sydney",
            coord: VivaldiCoord::at(-20.0, -30.0, 0.0, 5.0),
        },
        CdnNode {
            id: 5,
            name: "Singapore",
            coord: VivaldiCoord::at(5.0, -15.0, 0.0, 2.0),
        },
        CdnNode {
            id: 6,
            name: "Frankfurt",
            coord: VivaldiCoord::at(28.0, 18.0, 0.0, 3.0),
        },
        CdnNode {
            id: 7,
            name: "São Paulo",
            coord: VivaldiCoord::at(45.0, -25.0, 0.0, 6.0),
        },
        CdnNode {
            id: 8,
            name: "Mumbai",
            coord: VivaldiCoord::at(15.0, -10.0, 0.0, 3.0),
        },
    ];

    // Pre-built node name lookup: id → name (O(1) array index, no iter().find())
    let node_names: [&str; NUM_CDN_NODES + 1] = [
        "?", // id=0 (unused)
        "Tokyo",
        "London",
        "New York",
        "Sydney",
        "Singapore",
        "Frankfurt",
        "São Paulo",
        "Mumbai",
    ];

    // Maglev: O(1) content → node assignment
    let node_ids: Vec<u64> = nodes.iter().map(|n| n.id).collect();
    let maglev = MaglevHash::new(node_ids);

    // Client located near Tokyo
    let client_coord = VivaldiCoord::at(1.0, 1.0, 0.0, 1.0);
    let locator = ContentLocator::with_weights(client_coord, 0.3, 0.7);

    // Pre-built node reference list (reused across all routing calls)
    let node_refs: Vec<(u64, &VivaldiCoord)> = nodes.iter().map(|n| (n.id, &n.coord)).collect();

    println!("  Maglev hash table: {} nodes, O(1) lookup", NUM_CDN_NODES);
    println!("  Client location:   Near Tokyo (1.0, 1.0, 0.0)");
    println!();

    // RTT predictions from client to all nodes
    println!("  Vivaldi RTT Predictions (Client → Edge):");
    println!("  ┌─────────────┬──────────┐");
    println!("  │ Node        │ RTT (ms) │");
    println!("  ├─────────────┼──────────┤");
    for node in &nodes {
        println!(
            "  │ {:>11} │ {:>7.1}  │",
            node.name,
            client_coord.predict_rtt(&node.coord).to_f64(),
        );
    }
    println!("  └─────────────┴──────────┘");
    println!();

    // ========================================================================
    // PHASE 3: CLIENT REQUEST ROUTING
    // ========================================================================
    println!("━━━ PHASE 3: Content Request Routing ━━━");

    let asset_ids: [u64; NUM_ASSETS] = [1001, 1002, 1003];

    println!("  ┌──────────┬───────────┬────────────────┬──────────┐");
    println!("  │ Asset    │ Maglev    │ Vivaldi Best   │ RTT (ms) │");
    println!("  ├──────────┼───────────┼────────────────┼──────────┤");

    for (i, &asset_id) in asset_ids.iter().enumerate() {
        // Maglev O(1) primary assignment
        let primary_id = maglev.lookup(asset_id).unwrap();

        // Vivaldi latency-aware best node (reuses pre-built node_refs)
        let best = locator.find_best(asset_id, node_refs.clone()).unwrap();

        println!(
            "  │ {:>8} │ {:>9} │ {:>14} │ {:>7.1}  │",
            ASSET_NAMES[i],
            node_names[primary_id as usize],
            node_names[best.id as usize],
            best.predicted_rtt.to_f64(),
        );
    }
    println!("  └──────────┴───────────┴────────────────┴──────────┘");
    println!();

    // ========================================================================
    // PHASE 4: CACHE LAYER (Markov Prefetch + TinyLFU)
    // ========================================================================
    println!("━━━ PHASE 4: Cache Layer (Markov Prefetch) ━━━");

    let cache = AliceCache::<u64, Vec<u8>>::with_config(CacheConfig {
        capacity: 1000,
        num_nodes: NUM_CDN_NODES as i32,
        node_id: 1, // Tokyo edge node
        enable_oracle: true,
        ..Default::default()
    });

    // Cold requests
    println!(
        "  Cold start ({} requests → {} misses):",
        NUM_ASSETS, NUM_ASSETS
    );
    for (i, &id) in asset_ids.iter().enumerate() {
        if cache.get(&id).is_some() {
            println!("    Asset {} ({}): HIT", id, ASSET_NAMES[i]);
        } else {
            println!(
                "    Asset {} ({}): MISS → fetch from origin",
                id, ASSET_NAMES[i]
            );
            cache.put(id, assets[i].to_vec()); // single allocation
        }
    }
    println!();

    // Warm requests (should all hit)
    println!(
        "  Warm requests ({} requests → should be cached):",
        NUM_ASSETS
    );
    for (i, &id) in asset_ids.iter().enumerate() {
        if cache.get(&id).is_some() {
            println!("    Asset {} ({}): HIT", id, ASSET_NAMES[i]);
        } else {
            println!("    Asset {} ({}): MISS", id, ASSET_NAMES[i]);
            cache.put(id, assets[i].to_vec());
        }
    }
    println!();

    println!(
        "  Cache hit rate: {:.1}%  (hits: {}, misses: {})",
        cache.hit_rate() * 100.0,
        cache
            .stats()
            .hits
            .load(std::sync::atomic::Ordering::Relaxed),
        cache
            .stats()
            .misses
            .load(std::sync::atomic::Ordering::Relaxed),
    );
    println!();

    // Markov prediction
    println!("  Markov Prefetch Predictions:");
    println!(
        "    1001 → 1002: {}",
        if cache.should_prefetch(&1001, &1002) {
            "PREFETCH"
        } else {
            "no signal"
        },
    );
    println!(
        "    1002 → 1003: {}",
        if cache.should_prefetch(&1002, &1003) {
            "PREFETCH"
        } else {
            "no signal"
        },
    );
    println!();

    // ========================================================================
    // PHASE 5: FULL PIPELINE SIMULATION
    // ========================================================================
    println!(
        "━━━ PHASE 5: Full Pipeline Simulation ({} Requests) ━━━",
        SIMULATION_REQUESTS
    );

    cache.stats().reset();

    let mut total_asdf_bytes: u64 = 0;
    let mut total_gltf_equiv: u64 = 0;

    for request in 0..SIMULATION_REQUESTS {
        let idx = (request % NUM_ASSETS as u64) as usize;
        let asset_id = asset_ids[idx];

        // CDN routing (O(1) Maglev)
        let _assigned_node = maglev.lookup(asset_id);

        // Cache lookup — only allocate on miss
        let len = if cache.get(&asset_id).is_some() {
            asset_sizes[idx]
        } else {
            let size = assets[idx].len();
            cache.put(asset_id, assets[idx].to_vec());
            size
        };

        total_asdf_bytes += len as u64;
        total_gltf_equiv += GLTF_SIZES[idx] as u64;
    }

    let hit_rate = cache.hit_rate();
    let bandwidth_ratio = total_gltf_equiv as f64 / total_asdf_bytes as f64;

    println!("  Requests:        {}", SIMULATION_REQUESTS);
    println!("  Cache hit rate:  {:.1}%", hit_rate * 100.0);
    println!(
        "  ASDF transferred: {} bytes ({:.1} KB)",
        total_asdf_bytes,
        total_asdf_bytes as f64 / 1024.0,
    );
    println!(
        "  glTF equivalent:  {} bytes ({:.1} MB)",
        total_gltf_equiv,
        total_gltf_equiv as f64 / 1_048_576.0,
    );
    println!("  Bandwidth ratio:  {:.0}x reduction", bandwidth_ratio);
    println!();

    // ========================================================================
    // PHASE 6: SUMMARY
    // ========================================================================
    let client_rtt = client_coord.predict_rtt(&nodes[0].coord).to_f64();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         SDF ASSET DELIVERY PIPELINE SUMMARY                 ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║                                                              ║");
    println!("║  [Client]                                                    ║");
    println!("║     │ Request asset_id                                       ║");
    println!("║     ▼                                                        ║");
    println!("║  [ALICE-CDN] Vivaldi Routing + Maglev O(1)                   ║");
    println!(
        "║     │ {} edge nodes, RTT-optimized                            ║",
        NUM_CDN_NODES
    );
    println!(
        "║     │ Nearest: Tokyo ({:.1}ms)                               ║",
        client_rtt
    );
    println!("║     ▼                                                        ║");
    println!("║  [ALICE-Cache] 256-shard + Markov Prefetch                   ║");
    println!(
        "║     │ Hit rate: {:.1}%, TinyLFU eviction                     ║",
        hit_rate * 100.0
    );
    println!("║     ▼                                                        ║");
    println!("║  [ALICE-SDF] ASDF Binary Format (in-memory serialization)    ║");
    println!(
        "║     │ Sphere:  {} B vs glTF {} KB ({:.0}x)                 ║",
        asset_sizes[0],
        GLTF_SIZES[0] / 1024,
        GLTF_SIZES[0] as f64 / asset_sizes[0] as f64
    );
    println!(
        "║     │ CSG:     {} B vs glTF {} KB ({:.0}x)                ║",
        asset_sizes[1],
        GLTF_SIZES[1] / 1024,
        GLTF_SIZES[1] as f64 / asset_sizes[1] as f64
    );
    println!(
        "║     │ Complex: {} B vs glTF {} KB ({:.0}x)               ║",
        asset_sizes[2],
        GLTF_SIZES[2] / 1024,
        GLTF_SIZES[2] as f64 / asset_sizes[2] as f64
    );
    println!("║                                                              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  BANDWIDTH SAVINGS:                                          ║");
    println!(
        "║    {} req: {:.1} KB (ASDF) vs {:.1} MB (glTF) = {:.0}x     ║",
        SIMULATION_REQUESTS,
        total_asdf_bytes as f64 / 1024.0,
        total_gltf_equiv as f64 / 1_048_576.0,
        bandwidth_ratio
    );
    println!("║                                                              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  OPTIMIZATIONS:                                              ║");
    println!("║    ✓ In-memory ASDF serialization (zero disk I/O)            ║");
    println!("║    ✓ Fixed-array origin (zero HashMap alloc)                 ║");
    println!("║    ✓ Pre-built lookup tables (zero per-request alloc)        ║");
    println!("║    ✓ Single-clone cache population                           ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  COMPONENTS:                                                 ║");
    println!(
        "║    ALICE-SDF   v{}  ALICE-CDN  v{}  ALICE-Cache v{} ║",
        alice_sdf::VERSION,
        alice_cdn::VERSION,
        alice_cache::VERSION
    );
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("✓ SDF Asset Delivery Pipeline: SUCCESS");

    Ok(())
}
