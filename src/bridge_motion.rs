//! Motion bridges — ALICE-Motion ↔ Physics, Print, Animation, Edge, SDF
//!
//! 5 bridges connecting NURBS/Bezier trajectory control to the ALICE ecosystem.

use alice_motion::{CubicBezier, MotionPlan, TrapezoidalProfile, Vec3, VelocityProfile};
use alice_physics::Vec3Fix;

// ── Bridge 1: Motion → Physics (trajectory constraints) ─────────────────

/// Trajectory-constrained physics body state.
pub struct TrajectoryPhysicsState {
    /// Current position in Fix128.
    pub position: Vec3Fix,
    /// Current velocity in Fix128.
    pub velocity: Vec3Fix,
    /// Remaining duration.
    pub remaining_secs: f32,
    /// Progress (0.0..1.0).
    pub progress: f32,
}

/// Evaluate MotionPlan at time t and convert to ALICE-Physics Vec3Fix.
pub fn motion_to_physics_state(plan: &MotionPlan, t: f32) -> TrajectoryPhysicsState {
    let pos = plan.position(t);
    let vel = plan.velocity(t);
    let dur = plan.duration();
    let progress = if dur > 0.0 { (t / dur).min(1.0) } else { 1.0 };
    TrajectoryPhysicsState {
        position: Vec3Fix::from_f32(pos.x, pos.y, pos.z),
        velocity: Vec3Fix::from_f32(vel.x, vel.y, vel.z),
        remaining_secs: (dur - t).max(0.0),
        progress,
    }
}

/// Convert Physics Vec3Fix to Motion Vec3 (for trajectory feedback).
pub fn physics_to_motion_vec3(v: &Vec3Fix) -> Vec3 {
    Vec3::new(v.x.to_f32(), v.y.to_f32(), v.z.to_f32())
}

// ── Bridge 2: Motion → Print (S-curve → G-code feed rate) ───────────────

/// G-code motion segment for ALICE-Print.
pub struct GcodeMotionSegment {
    /// Start position (mm).
    pub start: (f32, f32, f32),
    /// End position (mm).
    pub end: (f32, f32, f32),
    /// Feed rate (mm/min).
    pub feed_rate: f32,
    /// Segment duration (seconds).
    pub duration_secs: f32,
}

/// Generate G-code segments from CubicBezier + velocity profile for ALICE-Print.
pub fn motion_to_print_segments(
    curve: &CubicBezier,
    v_max: f32,
    a_max: f32,
    num_segments: usize,
) -> Vec<GcodeMotionSegment> {
    let arc = curve.arc_length(64);
    let profile = TrapezoidalProfile::new(v_max, a_max, arc);
    let dur = profile.duration();
    let dt = dur / num_segments.max(1) as f32;
    let mut segments = Vec::with_capacity(num_segments);

    for i in 0..num_segments {
        let t0 = i as f32 * dt;
        let t1 = ((i + 1) as f32 * dt).min(dur);
        let s0 = profile.position_at(t0) / arc;
        let s1 = profile.position_at(t1) / arc;
        let p0 = curve.position(s0.min(1.0));
        let p1 = curve.position(s1.min(1.0));
        let speed = profile.velocity_at((t0 + t1) / 2.0);
        segments.push(GcodeMotionSegment {
            start: (p0.x, p0.y, p0.z),
            end: (p1.x, p1.y, p1.z),
            feed_rate: speed * 60.0, // mm/s → mm/min
            duration_secs: t1 - t0,
        });
    }
    segments
}

// ── Bridge 3: Motion → Animation (camera/character paths) ───────────────

/// Animation path keyframe for ALICE-Animation camera/character motion.
pub struct AnimPathKeyframe {
    /// Time offset in seconds.
    pub time_secs: f32,
    /// Position (x, y, z).
    pub position: (f32, f32, f32),
    /// Velocity vector (for motion blur direction).
    pub velocity: (f32, f32, f32),
    /// Speed (magnitude of velocity).
    pub speed: f32,
}

/// Sample MotionPlan into animation keyframes for ALICE-Animation.
pub fn motion_to_animation_keyframes(plan: &MotionPlan, fps: f32) -> Vec<AnimPathKeyframe> {
    let dur = plan.duration();
    let dt = 1.0 / fps;
    let count = (dur * fps) as usize + 1;
    let mut keyframes = Vec::with_capacity(count);
    let mut t = 0.0;

    while t <= dur {
        let p = plan.position(t);
        let v = plan.velocity(t);
        let spd = plan.speed(t);
        keyframes.push(AnimPathKeyframe {
            time_secs: t,
            position: (p.x, p.y, p.z),
            velocity: (v.x, v.y, v.z),
            speed: spd,
        });
        t += dt;
    }
    keyframes
}

