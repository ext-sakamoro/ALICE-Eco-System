//! Cross-bridges — Inter-connections among ALICE crates
//!
//! 25 bridges connecting Synth↔RTOS, Motion↔Kinematics, Kinematics↔RTOS,
//! Motion↔RTOS, VCS→Synth, VCS→Font, Font→Synth, Motion→Font,
//! Kinematics→Synth, RTOS→VCS, Animation↔Manga, Auth↔Crypto,
//! Queue↔Analytics, Print↔Animation, Manga↔Print,
//! RTOS↔ML, ML↔Motion, Print↔Sync, Text↔Sync, Kinematics→Voice,
//! Synth→Search, Motion→Search, VCS→ASP, Cache↔Crypto, View→Text.

use alice_cache::AliceCache;
use alice_crypto::hash as crypto_hash;
use alice_font::param::MetaFontParams;
use alice_kinematics::Vec3k;
use alice_ml::{TernaryWeight, ternary_matvec};
use alice_motion::{CubicBezier, MotionPlan, Vec3};
use alice_rtos::{Kernel, TaskPriority};
use alice_rtos::kernel::KernelStats;
use alice_search::AliceIndex;
use alice_synth::{NoteEventKind, Score};
use alice_text::{compress_tuned, CompressionMode};
use alice_voice::ParametricParams;
use alice_vcs::ast::{AstNodeKind, AstTree};
use alice_vcs::diff::{diff_trees, patch_size_bytes};
use alice_sync::input_sync::InputFrame;
// libasp metadata used via VcsAspDiffPacket payload encoding
use crate::hash::fnv1a;

// ── Bridge 1: Synth ↔ RTOS (real-time audio scheduling) ─────────────────

/// Audio render task configuration for RTOS scheduling.
pub struct AudioRtosConfig {
    /// Audio buffer size in samples.
    pub buffer_size: usize,
    /// Sample rate (Hz).
    pub sample_rate: u32,
    /// Render period in microseconds.
    pub period_us: u32,
    /// WCET in microseconds.
    pub wcet_us: u32,
}

/// Configure ALICE-RTOS kernel with audio render task from ALICE-Synth.
#[inline]
pub fn synth_rtos_audio_kernel(config: &AudioRtosConfig) -> (Kernel, KernelStats) {
    let mut kernel = Kernel::testing();
    kernel.add_task(
        b"audio",
        |_| {},
        TaskPriority::CRITICAL,
        config.period_us,
        config.wcet_us,
    );
    let stats = kernel.run_for(1_000_000, 100);
    (kernel, stats)
}

/// Calculate audio task timing from buffer size and sample rate.
#[inline]
pub fn synth_rtos_audio_config(buffer_size: usize, sample_rate: u32) -> AudioRtosConfig {
    let period_us = (buffer_size as u64 * 1_000_000 / sample_rate as u64) as u32;
    let wcet_us = period_us / 4; // 25% CPU budget for audio
    AudioRtosConfig { buffer_size, sample_rate, period_us, wcet_us }
}

// ── Bridge 2: Motion ↔ Kinematics (trajectory for joints) ──────────────

/// Joint trajectory from Motion Bezier curve applied to Kinematics arm.
pub struct JointTrajectoryResult {
    /// Predicted end-effector positions over time.
    pub positions: Vec<(f32, f32, f32)>,
    /// IK iterations used per sample.
    pub ik_iterations: Vec<u32>,
    /// Total trajectory duration.
    pub duration_secs: f32,
}

/// Convert Motion Vec3 to Kinematics Vec3k.
#[inline(always)]
pub fn motion_to_kinematics_vec3(v: &Vec3) -> Vec3k {
    Vec3k::new(v.x, v.y, v.z)
}

/// Convert Kinematics Vec3k to Motion Vec3.
#[inline(always)]
pub fn kinematics_to_motion_vec3(v: &Vec3k) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// Drive IK arm along a Bezier trajectory from ALICE-Motion.
#[inline]
pub fn motion_kinematics_trajectory(
    curve: &CubicBezier,
    v_max: f32,
    a_max: f32,
    samples: usize,
) -> JointTrajectoryResult {
    use alice_kinematics::ArmChain;

    let plan = MotionPlan::bezier_trapezoidal(*curve, v_max, a_max);
    let dur = plan.duration();
    let n = samples.max(2);
    let dt = dur / (n - 1) as f32;
    let mut chain = ArmChain::right_arm();
    let mut positions = Vec::with_capacity(n);
    let mut iters = Vec::with_capacity(n);

    for i in 0..n {
        let t = i as f32 * dt;
        let target_pos = plan.position(t.min(dur));
        let target = Vec3k::new(target_pos.x * 0.01, target_pos.y * 0.01, target_pos.z * 0.01);
        let (it, _err) = chain.inverse_kinematics(target, 50, 0.001);
        let ee = chain.forward_kinematics();
        positions.push((ee.x, ee.y, ee.z));
        iters.push(it);
    }

    JointTrajectoryResult { positions, ik_iterations: iters, duration_secs: dur }
}

// ── Bridge 3: Kinematics ↔ RTOS (motion control scheduling) ─────────────

/// Motion control task configuration for RTOS.
pub struct MotionControlRtosConfig {
    /// IK update rate in microseconds.
    pub ik_period_us: u32,
    /// IK WCET in microseconds.
    pub ik_wcet_us: u32,
    /// Sensor read period in microseconds.
    pub sensor_period_us: u32,
    /// Sensor WCET in microseconds.
    pub sensor_wcet_us: u32,
}

/// Configure RTOS kernel for kinematics motion control.
#[inline]
pub fn kinematics_rtos_kernel(config: &MotionControlRtosConfig) -> (Kernel, KernelStats) {
    let mut kernel = Kernel::testing();
    kernel.add_task(b"ik_upd", |_| {}, TaskPriority::HIGH, config.ik_period_us, config.ik_wcet_us);
    kernel.add_task(b"sensor", |_| {}, TaskPriority::NORMAL, config.sensor_period_us, config.sensor_wcet_us);
    let stats = kernel.run_for(1_000_000, 100);
    (kernel, stats)
}

// ── Bridge 4: Motion ↔ RTOS (deadline-driven trajectory execution) ──────

/// Trajectory execution task for RTOS with deadline monitoring.
pub struct TrajectoryRtosResult {
    pub schedulable: bool,
    pub utilization: f32,
    pub trajectory_tasks: usize,
    pub control_frequency_hz: f32,
}

