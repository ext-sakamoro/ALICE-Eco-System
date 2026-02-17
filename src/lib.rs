//! ALICE Eco-System — Unified Pipeline Library
//!
//! Connects 15 ALICE crates into unified pipelines with 144+ cross-crate bridges.
//!
//! ```text
//! Path A (IoT/Sensor):
//!   [Sensor] → [ALICE-Edge] → [ALICE-ASP] → [ALICE-CDN] → [ALICE-DB]
//!
//! Path B (Game/3D Asset):
//!   [ALICE-SDF] → [ALICE-CDN] → [ALICE-Cache]
//!   [ALICE-Sync] → [ALICE-Physics] → [Replay/Telemetry] → [ALICE-DB]
//!
//! Path C (Motion Capture Streaming):
//!   [ALICE-Edge] → [ALICE-Kinematics] → [ALICE-Sync] → [ALICE-Physics] → [ALICE-View]
//!
//! Path D (Anime Production):
//!   [ALICE-VCS] → [ALICE-SDF] → [ALICE-Animation] + [ALICE-Font] + [ALICE-Synth] → [ALICE-CDN]
//!
//! Path E (Real-Time Embedded):
//!   [ALICE-RTOS] → [ALICE-Edge] → [ALICE-Synth] → [ALICE-Streaming-Protocol]
//!
//! Path F (3D Print Optimization):
//!   [ALICE-SDF] → [ALICE-Motion] (S-curve) → [ALICE-Print] → .3mf
//! ```

pub mod pipeline;
pub mod bridge_font;
pub mod bridge_synth;
pub mod bridge_kinematics;
pub mod bridge_motion;
pub mod bridge_rtos;
pub mod bridge_vcs;
pub mod bridge_cross;

// Re-export pipeline API
pub use pipeline::{
    AlicePipeline, AssetDeliveryResult, CdnNodeConfig, GameTickResult, PipelineConfig,
    SensorIngestResult, MocapResult, AnimeProductionResult, EmbeddedResult, PrintOptResult,
};

// Re-export key types from constituent crates
pub use alice_cdn::content_types::ContentType;
pub use alice_physics::{Fix128, RigidBody, Vec3Fix};
pub use alice_sdf::{SdfNode, SdfTree};
pub use alice_sync::InputFrame;

// Re-export new crate key types
pub use alice_font::MetaFontParams;
pub use alice_synth::{Score, Synthesizer};
pub use alice_kinematics::{ArmChain, Intent, Vec3k};
pub use alice_motion::{CubicBezier, MotionPlan, Vec3 as MotionVec3};
pub use alice_rtos::{Kernel, Task};
pub use alice_vcs::{Repository, AstNodeKind};