// ── Bridge 4: Motion → Edge (actuator control packets) ──────────────────

/// Compact actuator control packet for ALICE-Edge IoT transport.
pub struct ActuatorEdgePacket {
    /// Bezier control points serialized (4 × 3 × f32 = 48 bytes).
    pub bezier_bytes: [u8; 48],
    /// Duration in milliseconds.
    pub duration_ms: u16,
    /// Max velocity (mm/s).
    pub v_max: f32,
}

/// Package CubicBezier trajectory for ALICE-Edge actuator streaming.
pub fn motion_to_edge_packet(curve: &CubicBezier, v_max: f32, duration_ms: u16) -> ActuatorEdgePacket {
    let mut bytes = [0u8; 48];
    let points = [curve.p0, curve.p1, curve.p2, curve.p3];
    for (i, p) in points.iter().enumerate() {
        let offset = i * 12;
        bytes[offset..offset + 4].copy_from_slice(&p.x.to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&p.y.to_le_bytes());
        bytes[offset + 8..offset + 12].copy_from_slice(&p.z.to_le_bytes());
    }
    ActuatorEdgePacket { bezier_bytes: bytes, duration_ms, v_max }
}

// ── Bridge 5: Motion → SDF (sweep SDF generation) ──────────────────────

/// Sweep profile for SDF extrusion along a Bezier path.
pub struct MotionSdfSweep {
    /// Sampled path points (x, y, z).
    pub path_points: Vec<(f32, f32, f32)>,
    /// Tangent vectors at each point.
    pub tangents: Vec<(f32, f32, f32)>,
    /// Arc length of the path.
    pub arc_length: f32,
    /// Number of samples.
    pub sample_count: usize,
}

/// Sample CubicBezier into path points for ALICE-SDF sweep extrusion.
pub fn motion_to_sdf_sweep(curve: &CubicBezier, samples: usize) -> MotionSdfSweep {
    let n = samples.max(2);
    let mut path_points = Vec::with_capacity(n);
    let mut tangents = Vec::with_capacity(n);

    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let p = curve.position(t);
        let v = curve.velocity(t);
        path_points.push((p.x, p.y, p.z));
        tangents.push((v.x, v.y, v.z));
    }

    MotionSdfSweep {
        path_points,
        tangents,
        arc_length: curve.arc_length(64),
        sample_count: n,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_curve() -> CubicBezier {
        CubicBezier::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 20.0, 0.0),
            Vec3::new(30.0, 20.0, 0.0),
            Vec3::new(40.0, 0.0, 0.0),
        )
    }

    #[test]
    fn test_motion_to_physics_state() {
        let curve = test_curve();
        let plan = MotionPlan::bezier_trapezoidal(curve, 100.0, 500.0);
        let state = motion_to_physics_state(&plan, 0.0);
        assert!(state.remaining_secs > 0.0);
        assert!((state.progress - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_physics_to_motion_vec3() {
        let v = Vec3Fix::from_f32(1.5, 2.5, 3.5);
        let m = physics_to_motion_vec3(&v);
        assert!((m.x - 1.5).abs() < 0.01);
        assert!((m.y - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_motion_to_print_segments() {
        let curve = test_curve();
        let segs = motion_to_print_segments(&curve, 100.0, 500.0, 10);
        assert_eq!(segs.len(), 10);
        assert!(segs[0].feed_rate > 0.0);
        assert!(segs[0].duration_secs > 0.0);
    }

    #[test]
    fn test_motion_to_animation_keyframes() {
        let curve = test_curve();
        let plan = MotionPlan::bezier_trapezoidal(curve, 100.0, 500.0);
        let kf = motion_to_animation_keyframes(&plan, 30.0);
        assert!(kf.len() > 1);
        assert!((kf[0].time_secs - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_motion_to_edge_packet() {
        let curve = test_curve();
        let pkt = motion_to_edge_packet(&curve, 100.0, 500);
        assert_eq!(pkt.bezier_bytes.len(), 48);
        assert_eq!(pkt.duration_ms, 500);
        // Verify first point is (0, 0, 0)
        let x = f32::from_le_bytes([pkt.bezier_bytes[0], pkt.bezier_bytes[1], pkt.bezier_bytes[2], pkt.bezier_bytes[3]]);
        assert!((x - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_motion_to_sdf_sweep() {
        let curve = test_curve();
        let sweep = motion_to_sdf_sweep(&curve, 20);
        assert_eq!(sweep.sample_count, 20);
        assert_eq!(sweep.path_points.len(), 20);
        assert_eq!(sweep.tangents.len(), 20);
        assert!(sweep.arc_length > 0.0);
    }
}
