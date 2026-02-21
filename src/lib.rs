//! ALICE Eco-System — Unified Pipeline Library
//!
//! Connects 38 ALICE crates into unified pipelines with 240 cross-crate bridges.
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
//!
//! Path G (AI Inference):
//!   [ALICE-ML] → [ALICE-TRT] → [ALICE-SDF] / [ALICE-Physics] / [ALICE-View]
//!
//! Path H (Voice Delivery):
//!   [ALICE-Voice] → [ALICE-Synth] → [ALICE-Codec] → [ALICE-CDN] → [ALICE-Cache]
//!
//! Path I (Full-Text Search):
//!   [ALICE-Text] → [ALICE-Search] → [ALICE-DB] / [ALICE-Browser]
//!
//! Path J (DNS + API Gateway):
//!   [ALICE-DNS] → [ALICE-API] → [ALICE-Auth] / [ALICE-CDN] / [ALICE-Cache]
//! ```

pub mod hash;
pub mod pipeline;
pub mod bridge_font;
pub mod bridge_synth;
pub mod bridge_kinematics;
pub mod bridge_motion;
pub mod bridge_rtos;
pub mod bridge_vcs;
pub mod bridge_cross;
pub mod bridge_voice;
pub mod bridge_codec;
pub mod bridge_text;
pub mod bridge_search;
pub mod bridge_ml;
pub mod bridge_physics;
pub mod bridge_trt;
pub mod bridge_dns;
pub mod bridge_api;
pub mod bridge_zip;
pub mod bridge_auth;
pub mod bridge_crypto_ext;
#[cfg(feature = "animation")]
pub mod bridge_animation;
#[cfg(feature = "manga")]
pub mod bridge_manga;
#[cfg(feature = "print")]
pub mod bridge_print_ext;
pub mod bridge_analytics;
pub mod bridge_queue;
pub mod bridge_asp;
pub mod bridge_edge_ext;
pub mod bridge_cdn_ext;
pub mod bridge_container;
#[cfg(feature = "firewall")]
pub mod bridge_firewall;
pub mod bridge_browser;
pub mod bridge_cloud_gateway;
pub mod bridge_view;
pub mod bridge_db;
pub mod bridge_sync;
pub mod bridge_cache;
pub mod bridge_sdf;
pub mod bridge_semantic_telemetry;

// Re-export pipeline API
pub use pipeline::{
    AlicePipeline, AssetDeliveryResult, CdnNodeConfig, GameTickResult, PipelineConfig,
    SensorIngestResult, MocapResult, AnimeProductionResult, EmbeddedResult, PrintOptResult,
    AiInferenceResult, VoiceDeliveryResult, FullTextSearchResult, DnsApiGatewayResult,
    path_g_ai_inference, path_h_voice_delivery, path_i_fulltext_search, path_j_dns_api_gateway,
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