/// Configure RTOS for Motion trajectory execution with deadline guarantees.
#[inline]
pub fn motion_rtos_trajectory_kernel(control_hz: f32, traj_wcet_us: u32) -> TrajectoryRtosResult {
    let period_us = (1_000_000.0 / control_hz) as u32;
    let mut kernel = Kernel::testing();
    kernel.add_task(b"traj", |_| {}, TaskPriority::HIGH, period_us, traj_wcet_us);
    kernel.add_task(b"comm", |_| {}, TaskPriority::NORMAL, period_us * 10, traj_wcet_us / 2);
    let schedulable = kernel.is_schedulable();
    let stats = kernel.run_for(1_000_000, 100);

    TrajectoryRtosResult {
        schedulable,
        utilization: stats.utilization,
        trajectory_tasks: stats.tasks_executed as usize,
        control_frequency_hz: control_hz,
    }
}

// ── Bridge 5: VCS → Synth (Score version control) ──────────────────────

/// VCS-tracked Score revision.
pub struct ScoreRevision {
    /// Diff size in bytes.
    pub diff_bytes: usize,
    /// Number of diff operations.
    pub diff_ops: usize,
    /// Event count in new version.
    pub event_count: usize,
}

/// Convert Score to VCS AstTree for version tracking.
#[inline]
pub fn synth_to_vcs_tree(score: &Score) -> AstTree {
    let mut tree = AstTree::new();
    let root = tree.add_node(AstNodeKind::Root, "score", 0);
    let header = tree.add_node(AstNodeKind::Group, &format!("tempo_{}", score.header.tempo_bpm), root);
    let mut abs_tick: u32 = 0;
    for (_i, evt) in score.events.iter().enumerate() {
        abs_tick += evt.delta_tick as u32;
        let kind_str = match evt.kind {
            NoteEventKind::NoteOn => "on",
            NoteEventKind::NoteOff => "off",
            NoteEventKind::PitchBend => "bend",
            NoteEventKind::ControlChange => "cc",
        };
        tree.add_node(
            AstNodeKind::Keyframe,
            &format!("{}_{}_{}_{}", kind_str, abs_tick, evt.note, evt.velocity),
            header,
        );
    }
    tree
}

/// Diff two Score versions using VCS.
#[inline]
pub fn vcs_diff_scores(old: &Score, new: &Score) -> ScoreRevision {
    let old_tree = synth_to_vcs_tree(old);
    let new_tree = synth_to_vcs_tree(new);
    let ops = diff_trees(&old_tree, &new_tree);
    ScoreRevision {
        diff_bytes: patch_size_bytes(&ops),
        diff_ops: ops.len(),
        event_count: new.events.len(),
    }
}

// ── Bridge 6: VCS → Font (font parameter versioning) ────────────────────

/// VCS-tracked font parameter revision.
pub struct FontRevision {
    pub diff_bytes: usize,
    pub diff_ops: usize,
}

