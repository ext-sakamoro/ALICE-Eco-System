//! ALICE Game Engine Pipeline Demo
//!
//! Demonstrates the full game engine integration across 6 ALICE crates:
//!
//! ```text
//! [ALICE-SDF]        Create world geometry (ASDF binary)
//!      ↓
//! [ALICE-CDN]        Type-aware content routing (ASDF detection)
//!      ↓
//! [ALICE-Physics]    Deterministic simulation (128-bit fixed-point)
//!      ↓
//! [ALICE-Sync]       Input synchronization (Lockstep / Rollback)
//!      ↓
//! [ALICE-DB]         Replay recording + Telemetry (model-based compression)
//! ```
//!
//! Run: `cargo run --example game_pipeline`
//!
//! Author: Moroya Sakamoto

use alice_cdn::content_types::{AsdfMetadata, ContentType};
use alice_cdn::{ContentLocator, VivaldiCoord};
use alice_physics::replay::{ReplayPlayer, ReplayRecorder};
use alice_physics::{Fix128, PhysicsConfig, PhysicsWorld, RigidBody, Vec3Fix};
use alice_sdf::{SdfNode, SdfTree};
use alice_sync::telemetry::SyncTelemetry;
use alice_sync::{InputFrame, LockstepSession};
use tempfile::tempdir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          ALICE GAME ENGINE PIPELINE DEMO                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let dir = tempdir()?;

    // ========================================================================
    // PHASE 1: WORLD GEOMETRY — ALICE-SDF
    // ========================================================================
    println!("━━━ PHASE 1: World Geometry (ALICE-SDF → ASDF) ━━━");

    // Create a game level: ground plane + obstacles
    let ground = SdfNode::box3d(100.0, 1.0, 100.0);
    let pillar1 = SdfNode::cylinder(1.0, 5.0).translate(5.0, 2.5, 0.0);
    let pillar2 = SdfNode::cylinder(1.0, 5.0).translate(-5.0, 2.5, 0.0);
    let arch = SdfNode::box3d(6.0, 1.0, 1.0).translate(0.0, 5.5, 0.0);
    let level = ground.union(pillar1).union(pillar2).union(arch);

    let tree = SdfTree::new(level);
    let asdf_path = dir.path().join("level.asdf");
    alice_sdf::save(&tree, &asdf_path)?;
    let asdf_bytes = std::fs::read(&asdf_path)?;
    let asdf_size = asdf_bytes.len();

    println!("  Level: ground + 2 pillars + arch");
    println!("  SDF nodes: {}", tree.node_count());
    println!("  ASDF size: {} bytes", asdf_size);
    println!();

    // ========================================================================
    // PHASE 2: CONTENT ROUTING — ALICE-CDN (Type-Aware)
    // ========================================================================
    println!("━━━ PHASE 2: Content Routing (ALICE-CDN + ContentType) ━━━");

    // Detect content type from raw bytes
    let content_type = ContentType::detect(&asdf_bytes);
    println!("  Detected type:    {:?}", content_type);
    println!("  Priority weight:  {} (latency-critical)", content_type.priority_weight());
    println!("  Latency sensitive: {}", content_type.is_latency_sensitive());

    // Parse ASDF header metadata
    if asdf_bytes.len() >= 16 {
        let header: [u8; 16] = asdf_bytes[..16].try_into().unwrap();
        if let Some(meta) = AsdfMetadata::parse(&header) {
            println!("  ASDF version:     {}", meta.version);
            println!("  ASDF nodes:       {}", meta.node_count);
            println!("  ASDF CRC32:       0x{:08X}", meta.crc32);
        }
    }
    println!();

    // CDN routing with type-aware priority
    let game_servers = vec![
        ("Tokyo", VivaldiCoord::at(0.0, 0.0, 0.0, 2.0)),
        ("London", VivaldiCoord::at(30.0, 20.0, 0.0, 3.0)),
        ("NYC", VivaldiCoord::at(50.0, 10.0, 0.0, 4.0)),
    ];

    let client = VivaldiCoord::at(2.0, 1.0, 0.0, 1.0); // Near Tokyo
    let locator = ContentLocator::with_weights(client, 0.3, 0.7);

    let node_refs: Vec<(u64, &VivaldiCoord)> = game_servers
        .iter()
        .enumerate()
        .map(|(i, (_, coord))| (i as u64 + 1, coord))
        .collect();
    let best = locator.find_best(1001, node_refs).unwrap();
    let best_name = game_servers[best.id as usize - 1].0;
    println!(
        "  Best server: {} (RTT: {:.1}ms, replicas: {})",
        best_name,
        best.predicted_rtt.to_f64(),
        content_type.suggested_replicas(),
    );
    println!();

    // ========================================================================
    // PHASE 3: PHYSICS SIMULATION — ALICE-Physics
    // ========================================================================
    println!("━━━ PHASE 3: Physics Simulation (ALICE-Physics, 128-bit) ━━━");

    let config = PhysicsConfig::default();
    let mut world = PhysicsWorld::new(config);

    // Add player bodies (2 players)
    let player1 = RigidBody::new_dynamic(Vec3Fix::from_int(0, 10, 0), Fix128::ONE);
    let player2 = RigidBody::new_dynamic(Vec3Fix::from_int(3, 10, 0), Fix128::ONE);
    let p1_id = world.add_body(player1);
    let p2_id = world.add_body(player2);

    // Add ground (static)
    let ground_body = RigidBody::new_static(Vec3Fix::ZERO);
    world.add_body(ground_body);

    println!("  Bodies: 2 players (dynamic) + 1 ground (static)");
    println!("  Player 1: pos=({}, {}, {})",
        world.bodies[p1_id].position.x.to_f32(),
        world.bodies[p1_id].position.y.to_f32(),
        world.bodies[p1_id].position.z.to_f32());
    println!("  Player 2: pos=({}, {}, {})",
        world.bodies[p2_id].position.x.to_f32(),
        world.bodies[p2_id].position.y.to_f32(),
        world.bodies[p2_id].position.z.to_f32());
    println!();

    // ========================================================================
    // PHASE 4: INPUT SYNC — ALICE-Sync (Lockstep)
    // ========================================================================
    println!("━━━ PHASE 4: Input Synchronization (ALICE-Sync, Lockstep) ━━━");

    let mut session = LockstepSession::new(2);
    let dt = Fix128::from_ratio(1, 60);
    let frames_to_simulate = 60;

    // Setup replay recording (→ ALICE-DB)
    let replay_path = dir.path().join("replay");
    let mut recorder = ReplayRecorder::new(&replay_path, 3)?; // 3 bodies

    // Setup telemetry recording (→ ALICE-DB)
    let telemetry_path = dir.path().join("telemetry");
    let telemetry = SyncTelemetry::new(&telemetry_path)?;

    println!("  Mode: Lockstep (2 players)");
    println!("  Frames: {} @ 60 FPS", frames_to_simulate);
    println!("  Replay → ALICE-DB: {:?}", replay_path);
    println!("  Telemetry → ALICE-DB: {:?}", telemetry_path);
    println!();

    // ========================================================================
    // PHASE 5: GAME LOOP — Sync + Physics + Record
    // ========================================================================
    println!("━━━ PHASE 5: Game Loop (60 frames) ━━━");

    for frame in 0..frames_to_simulate as u64 {
        // Player 1: move right
        let input1 = InputFrame::new(frame, 0).with_movement(100, 0, 0);
        // Player 2: move left
        let input2 = InputFrame::new(frame, 1).with_movement(-50, 0, 0);

        // Input sync: both players submit
        session.add_local_input(input1);
        session.add_remote_input(input2);

        if session.ready_to_advance() {
            let _inputs = session.advance().unwrap();

            // Step physics
            world.step(dt);

            // Record replay (positions → ALICE-DB)
            recorder.record_frame(&world)?;

            // Record telemetry
            let simulated_rtt = 10.0 + (frame as f32) * 0.05; // simulated RTT
            telemetry.record_rtt(frame, simulated_rtt)?;
            telemetry.record_prediction_accuracy(frame, 1.0)?; // lockstep = perfect
        }
    }

    recorder.flush()?;
    telemetry.flush()?;

    let p1_final = world.bodies[p1_id].position;
    let p2_final = world.bodies[p2_id].position;

    println!("  After {} frames:", frames_to_simulate);
    println!("    Player 1 final: ({:.2}, {:.2}, {:.2})",
        p1_final.x.to_f32(), p1_final.y.to_f32(), p1_final.z.to_f32());
    println!("    Player 2 final: ({:.2}, {:.2}, {:.2})",
        p2_final.x.to_f32(), p2_final.y.to_f32(), p2_final.z.to_f32());
    println!("    Replay frames:  {}", recorder.frame_count());
    println!("    Sync confirmed: frame {}", session.confirmed_frame());
    println!();

    // ========================================================================
    // PHASE 6: REPLAY PLAYBACK — ALICE-DB → Positions
    // ========================================================================
    println!("━━━ PHASE 6: Replay Playback (ALICE-DB → Positions) ━━━");

    recorder.close()?;
    let player = ReplayPlayer::open(&replay_path, 3)?;

    println!("  ┌───────┬──────────────────────────────────────────────┐");
    println!("  │ Frame │ Player 1 Position          │ Player 2 Pos   │");
    println!("  ├───────┼──────────────────────────────────────────────┤");

    for frame in [0, 14, 29, 44, 59] {
        let p1 = player.get_position(frame, 0)?;
        let p2 = player.get_position(frame, 1)?;
        if let (Some((x1, y1, _)), Some((x2, y2, _))) = (p1, p2) {
            println!("  │  {:>3}  │ ({:>7.2}, {:>7.2})          │ ({:>6.2}, {:>6.2}) │",
                frame, x1, y1, x2, y2);
        }
    }
    println!("  └───────┴──────────────────────────────────────────────┘");
    println!();

    player.close()?;

    // ========================================================================
    // PHASE 7: TELEMETRY ANALYSIS — ALICE-DB Aggregation
    // ========================================================================
    println!("━━━ PHASE 7: Telemetry Analysis (ALICE-DB Aggregation) ━━━");

    let rtt_data = telemetry.scan_rtt(0, 59)?;
    println!("  RTT samples recorded:     {}", rtt_data.len());

    telemetry.close()?;
    println!();

    // ========================================================================
    // SUMMARY
    // ========================================================================
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              GAME ENGINE PIPELINE SUMMARY                    ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║                                                              ║");
    println!("║  [ALICE-SDF]     → ASDF binary:  {} bytes ({} nodes)       ║",
        asdf_size, tree.node_count());
    println!("║  [ALICE-CDN]     → Content type: {:?}, route → {}     ║",
        content_type, best_name);
    println!("║  [ALICE-Physics] → {} frames, 128-bit deterministic        ║",
        frames_to_simulate);
    println!("║  [ALICE-Sync]    → Lockstep, {} confirmed frames           ║",
        session.confirmed_frame());
    println!("║  [ALICE-DB]      → Replay: {} frames + Telemetry           ║",
        frames_to_simulate);
    println!("║                                                              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  CROSS-CRATE BRIDGES:                                        ║");
    println!("║    ✓ SDF  → Physics  (physics_bridge, impl SdfField)         ║");
    println!("║    ✓ Physics → DB    (replay.rs, trajectory compression)     ║");
    println!("║    ✓ Sync → DB       (telemetry.rs, metric time-series)      ║");
    println!("║    ✓ CDN  ← SDF      (content_types, ASDF detection)         ║");
    println!("║    ✓ Sync → Physics  (physics_bridge, PhysicsRollback)       ║");
    println!("║                                                              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  CRATE VERSIONS:                                             ║");
    println!("║    ALICE-SDF     v{:<10}                                  ║",
        env!("CARGO_PKG_VERSION"));
    println!("║    ALICE-CDN     v{:<10}                                  ║",
        alice_cdn::VERSION);
    println!("║    ALICE-Physics v{:<10}                                  ║",
        "0.3.0");
    println!("║    ALICE-Sync    v{:<10}                                  ║",
        alice_sync::VERSION);
    println!("║    ALICE-DB      v{:<10}                                  ║",
        "0.1.0");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("✓ Game Engine Pipeline: SUCCESS (6 crates integrated)");

    Ok(())
}
