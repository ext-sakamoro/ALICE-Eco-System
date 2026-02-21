//! ALICE Eco-System — Unified Pipeline Library
//!
//! Connects 51 ALICE crates into unified pipelines with 411 cross-crate bridges.
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
//!
//! Path K (Financial Trading):
//!   [ALICE-FIX] → [ALICE-Risk] → [ALICE-Ledger] → [ALICE-Settlement] → [ALICE-DB]
//!
//! Path L (Molecular Biology):
//!   [ALICE-Bio] → [ALICE-SDF] (molecular SDF) → [ALICE-Analytics] / [ALICE-DB] / [ALICE-Cache]
//!
//! Path M (Legal Compliance):
//!   [ALICE-Legal] → [ALICE-Analytics] (statute/contract metrics) → [ALICE-DB] (audit trail)
//!
//! Path N (Energy Grid):
//!   [ALICE-Energy] → [ALICE-Analytics] (grid balance) → [ALICE-Edge] (phase correction) → [ALICE-DB] / [ALICE-Cache]
//!
//! Path O (Deep-Space Communication):
//!   [ALICE-Space] → [ALICE-Edge] (differential telemetry) → [ALICE-Analytics] / [ALICE-DB] / [ALICE-Cache]
//!
//! Path P (Brain-Computer Interface):
//!   [ALICE-Neural] → [ALICE-Analytics] (spike rates) → [ALICE-Edge] (intent telemetry) → [ALICE-DB] / [ALICE-Cache]
//!
//! Path Q (Planetary Climate):
//!   [ALICE-Climate] → [ALICE-Analytics] (observation/field metrics) → [ALICE-Edge] (anomaly alerts) → [ALICE-DB] / [ALICE-Cache]
//!
//! Path R (Inverse Entropy Restoration):
//!   [ALICE-History] → [ALICE-Analytics] (degradation/quality metrics) → [ALICE-DB] (restoration records) → [ALICE-Cache]
//!
//! Path S (Molecular Compilation):
//!   [ALICE-Atoms] → [ALICE-Analytics] (crystal/band/property metrics) → [ALICE-DB] (compilation records) → [ALICE-Cache]
//!
//! Path T: Reserved for future use.
//!
//! Path U (Presence Protocol):
//!   [ALICE-Presence] → [ALICE-Edge] (event telemetry) → [ALICE-Analytics] (crossing/proximity) → [ALICE-DB] / [ALICE-Cache]
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
pub mod bridge_cdn;
pub mod bridge_edge;
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
pub mod bridge_ledger;
pub mod bridge_risk_ext;
pub mod bridge_fix_ext;
pub mod bridge_settlement_ext;
#[cfg(feature = "edge-commercial")]
pub mod bridge_edge_commercial;
#[cfg(feature = "streaming-protocol-commercial")]
pub mod bridge_asp_commercial;
pub mod bridge_bio;
pub mod bridge_legal;
pub mod bridge_energy;
pub mod bridge_space;
pub mod bridge_space_cross;
pub mod bridge_bio_cross;
pub mod bridge_legal_cross;
pub mod bridge_energy_cross;
#[cfg(feature = "neural")]
pub mod bridge_neural;
#[cfg(feature = "neural")]
pub mod bridge_neural_cross;
pub mod bridge_climate;
pub mod bridge_climate_cross;
pub mod bridge_history;
pub mod bridge_history_cross;
#[cfg(feature = "atoms")]
pub mod bridge_atoms;
#[cfg(feature = "atoms")]
pub mod bridge_atoms_cross;
pub mod bridge_presence;
pub mod bridge_presence_cross;

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

// Re-export financial domain types
pub use alice_ledger::{OrderBook, Order, OrderId, Side, Fill, Position, PositionTracker};
pub use alice_risk::{PreTradeChecker, RiskLimits, RiskReject, MarginCalculator, CircuitBreaker};
pub use alice_fix::{FixMessage, FixBuilder, FixSession};
pub use alice_settlement::{Trade, SettlementStatus, NettingEngine, ClearingHouse, SettlementJournal};

// Re-export science & domain-specific types (Path L-Q)
pub use alice_bio::{AminoAcid, Residue, ProteinSdf, TotalEnergy};
pub use alice_legal::{StatuteTree, Contract, AuditLog, AuditEntry};
pub use alice_energy::{PowerGrid, PowerNode, BatteryState, PhaseCorrection};
pub use alice_space::{CommLink, ModelDifferential, MissionEvent, ControlDecision};
pub use alice_climate::{WeatherStation, Observation, ClimateResponse, AnomalyKind};

// Re-export advanced domain types (Path R-U)
pub use alice_history::{Fragment, FragmentKind, RestorationResult, InversionConfig};
pub use alice_presence::{VivaldiCoord, PresenceEvent, CrossingRecord, CrossingStatus};
