//! Cross-bridges — Inter-connections among the 6 new ALICE crates
//!
//! 10 bridges connecting Synth↔RTOS, Motion↔Kinematics, Kinematics↔RTOS,
//! Motion↔RTOS, VCS→Synth, VCS→Font, Font→Synth, Motion→Font,
//! Kinematics→Synth, RTOS→VCS.

use alice_font::param::MetaFontParams;
use alice_kinematics::Vec3k;
use alice_motion::{CubicBezier, MotionPlan, Vec3};
use alice_rtos::{Kernel, TaskPriority};
use alice_rtos::kernel::KernelStats;
use alice_synth::{NoteEventKind, Score};
use alice_vcs::ast::{AstNodeKind, AstTree};
use alice_vcs::diff::{diff_trees, patch_size_bytes};

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
pub fn motion_to_kinematics_vec3(v: &Vec3) -> Vec3k {
    Vec3k::new(v.x, v.y, v.z)
}

/// Convert Kinematics Vec3k to Motion Vec3.
pub fn kinematics_to_motion_vec3(v: &Vec3k) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// Drive IK arm along a Bezier trajectory from ALICE-Motion.
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
            _ => {}
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
}
