//! ALICE Eco-System — Unified Pipeline Library
//!
//! Connects all 9 ALICE crates into a single Edge → DB pipeline.
//!
//! ```text
//! Path A (IoT/Sensor):
//!   [Sensor] → [ALICE-Edge] → [ALICE-ASP] → [ALICE-CDN] → [ALICE-DB]
//!
//! Path B (Game/3D Asset):
//!   [ALICE-SDF] → [ALICE-CDN] → [ALICE-Cache]
//!   [ALICE-Sync] → [ALICE-Physics] → [Replay/Telemetry] → [ALICE-DB]
//! ```

pub mod pipeline;

// Re-export pipeline API
pub use pipeline::{
    AlicePipeline, AssetDeliveryResult, CdnNodeConfig, GameTickResult, PipelineConfig,
    SensorIngestResult,
};

// Re-export key types from constituent crates
pub use alice_cdn::content_types::ContentType;
pub use alice_physics::{Fix128, RigidBody, Vec3Fix};
pub use alice_sdf::{SdfNode, SdfTree};
pub use alice_sync::InputFrame;