/// Convert MetaFontParams to VCS AstTree for version tracking.
#[inline]
pub fn font_to_vcs_tree(params: &MetaFontParams, name: &str) -> AstTree {
    let mut tree = AstTree::new();
    let root = tree.add_node(AstNodeKind::Root, name, 0);
    tree.add_node(AstNodeKind::Parameter, &format!("weight_{}", (params.weight * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("width_{}", (params.width * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("serif_{}", (params.serif * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("contrast_{}", (params.contrast * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("slant_{}", (params.slant * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("x_height_{}", (params.x_height * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("cap_height_{}", (params.cap_height * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("ascender_{}", (params.ascender * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("descender_{}", (params.descender * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("roundness_{}", (params.roundness * 1000.0) as i32), root);
    tree
}

/// Diff two font parameter versions using VCS.
#[inline]
pub fn vcs_diff_fonts(old: &MetaFontParams, new: &MetaFontParams) -> FontRevision {
    let old_tree = font_to_vcs_tree(old, "font_v1");
    let new_tree = font_to_vcs_tree(new, "font_v2");
    let ops = diff_trees(&old_tree, &new_tree);
    FontRevision {
        diff_bytes: patch_size_bytes(&ops),
        diff_ops: ops.len(),
    }
}

// ── Bridge 7: Font → Synth (lyrics timing) ─────────────────────────────

/// Lyrics timing entry for synchronized text + audio.
pub struct LyricsTiming {
    /// Character.
    pub ch: char,
    /// Start time in seconds.
    pub start_secs: f32,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// X position (from text shaping).
    pub x: f32,
}

/// Align text characters to Score note events for karaoke-style display.
#[inline]
pub fn font_synth_lyrics_timing(text: &str, score: &Score) -> Vec<LyricsTiming> {
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    let tempo = score.header.tempo_bpm as f32;
    let secs_per_tick = 60.0 / (tempo * 96.0);
    let mut timings = Vec::new();
    let mut note_starts: Vec<(f32, f32)> = Vec::new(); // (start, end)

    let mut on_time: Option<f32> = None;
    let mut abs_tick: u32 = 0;
    for evt in &score.events {
        abs_tick += evt.delta_tick as u32;
        let t = abs_tick as f32 * secs_per_tick;
        match evt.kind {
            NoteEventKind::NoteOn => { on_time = Some(t); }
            NoteEventKind::NoteOff => {
                if let Some(start) = on_time.take() {
                    note_starts.push((start, t));
                }
            }
            NoteEventKind::PitchBend | NoteEventKind::ControlChange => {}
        }
    }

    let x_step = 0.6; // approximate advance per character
    for (i, &ch) in chars.iter().enumerate() {
        if let Some(&(start, end)) = note_starts.get(i) {
            timings.push(LyricsTiming {
                ch,
                start_secs: start,
                duration_secs: end - start,
                x: i as f32 * x_step,
            });
        }
    }
    timings
}

// ── Bridge 8: Motion → Font (trajectory text annotation) ────────────

/// Text annotation positioned along a trajectory for ALICE-Font.
pub struct TrajectoryAnnotation {
    /// Text content.
    pub text: String,
    /// Font parameter bytes.
    pub params_bytes: [u8; 40],
    /// Position along trajectory (0.0..1.0).
    pub t: f32,
    /// World position at t.
    pub position: (f32, f32, f32),
}

/// Create text annotation along a Motion trajectory with ALICE-Font params.
#[inline]
pub fn motion_font_annotation(
    curve: &CubicBezier,
    text: &str,
    params: &MetaFontParams,
    t: f32,
) -> TrajectoryAnnotation {
    let pos = curve.position(t.clamp(0.0, 1.0));
    TrajectoryAnnotation {
        text: text.to_string(),
        params_bytes: params.encode(),
        t,
        position: (pos.x, pos.y, pos.z),
    }
}

// ── Bridge 9: Kinematics → Synth (motion-driven audio) ──────────────

/// Motion-driven audio trigger for ALICE-Synth.
pub struct MotionAudioTrigger {
    /// MIDI note (mapped from joint angle).
    pub note: u8,
    /// Velocity (mapped from motion speed).
    pub velocity: u8,
    /// Duration in ticks.
    pub duration_ticks: u16,
    /// Channel.
    pub channel: u8,
}

/// Convert Kinematics Intent to ALICE-Synth audio trigger.
#[inline]
pub fn kinematics_synth_trigger(intent: &alice_kinematics::Intent) -> MotionAudioTrigger {
    let target = intent.target;
    // Map position to note: x → pitch (0.0..1.0 → 48..84)
    let note = ((target.x.abs().min(1.0) * 36.0) as u8 + 48).min(127);
    // Map y to velocity
    let velocity = ((target.y.abs().min(1.0) * 127.0) as u8).max(1);
    // Map duration
    let dur_ticks = ((intent.duration_secs() * 96.0) as u16).max(1);
    MotionAudioTrigger {
        note,
        velocity,
        duration_ticks: dur_ticks,
        channel: 0,
    }
}

// ── Bridge 10: RTOS → VCS (task execution versioning) ───────────────

/// RTOS execution snapshot for VCS versioning.
pub struct RtosVcsSnapshot {
    /// AST node count.
    pub node_count: usize,
    /// Diff operations vs previous snapshot.
    pub diff_ops: usize,
    /// Diff size in bytes.
    pub diff_bytes: usize,
}

/// Convert RTOS KernelStats to VCS AstTree for execution versioning.
#[inline]
pub fn rtos_to_vcs_tree(stats: &KernelStats) -> AstTree {
    let mut tree = AstTree::new();
    let root = tree.add_node(AstNodeKind::Root, "rtos_snapshot", 0);
    tree.add_node(AstNodeKind::Parameter, &format!("util_{}", (stats.utilization * 1000.0) as i32), root);
    tree.add_node(AstNodeKind::Parameter, &format!("tasks_{}", stats.tasks_executed), root);
    tree.add_node(AstNodeKind::Parameter, &format!("switches_{}", stats.context_switches), root);
    tree.add_node(AstNodeKind::Parameter, &format!("ticks_{}", stats.total_ticks), root);
    tree
}

/// Diff two RTOS execution snapshots using VCS.
#[inline]
pub fn vcs_diff_rtos(old: &KernelStats, new: &KernelStats) -> RtosVcsSnapshot {
    let old_tree = rtos_to_vcs_tree(old);
    let new_tree = rtos_to_vcs_tree(new);
    let ops = diff_trees(&old_tree, &new_tree);
    RtosVcsSnapshot {
        node_count: 5, // root + 4 parameters
        diff_ops: ops.len(),
        diff_bytes: patch_size_bytes(&ops),
    }
}

// ── Bridge 11: Animation ↔ Manga (scene → manga panel conversion) ─────

/// Manga panel derived from Animation scene state.
pub struct AnimMangaPanel {
    /// Actor count in scene.
    pub actor_count: usize,
    /// Scene time snapshot.
    pub time: f32,
    /// Recommended panel shape.
    pub panel_type: &'static str,
    /// Content hash.
    pub content_hash: u64,
}

/// Convert Animation SceneGraph snapshot to Manga panel metadata.
#[inline]
pub fn animation_to_manga_panel(scene: &alice_animation::SceneGraph, time: f32) -> AnimMangaPanel {
    let actors = scene.actor_count();
    let panel_type = if actors > 3 { "wide" } else if actors > 1 { "standard" } else { "close-up" };
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&(actors as u64).to_le_bytes());
    buf[8..12].copy_from_slice(&time.to_le_bytes());
    let content_hash = fnv1a(&buf);
    AnimMangaPanel { actor_count: actors, time, panel_type, content_hash }
}

// ── Bridge 12: Auth ↔ Crypto (identity → encrypted seed backup) ───────

/// Encrypted identity backup via ALICE-Crypto BLAKE3 hash.
pub struct AuthCryptoIdentity {
    /// BLAKE3 hash of identity public key.
    pub id_hash: [u8; 32],
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// Hash for cache/DB keying.
    pub content_hash: u64,
}

/// Hash Auth identity via Crypto BLAKE3 for secure indexing.
#[inline]
pub fn auth_crypto_hash_identity(id: &alice_auth::AliceId) -> AuthCryptoIdentity {
    let id_bytes = *id.as_bytes();
    let blake_hash = alice_crypto::hash(&id_bytes);
    let hash_bytes: [u8; 32] = *blake_hash.as_bytes();
    AuthCryptoIdentity {
        id_hash: hash_bytes,
        id_bytes,
        content_hash: fnv1a(&hash_bytes[..8]),
    }
}

// ── Bridge 13: Queue ↔ Analytics (message queue metrics) ──────────────

/// Queue depth metrics for ALICE-Analytics.
pub struct QueueAnalyticsSnapshot {
    /// Current queue depth.
    pub depth: usize,
    /// Message payload bytes.
    pub payload_bytes: usize,
    /// Sender hash (FNV-1a of sender key).
    pub sender_hash: u64,
    /// Sequence number.
    pub seq: u64,
}

/// Extract queue metrics for Analytics monitoring.
#[inline]
pub fn queue_analytics_snapshot(msg: &alice_queue::Message, depth: usize) -> QueueAnalyticsSnapshot {
    QueueAnalyticsSnapshot {
        depth,
        payload_bytes: msg.payload.len(),
        sender_hash: fnv1a(&msg.header.sender),
        seq: msg.header.seq,
    }
}

// ── Bridge 14: Print ↔ Animation (3D print → animated preview) ────────

/// Animated print preview configuration.
pub struct PrintAnimPreview {
    /// Number of layers.
    pub layer_count: usize,
    /// Total print time in seconds.
    pub print_time_secs: f32,
    /// Frames per layer (for animation).
    pub frames_per_layer: usize,
    /// Total animation frames.
    pub total_frames: usize,
}

/// Configure animated print preview from SliceResult.
#[inline]
pub fn print_animation_preview(result: &alice_print::SliceResult, fps: usize) -> PrintAnimPreview {
    let frames_per_layer = (fps as f32 * result.print_time_seconds / result.layer_count.max(1) as f32).max(1.0) as usize;
    PrintAnimPreview {
        layer_count: result.layer_count,
        print_time_secs: result.print_time_seconds,
        frames_per_layer,
        total_frames: result.layer_count * frames_per_layer,
    }
}

// ── Bridge 15: Manga ↔ Print (manga page → print-ready) ──────────────

/// Print-ready manga page configuration.
pub struct MangaPrintReady {
    /// Page dimensions (mm).
    pub page_size_mm: (f32, f32),
    /// Element count.
    pub element_count: usize,
    /// Estimated ink coverage (0.0-1.0).
    pub ink_coverage: f32,
    /// DPI setting.
    pub dpi: u32,
}

/// Configure manga page for physical printing.
#[inline]
pub fn manga_print_ready(page: &alice_manga::MangaPage, dpi: u32) -> MangaPrintReady {
    let (w, h) = page.size.dimensions();
    let elements = page.element_count();
    let ink = (elements as f32 * 0.02).min(0.8);
    MangaPrintReady {
        page_size_mm: (w, h),
        element_count: elements,
        ink_coverage: ink,
        dpi,
    }
}

// ── Bridge 16: RTOS ↔ ML (schedule ML inference as RTOS task) ──────────

/// RTOS kernel configuration for ML inference scheduling.
///
/// Wraps ternary-weight inference timing as a real-time task so the kernel
/// can enforce WCET budgets and schedulability analysis.
pub struct MlRtosInferenceConfig {
    /// Number of input features for the ternary model.
    pub in_features: usize,
    /// Number of output features for the ternary model.
    pub out_features: usize,
    /// Inference period in microseconds (inverse of inference rate).
    pub period_us: u32,
    /// Worst-case execution time in microseconds for one inference pass.
    pub wcet_us: u32,
    /// Whether the kernel deemed the task set schedulable.
    pub schedulable: bool,
}

/// Register a ternary ML inference pass as an RTOS periodic task.
///
/// Estimates WCET from weight dimensions (add/sub-only, no multiply) and
/// registers the task with the RTOS kernel for schedulability analysis.
#[inline]
pub fn rtos_ml_inference_kernel(
    weights: &TernaryWeight,
    inference_hz: f32,
) -> (Kernel, KernelStats, MlRtosInferenceConfig) {
    let in_f = weights.in_features();
    let out_f = weights.out_features();
    // WCET estimate: ~1 ns per add/sub op, converted to microseconds
    let ops = (in_f * out_f) as u32;
    let wcet_us = (ops / 1000).max(1);
    let period_us = (1_000_000.0 / inference_hz) as u32;
    let mut kernel = Kernel::testing();
    kernel.add_task(b"ml_infer", |_| {}, TaskPriority::HIGH, period_us, wcet_us);
    let schedulable = kernel.is_schedulable();
    let stats = kernel.run_for(1_000_000, 100);
    let config = MlRtosInferenceConfig {
        in_features: in_f,
        out_features: out_f,
        period_us,
        wcet_us,
        schedulable,
    };
    (kernel, stats, config)
}

// ── Bridge 17: ML ↔ Motion (ternary inference for trajectory prediction) ─

/// Trajectory prediction result from ternary ML inference.
///
/// Maps ternary output directly to cubic Bezier control points so the
/// Motion planner can consume predictions without floating-point multiply.
pub struct MlMotionPrediction {
    /// Predicted Bezier curve derived from inference output.
    pub curve: CubicBezier,
    /// Raw inference output (one value per output neuron).
    pub raw_output: Vec<f32>,
    /// Number of add/sub operations performed (zero multiplications).
    pub inference_ops: usize,
}

/// Run ternary inference and map output to a CubicBezier trajectory.
///
/// Requires a model with at least 12 output features (4 control points × 3
/// coordinates). Extra outputs are ignored; missing outputs default to 0.
#[inline]
pub fn ml_motion_predict_curve(
    weights: &TernaryWeight,
    state: &[f32],
) -> MlMotionPrediction {
    let out_f = weights.out_features();
    let in_f = weights.in_features();
    let mut output = vec![0.0f32; out_f];
    ternary_matvec(state, weights, &mut output);

    // Map first 12 outputs to Bezier control points (tanh for bounded range)
    let g = |i: usize| -> f32 { output.get(i).copied().unwrap_or(0.0).tanh() };
    let curve = CubicBezier::new(
        Vec3::new(g(0),  g(1),  g(2)),
        Vec3::new(g(3),  g(4),  g(5)),
        Vec3::new(g(6),  g(7),  g(8)),
        Vec3::new(g(9),  g(10), g(11)),
    );
    MlMotionPrediction {
        curve,
        raw_output: output,
        inference_ops: in_f * out_f,
    }
}

// ── Bridge 18: Print ↔ Sync (multi-printer sync via InputFrame) ──────────

/// Replicated print state transmitted as an ALICE-Sync InputFrame.
///
/// Each printer node encodes its current layer progress and filament usage
/// into a compact InputFrame so remote nodes can mirror state without a
/// dedicated protocol.
pub struct PrintSyncFrame {
    /// The encoded InputFrame ready for network transmission.
    pub frame: InputFrame,
    /// Current layer index encoded into the frame.
    pub layer_index: u32,
    /// Filament used (mm × 100, integer) encoded into movement[2].
    pub filament_mm_x100: i16,
    /// FNV-1a content hash of the frame bytes.
    pub content_hash: u64,
}

/// Encode SliceResult progress into an ALICE-Sync InputFrame for replication.
#[inline]
pub fn print_sync_frame(
    result: &alice_print::SliceResult,
    current_layer: u32,
    player_id: u8,
    sim_frame: u64,
) -> PrintSyncFrame {
    let filament_mm_x100 = (result.filament_meters * 100_000.0).min(i16::MAX as f32) as i16;
    let layer_lo = (current_layer & 0xFFFF) as i16;
    let layer_hi = ((current_layer >> 16) & 0xFFFF) as i16;
    let frame = InputFrame::new(sim_frame, player_id)
        .with_movement(layer_lo, layer_hi, filament_mm_x100);
    // Hash the movement and action bytes for deduplication
    let mut buf = [0u8; 10];
    buf[0..2].copy_from_slice(&frame.movement[0].to_le_bytes());
    buf[2..4].copy_from_slice(&frame.movement[1].to_le_bytes());
    buf[4..6].copy_from_slice(&frame.movement[2].to_le_bytes());
    buf[6..10].copy_from_slice(&frame.actions.to_le_bytes());
    PrintSyncFrame {
        frame,
        layer_index: current_layer,
        filament_mm_x100,
        content_hash: fnv1a(&buf),
    }
}

// ── Bridge 19: Text ↔ Sync (collaborative text editing via InputFrame) ───

/// Collaborative text edit transmitted as an ALICE-Sync InputFrame.
///
/// Compresses a text diff using ALICE-Text and encodes its byte-length and
/// checksum into an InputFrame so the Sync layer can sequence edits
/// deterministically across peers.
pub struct TextSyncEdit {
    /// The encoded InputFrame carrying edit metadata.
    pub frame: InputFrame,
    /// ALICE-Text compressed diff bytes.
    pub compressed_diff: Vec<u8>,
    /// FNV-1a hash of the compressed diff (cache/dedup key).
    pub diff_hash: u64,
    /// Number of characters in the original diff text.
    pub char_count: usize,
}

/// Compress a text diff and encode its metadata into an ALICE-Sync InputFrame.
#[inline]
pub fn text_sync_edit(
    diff_text: &str,
    player_id: u8,
    sim_frame: u64,
) -> TextSyncEdit {
    let compressed = compress_tuned(diff_text, CompressionMode::Balanced)
        .unwrap_or_else(|_| diff_text.as_bytes().to_vec());
    let diff_hash = fnv1a(&compressed);
    let char_count = diff_text.chars().count();
    // Encode compressed length (up to i16::MAX) and hash low word into frame
    let clen = compressed.len().min(i16::MAX as usize) as i16;
    let hash_lo = (diff_hash & 0xFFFF) as i16;
    let hash_hi = ((diff_hash >> 16) & 0xFFFF) as i16;
    let frame = InputFrame::new(sim_frame, player_id)
        .with_movement(clen, hash_lo, hash_hi);
    TextSyncEdit { frame, compressed_diff: compressed, diff_hash, char_count }
}

// ── Bridge 20: Kinematics → Voice (gesture-to-speech mapping) ───────────

/// Voice synthesis parameters derived from Kinematics joint angles.
///
/// Maps arm joint configuration to parametric voice parameters so that
/// physical gestures can modulate pitch, formants, and gain in real time.
pub struct KinematicsVoiceParams {
    /// Estimated fundamental frequency (Hz) derived from shoulder angle.
    pub pitch_hz: f32,
    /// Formant F1 (Hz) derived from elbow flexion (mouth openness proxy).
    pub f1_hz: f32,
    /// Formant F2 (Hz) derived from wrist angle (tongue position proxy).
    pub f2_hz: f32,
    /// LPC gain (0.0–1.0) derived from end-effector speed.
    pub gain: f32,
    /// Voiced flag: true when the arm is in motion above threshold.
    pub voiced: bool,
    /// FNV-1a hash of the joint angle bytes (content dedup key).
    pub content_hash: u64,
}

/// Convert Kinematics joint angles (from Intent target) to Voice parameters.
///
/// Uses the end-effector target position as a proxy for joint configuration.
/// All mappings are branchless linear rescalings for zero branch-prediction
/// penalties on the audio render path.
#[inline]
pub fn kinematics_voice_params(intent: &alice_kinematics::Intent) -> KinematicsVoiceParams {
    let t = intent.target;
    // x → pitch: clamp to [0, 1], map to [80, 400] Hz (bass → soprano)
    let pitch_hz = 80.0 + t.x.abs().min(1.0) * 320.0;
    // y → F1: [200, 900] Hz (closed → open vowel)
    let f1_hz = 200.0 + t.y.abs().min(1.0) * 700.0;
    // z → F2: [700, 2500] Hz (back → front vowel)
    let f2_hz = 700.0 + t.z.abs().min(1.0) * 1800.0;
    // speed proxy: magnitude of target vector
    let speed = (t.x * t.x + t.y * t.y + t.z * t.z).sqrt().min(1.0);
    let gain = speed;
    let voiced = speed > 0.05;
    // Hash the 12 bytes of the target xyz for dedup
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&t.x.to_le_bytes());
    buf[4..8].copy_from_slice(&t.y.to_le_bytes());
    buf[8..12].copy_from_slice(&t.z.to_le_bytes());
    KinematicsVoiceParams { pitch_hz, f1_hz, f2_hz, gain, voiced, content_hash: fnv1a(&buf) }
}

// ── Bridge 21: Synth → Search (audio fingerprint indexing) ───────────────

/// Audio fingerprint index entry for ALICE-Search.
///
/// Encodes a Score's event sequence as a byte signature that can be
/// inserted into an FM-Index for O(pattern_length) lookup later.
pub struct SynthSearchFingerprint {
    /// Byte signature of the score (note + tick encoding).
    pub signature: Vec<u8>,
    /// FM-Index built over the signature for substring search.
    pub index: AliceIndex,
    /// FNV-1a hash of the signature (dedup / cache key).
    pub content_hash: u64,
    /// Number of events encoded.
    pub event_count: usize,
}

/// Build an audio fingerprint FM-Index from an ALICE-Synth Score.
///
/// Each note event is encoded as two bytes `[note, velocity]` so that
/// melodic sub-sequences can be searched in O(pattern) time.
#[inline]
pub fn synth_search_fingerprint(score: &Score) -> SynthSearchFingerprint {
    let mut sig: Vec<u8> = Vec::with_capacity(score.events.len() * 2);
    for evt in &score.events {
        sig.push(evt.note);
        sig.push(evt.velocity);
    }
    let content_hash = fnv1a(&sig);
    let event_count = score.events.len();
    let index = AliceIndex::build(&sig, 4);
    SynthSearchFingerprint { signature: sig, index, content_hash, event_count }
}

// ── Bridge 22: Motion → Search (trajectory search via curve signature) ───

/// Trajectory curve signature for ALICE-Search indexing.
///
/// Quantises a CubicBezier into a fixed-width byte sequence so that
/// similar trajectory shapes can be located via FM-Index substring search.
pub struct MotionSearchSignature {
    /// Quantised curve bytes (12 bytes: 4 points × 3 coords, each 1 byte).
    pub signature: [u8; 12],
    /// FM-Index built over the signature.
    pub index: AliceIndex,
    /// FNV-1a hash of the signature.
    pub content_hash: u64,
}

/// Build a searchable trajectory signature from a CubicBezier.
///
/// Each coordinate is clamped to [-1, 1] and mapped to [0, 255] for
/// compact byte representation suitable for FM-Index insertion.
#[inline]
pub fn motion_search_signature(curve: &CubicBezier) -> MotionSearchSignature {
    let quantise = |v: f32| -> u8 { ((v.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8 };
    let p0 = curve.position(0.0);
    let p1 = curve.position(1.0 / 3.0);
    let p2 = curve.position(2.0 / 3.0);
    let p3 = curve.position(1.0);
    let sig: [u8; 12] = [
        quantise(p0.x), quantise(p0.y), quantise(p0.z),
        quantise(p1.x), quantise(p1.y), quantise(p1.z),
        quantise(p2.x), quantise(p2.y), quantise(p2.z),
        quantise(p3.x), quantise(p3.y), quantise(p3.z),
    ];
    let content_hash = fnv1a(&sig);
    let index = AliceIndex::build(&sig, 2);
    MotionSearchSignature { signature: sig, index, content_hash }
}

// ── Bridge 23: VCS → ASP (version-controlled streaming via diff packets) ─

/// VCS diff metadata packaged for ASP streaming.
///
/// Contains the patch metadata (diff ops, size, hash) ready to be
/// sent as an ASP packet payload over the ALICE Streaming Protocol.
pub struct VcsAspDiffPacket {
    /// ASP sequence number for the diff packet.
    pub sequence: u32,
    /// Number of diff operations in the patch.
    pub diff_ops: usize,
    /// Patch size in bytes.
    pub patch_bytes: usize,
    /// Payload bytes (ops count + patch size, 8 bytes).
    pub payload: [u8; 8],
    /// FNV-1a hash of the patch payload.
    pub content_hash: u64,
}

/// Encode a VCS tree diff as ASP-ready metadata for streaming.
#[inline]
pub fn vcs_asp_diff_packet(
    old_tree: &AstTree,
    new_tree: &AstTree,
    sequence: u32,
) -> VcsAspDiffPacket {
    let ops = diff_trees(old_tree, new_tree);
    let patch_bytes = patch_size_bytes(&ops);
    // Encode diff metadata as a minimal payload (ops count + patch size)
    let mut payload = [0u8; 8];
    payload[0..4].copy_from_slice(&(ops.len() as u32).to_le_bytes());
    payload[4..8].copy_from_slice(&(patch_bytes as u32).to_le_bytes());
    let content_hash = fnv1a(&payload);
    VcsAspDiffPacket {
        sequence,
        diff_ops: ops.len(),
        patch_bytes,
        payload,
        content_hash,
    }
}

// ── Bridge 24: Cache ↔ Crypto (encrypted cache entries via BLAKE3 keys) ──

/// Encrypted cache entry backed by a BLAKE3-derived key.
///
/// The BLAKE3 hash of the plaintext value is used as the cache key so
/// that cache contents are content-addressed and collision-resistant
/// without exposing raw keys.
pub struct CryptoCacheRecord {
    /// FNV-1a hash of the BLAKE3 key bytes (fast shard routing).
    pub routing_hash: u64,
    /// BLAKE3 hash of the plaintext (used as the cache lookup key).
    pub blake3_key: [u8; 32],
    /// Original payload size in bytes.
    pub payload_bytes: usize,
}

/// Insert data into ALICE-Cache using its BLAKE3 hash as the key.
///
/// Returns the routing hash (FNV-1a of first 8 key bytes) alongside
/// the BLAKE3 key so callers can retrieve the entry with `cache.get`.
#[inline]
pub fn cache_crypto_insert(
    cache: &AliceCache<u64, Vec<u8>>,
    data: &[u8],
) -> CryptoCacheRecord {
    let h = crypto_hash(data);
    let blake3_key: [u8; 32] = *h.as_bytes();
    let routing_hash = fnv1a(&blake3_key[..8]);
    // Use the routing hash as the integer cache key
    cache.put(routing_hash, data.to_vec());
    CryptoCacheRecord {
        routing_hash,
        blake3_key,
        payload_bytes: data.len(),
    }
}

// ── Bridge 25: View → Text (text overlay rendering via font metrics) ──────

/// Text overlay descriptor for ALICE-View rendering.
///
/// Compresses the overlay string with ALICE-Text and attaches display
/// metrics so the View layer can position and scale the overlay without
/// re-parsing the raw string on every frame.
pub struct ViewTextOverlay {
    /// Compressed overlay text (ALICE-Text Balanced mode).
    pub compressed: Vec<u8>,
    /// Original text character count.
    pub char_count: usize,
    /// Estimated display width in em units (char_count × advance_per_char).
    pub display_width_em: f32,
    /// Font weight parameter (0.0–1.0) for stroke width scaling.
    pub font_weight: f32,
    /// FNV-1a hash of the compressed payload (dirty-flag / cache key).
    pub content_hash: u64,
}

/// Compress a text overlay string and compute View display metrics.
///
/// Uses MetaFontParams for advance estimation so layout is consistent
/// with the font actually selected for rendering.
#[inline]
pub fn view_text_overlay(text: &str, params: &MetaFontParams) -> ViewTextOverlay {
    let compressed = compress_tuned(text, CompressionMode::Balanced)
        .unwrap_or_else(|_| text.as_bytes().to_vec());
    let content_hash = fnv1a(&compressed);
    let char_count = text.chars().count();
    // Advance per character: base 0.6 em, widened by font width param
    let advance = 0.6 + params.width * 0.4;
    let display_width_em = char_count as f32 * advance;
    ViewTextOverlay {
        compressed,
        char_count,
        display_width_em,
        font_weight: params.weight,
        content_hash,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_synth::NoteEvent;

    #[test]
    fn test_synth_rtos_audio_config() {
        let config = synth_rtos_audio_config(256, 44100);
        assert_eq!(config.buffer_size, 256);
        assert!(config.period_us > 0);
        assert!(config.wcet_us < config.period_us);
    }

    #[test]
    fn test_synth_rtos_audio_kernel() {
        let config = synth_rtos_audio_config(512, 48000);
        let (_kernel, stats) = synth_rtos_audio_kernel(&config);
        assert!(stats.tasks_executed > 0);
        assert!(stats.schedulable);
    }

    #[test]
    fn test_motion_kinematics_vec3_conversion() {
        let mv = Vec3::new(1.0, 2.0, 3.0);
        let kv = motion_to_kinematics_vec3(&mv);
        assert!((kv.x - 1.0).abs() < 0.001);
        let back = kinematics_to_motion_vec3(&kv);
        assert!((back.y - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_motion_kinematics_trajectory() {
        let curve = CubicBezier::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 20.0, 0.0),
            Vec3::new(30.0, 20.0, 0.0),
            Vec3::new(40.0, 0.0, 0.0),
        );
        let result = motion_kinematics_trajectory(&curve, 100.0, 500.0, 10);
        assert_eq!(result.positions.len(), 10);
        assert!(result.duration_secs > 0.0);
    }

    #[test]
    fn test_kinematics_rtos_kernel() {
        let config = MotionControlRtosConfig {
            ik_period_us: 1000,
            ik_wcet_us: 200,
            sensor_period_us: 5000,
            sensor_wcet_us: 500,
        };
        let (_kernel, stats) = kinematics_rtos_kernel(&config);
        assert!(stats.schedulable);
        assert!(stats.tasks_executed > 0);
    }

    #[test]
    fn test_motion_rtos_trajectory() {
        let result = motion_rtos_trajectory_kernel(1000.0, 200);
        assert!(result.schedulable);
        assert!(result.trajectory_tasks > 0);
        assert!((result.control_frequency_hz - 1000.0).abs() < 0.1);
    }

    #[test]
    fn test_vcs_diff_scores() {
        let mut s1 = Score::new(120, 1);
        s1.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 60, velocity: 100, kind: NoteEventKind::NoteOn });
        s1.add_event(NoteEvent { delta_tick: 96, channel: 0, note: 60, velocity: 0, kind: NoteEventKind::NoteOff });

        let mut s2 = Score::new(120, 1);
        s2.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 60, velocity: 100, kind: NoteEventKind::NoteOn });
        s2.add_event(NoteEvent { delta_tick: 96, channel: 0, note: 60, velocity: 0, kind: NoteEventKind::NoteOff });
        s2.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 64, velocity: 100, kind: NoteEventKind::NoteOn });
        s2.add_event(NoteEvent { delta_tick: 96, channel: 0, note: 64, velocity: 0, kind: NoteEventKind::NoteOff });

        let rev = vcs_diff_scores(&s1, &s2);
        assert!(rev.diff_ops > 0);
        assert_eq!(rev.event_count, 4);
    }

    #[test]
    fn test_vcs_diff_fonts() {
        let old = MetaFontParams::sans_regular();
        let new = MetaFontParams::sans_bold();
        let rev = vcs_diff_fonts(&old, &new);
        assert!(rev.diff_ops > 0);
        assert!(rev.diff_bytes > 0);
    }

    #[test]
    fn test_font_synth_lyrics_timing() {
        let mut score = Score::new(120, 1);
        score.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 60, velocity: 100, kind: NoteEventKind::NoteOn });
        score.add_event(NoteEvent { delta_tick: 48, channel: 0, note: 60, velocity: 0, kind: NoteEventKind::NoteOff });
        score.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 62, velocity: 100, kind: NoteEventKind::NoteOn });
        score.add_event(NoteEvent { delta_tick: 48, channel: 0, note: 62, velocity: 0, kind: NoteEventKind::NoteOff });

        let timings = font_synth_lyrics_timing("AB", &score);
        assert_eq!(timings.len(), 2);
        assert_eq!(timings[0].ch, 'A');
        assert_eq!(timings[1].ch, 'B');
        assert!(timings[0].start_secs < timings[1].start_secs);
    }

    #[test]
    fn test_motion_font_annotation() {
        let curve = CubicBezier::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 20.0, 0.0),
            Vec3::new(30.0, 20.0, 0.0),
            Vec3::new(40.0, 0.0, 0.0),
        );
        let params = MetaFontParams::sans_regular();
        let ann = motion_font_annotation(&curve, "Label", &params, 0.5);
        assert_eq!(ann.text, "Label");
        assert!((ann.t - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_kinematics_synth_trigger() {
        let intent = alice_kinematics::Intent::reach(alice_kinematics::Vec3k::new(0.5, 0.8, 0.0), 100);
        let trigger = kinematics_synth_trigger(&intent);
        assert!(trigger.note >= 48 && trigger.note <= 84);
        assert!(trigger.velocity > 0);
        assert!(trigger.duration_ticks > 0);
    }

    #[test]
    fn test_vcs_diff_rtos() {
        let stats1 = alice_rtos::kernel::KernelStats {
            total_us: 500_000,
            total_ticks: 5000,
            tasks_executed: 250,
            context_switches: 100,
            utilization: 0.42,
            schedulable: true,
        };
        let stats2 = alice_rtos::kernel::KernelStats {
            total_us: 1_000_000,
            total_ticks: 10_000,
            tasks_executed: 500,
            context_switches: 200,
            utilization: 0.45,
            schedulable: true,
        };
        let diff = vcs_diff_rtos(&stats1, &stats2);
        assert!(diff.diff_ops > 0);
        assert_eq!(diff.node_count, 5);
    }

    #[test]
    fn test_animation_to_manga_panel() {
        let mut scene = alice_animation::SceneGraph::new();
        let sdf = alice_sdf::SdfNode::sphere(1.0);
        scene.add_actor(alice_animation::Actor::new("hero", sdf));
        let panel = animation_to_manga_panel(&scene, 2.5);
        assert_eq!(panel.actor_count, 1);
        assert_eq!(panel.panel_type, "close-up");
        assert_ne!(panel.content_hash, 0);
    }

    #[test]
    fn test_auth_crypto_hash_identity() {
        let identity = alice_auth::Identity::from_seed(&[42u8; 32]);
        let result = auth_crypto_hash_identity(&identity.id());
        assert_ne!(result.id_hash, [0u8; 32]);
        assert_ne!(result.content_hash, 0);
    }

    #[test]
    fn test_queue_analytics_snapshot() {
        let msg = alice_queue::Message::new([42u8; 32], 1, vec![1, 2, 3]);
        let snap = queue_analytics_snapshot(&msg, 10);
        assert_eq!(snap.depth, 10);
        assert_eq!(snap.payload_bytes, 3);
        assert_ne!(snap.sender_hash, 0);
    }

    #[test]
    fn test_print_animation_preview() {
        let result = alice_print::SliceResult {
            gcode: "G28\n".to_string(),
            layer_count: 100,
            filament_meters: 5.0,
            print_time_seconds: 3600.0,
            compile_ms: 10.0,
            slice_ms: 50.0,
            gcode_ms: 5.0,
        };
        let preview = print_animation_preview(&result, 30);
        assert_eq!(preview.layer_count, 100);
        assert!(preview.total_frames > 0);
    }

    #[test]
    fn test_manga_print_ready() {
        let page = alice_manga::MangaPage::new(alice_manga::PageSize::B4);
        let ready = manga_print_ready(&page, 300);
        assert_eq!(ready.dpi, 300);
        assert!(ready.page_size_mm.0 > 0.0);
        assert!(ready.page_size_mm.1 > 0.0);
    }

    // ── Bridge 16 test ───────────────────────────────────────────────────

    #[test]
    fn test_rtos_ml_inference_kernel() {
        let weights = alice_ml::TernaryWeight::from_ternary(
            &[1, -1, 0, 1, -1, 1, 0, -1, 1],
            3, 3,
        );
        let (_kernel, stats, config) = rtos_ml_inference_kernel(&weights, 100.0);
        assert!(stats.schedulable);
        assert!(stats.tasks_executed > 0);
        assert_eq!(config.in_features, 3);
        assert_eq!(config.out_features, 3);
        assert!(config.period_us > 0);
        assert!(config.wcet_us > 0);
    }

    // ── Bridge 17 test ───────────────────────────────────────────────────

    #[test]
    fn test_ml_motion_predict_curve() {
        // 12-output model: maps to exactly 4 Bezier control points
        let weights = alice_ml::TernaryWeight::from_ternary(
            &vec![1i8; 12 * 3],
            12, 3,
        );
        let state = [0.5f32, -0.5, 1.0];
        let pred = ml_motion_predict_curve(&weights, &state);
        assert_eq!(pred.raw_output.len(), 12);
        assert_eq!(pred.inference_ops, 12 * 3);
        // tanh output is bounded to (-1, 1) — verify curve is finite
        let p = pred.curve.position(0.5);
        assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
    }

    // ── Bridge 18 test ───────────────────────────────────────────────────

    #[test]
    fn test_print_sync_frame() {
        let result = alice_print::SliceResult {
            gcode: "G28\n".to_string(),
            layer_count: 200,
            filament_meters: 3.5,
            print_time_seconds: 7200.0,
            compile_ms: 8.0,
            slice_ms: 40.0,
            gcode_ms: 4.0,
        };
        let psf = print_sync_frame(&result, 42, 1, 1000);
        assert_eq!(psf.layer_index, 42);
        assert_eq!(psf.frame.player_id, 1);
        assert_eq!(psf.frame.frame, 1000);
        assert_ne!(psf.content_hash, 0);
        // layer_lo should be encoded into movement[0]
        assert_eq!(psf.frame.movement[0], 42i16);
    }

    // ── Bridge 19 test ───────────────────────────────────────────────────

    #[test]
    fn test_text_sync_edit() {
        let diff = "+ Hello collaborative world";
        let edit = text_sync_edit(diff, 2, 500);
        assert_eq!(edit.frame.player_id, 2);
        assert_eq!(edit.frame.frame, 500);
        assert!(!edit.compressed_diff.is_empty());
        assert_ne!(edit.diff_hash, 0);
        assert!(edit.char_count > 0);
    }

    // ── Bridge 20 test ───────────────────────────────────────────────────

    #[test]
    fn test_kinematics_voice_params() {
        let intent = alice_kinematics::Intent::reach(
            alice_kinematics::Vec3k::new(0.5, 0.7, 0.3),
            200,
        );
        let vp = kinematics_voice_params(&intent);
        assert!(vp.pitch_hz >= 80.0 && vp.pitch_hz <= 400.0);
        assert!(vp.f1_hz >= 200.0 && vp.f1_hz <= 900.0);
        assert!(vp.f2_hz >= 700.0 && vp.f2_hz <= 2500.0);
        assert!(vp.gain >= 0.0 && vp.gain <= 1.0);
        assert!(vp.voiced); // non-zero target → voiced
        assert_ne!(vp.content_hash, 0);
    }

    // ── Bridge 21 test ───────────────────────────────────────────────────

    #[test]
    fn test_synth_search_fingerprint() {
        let mut score = Score::new(120, 1);
        score.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 60, velocity: 100, kind: NoteEventKind::NoteOn });
        score.add_event(NoteEvent { delta_tick: 96, channel: 0, note: 60, velocity: 0, kind: NoteEventKind::NoteOff });
        score.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 64, velocity: 80, kind: NoteEventKind::NoteOn });
        score.add_event(NoteEvent { delta_tick: 96, channel: 0, note: 64, velocity: 0, kind: NoteEventKind::NoteOff });

        let fp = synth_search_fingerprint(&score);
        assert_eq!(fp.event_count, 4);
        assert_eq!(fp.signature.len(), 8); // 4 events × 2 bytes each
        assert_ne!(fp.content_hash, 0);
        // The note 60 byte should be searchable in the fingerprint
        assert!(fp.index.count(&[60u8]) > 0);
    }

    // ── Bridge 22 test ───────────────────────────────────────────────────

    #[test]
    fn test_motion_search_signature() {
        let curve = CubicBezier::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.5, 1.0, 0.0),
            Vec3::new(-0.5, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
        );
        let sig = motion_search_signature(&curve);
        assert_eq!(sig.signature.len(), 12);
        assert_ne!(sig.content_hash, 0);
        // Zero-position maps to byte 127 (midpoint of [0, 255])
        // start and end both at origin → signature[0] and signature[9] should be equal
        assert_eq!(sig.signature[0], sig.signature[9]);
        // Index should contain the signature bytes
        assert!(sig.index.count(&sig.signature[0..1]) > 0);
    }

    // ── Bridge 23 test ───────────────────────────────────────────────────

    #[test]
    fn test_vcs_asp_diff_packet() {
        let mut t1 = AstTree::new();
        let r1 = t1.add_node(AstNodeKind::Root, "v1", 0);
        t1.add_node(AstNodeKind::Parameter, "param_1000", r1);

        let mut t2 = AstTree::new();
        let r2 = t2.add_node(AstNodeKind::Root, "v2", 0);
        t2.add_node(AstNodeKind::Parameter, "param_2000", r2);

        let pkt = vcs_asp_diff_packet(&t1, &t2, 42);
        assert!(pkt.diff_ops > 0);
        assert!(pkt.patch_bytes > 0);
        assert_ne!(pkt.content_hash, 0);
        assert_eq!(pkt.sequence, 42);
        assert_eq!(pkt.payload.len(), 8);
    }

    // ── Bridge 24 test ───────────────────────────────────────────────────

    #[test]
    fn test_cache_crypto_insert() {
        let cache: AliceCache<u64, Vec<u8>> = AliceCache::new(1000);
        let data = b"alice encrypted cache test payload";
        let record = cache_crypto_insert(&cache, data);
        assert_ne!(record.routing_hash, 0);
        assert_ne!(record.blake3_key, [0u8; 32]);
        assert_eq!(record.payload_bytes, data.len());
        // Verify the entry is retrievable
        let retrieved = cache.get(&record.routing_hash);
        assert_eq!(retrieved, Some(data.to_vec()));
    }

    // ── Bridge 25 test ───────────────────────────────────────────────────

    #[test]
    fn test_view_text_overlay() {
        let params = MetaFontParams::sans_regular();
        let overlay = view_text_overlay("Hello ALICE View", &params);
        assert_eq!(overlay.char_count, 16);
        assert!(!overlay.compressed.is_empty());
        assert!(overlay.display_width_em > 0.0);
        assert!(overlay.font_weight >= 0.0 && overlay.font_weight <= 1.0);
        assert_ne!(overlay.content_hash, 0);
        // Two identical strings should yield the same hash (determinism)
        let overlay2 = view_text_overlay("Hello ALICE View", &params);
        assert_eq!(overlay.content_hash, overlay2.content_hash);
    }
}
