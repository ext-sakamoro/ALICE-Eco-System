// Justified pedantic suppression for bridge/pipeline code:
// - inline_always: FNV-1a hash hot paths in every bridge file
// - cast_*: intentional narrowing in bridge serialization (u8→u64, usize→u32, f32→i32)
// - similar_names: bridge variable pairs mirror source/target crate naming
// - module_name_repetitions: bridge_xxx types intentionally repeat module prefix
// - too_many_lines: pipeline orchestration functions span multiple crate calls
// - missing_docs: bridge conversion functions are self-documenting by signature
// - unreadable_literal: FNV-1a constants are standard hex values
// - doc_markdown: bridge doc comments use snake_case field names as prose, not code references
// - ignored_unit_patterns: match guards on () use _ for readability in bridge encoding tables
// - too_many_arguments: some bridge conversion functions mirror multi-field source structs
// - missing_const_for_fn: bridge functions reference non-const dependencies
#![allow(
    clippy::inline_always,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::similar_names,
    clippy::module_name_repetitions,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::missing_docs_in_private_items,
    clippy::doc_markdown,
    clippy::ignored_unit_patterns,
    clippy::too_many_arguments,
    clippy::missing_const_for_fn
)]

//! ALICE Eco-System — Unified Pipeline Library
//!
//! Connects 183 ALICE crates into unified pipelines with 1,250 cross-crate bridges across 230 bridge modules.
//! Powers 146 `SaaS` services (AGPL-3.0-or-later) via the MIT Core + AGPL `SaaS` Shell pattern.
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
//! Path G (AI Inference → 3D):
//!   [ALICE-ML] → [ALICE-TRT] → [ALICE-SDF] / [ALICE-Physics] / [ALICE-View]
//!   [LLM] → [ALICE-LOL] → [ALICE-SDF] → [ALICE-View] / [ALICE-Print] / [ALICE-Physics]
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
//! Path T (Container Deployment):
//!   [ALICE-Container] → [ALICE-Auth] → [ALICE-API] → [ALICE-CDN]
//!
//! Path U (Presence Protocol):
//!   [ALICE-Presence] → [ALICE-Edge] (event telemetry) → [ALICE-Analytics] (crossing/proximity) → [ALICE-DB] / [ALICE-Cache]
//!
//! Path V (Tokenization Pipeline):
//!   [ALICE-Token] → [ALICE-Text] / [ALICE-ML] / [ALICE-Search] / [ALICE-DB] / [ALICE-Cache] / [ALICE-Analytics]
//!
//! Path W (Cognitive Agent Pipeline — Project-ALICE V3):
//!   [ALICE-Factory] → [ALICE-Core] → [ALICE-Cognitive] → [ALICE-Autonomy] → [ALICE-Consciousness]
//!   → [ALICE-Analytics] / [ALICE-DB] / [ALICE-Cache] / [ALICE-Compliance] / [ALICE-Log]
//!
//! Path X (Swarm Intelligence Pipeline — Project-ALICE V3):
//!   [ALICE-Factory] → [ALICE-Swarm(V3)] → [ALICE-Innovation]
//!   → [ALICE-Analytics] / [ALICE-DB] / [ALICE-Cache] / [ALICE-ML] / [ALICE-Search]
//! ```

pub mod bridge_analytics;
#[cfg(feature = "animation")]
pub mod bridge_animation;
pub mod bridge_api;
pub mod bridge_asp;
#[cfg(feature = "streaming-protocol-commercial")]
pub mod bridge_asp_commercial;
#[cfg(feature = "atoms")]
pub mod bridge_atoms;
#[cfg(feature = "atoms")]
pub mod bridge_atoms_cross;
pub mod bridge_auth;
pub mod bridge_auth_ext;
pub mod bridge_bio;
pub mod bridge_bio_cross;
pub mod bridge_bridge;
pub mod bridge_browser;
pub mod bridge_cache;
pub mod bridge_cdn;
pub mod bridge_cdn_ext;
pub mod bridge_climate;
pub mod bridge_climate_cross;
pub mod bridge_cloud_gateway;
pub mod bridge_codec;
pub mod bridge_container;
pub mod bridge_cross;
pub mod bridge_crypto;
pub mod bridge_crypto_ext;
pub mod bridge_db;
pub mod bridge_db_enterprise;
pub mod bridge_digital_twin_cross;
pub mod bridge_dns;
pub mod bridge_document_cross;
pub mod bridge_edge;
#[cfg(feature = "edge-commercial")]
pub mod bridge_edge_commercial;
pub mod bridge_edge_ext;
pub mod bridge_energy;
pub mod bridge_energy_cross;
#[cfg(feature = "firewall")]
pub mod bridge_firewall;
pub mod bridge_fix;
pub mod bridge_fix_ext;
pub mod bridge_font;
pub mod bridge_history;
pub mod bridge_history_cross;
pub mod bridge_image_cross;
pub mod bridge_kinematics;
pub mod bridge_ledger;
pub mod bridge_legal;
pub mod bridge_legal_cross;
pub mod bridge_lol;
#[cfg(feature = "manga")]
pub mod bridge_manga;
pub mod bridge_ml;
pub mod bridge_motion;
#[cfg(feature = "neural")]
pub mod bridge_neural;
#[cfg(feature = "neural")]
pub mod bridge_neural_cross;
pub mod bridge_physics;
pub mod bridge_physics_2d;
pub mod bridge_physics_scene_io;
pub mod bridge_physics_softbody;
pub mod bridge_presence;
pub mod bridge_presence_cross;
#[cfg(feature = "print")]
pub mod bridge_print_ext;
pub mod bridge_queue;
pub mod bridge_reverse;
pub mod bridge_risk;
pub mod bridge_risk_ext;
pub mod bridge_rtos;
pub mod bridge_sdf;
pub mod bridge_sdf_destruction;
pub mod bridge_sdf_material;
pub mod bridge_search;
pub mod bridge_semantic_telemetry;
pub mod bridge_semantic_telemetry_cross;
pub mod bridge_settlement_ext;
pub mod bridge_space;
pub mod bridge_space_cross;
pub mod bridge_sync;
pub mod bridge_synth;
pub mod bridge_telemetry_hooks;
pub mod bridge_text;
pub mod bridge_token;
pub mod bridge_train;
pub mod bridge_train_qat;
pub mod bridge_trt;
pub mod bridge_vcs;
pub mod bridge_view;
pub mod bridge_voice;
#[cfg(feature = "voice-commercial")]
pub mod bridge_voice_commercial;
pub mod bridge_zip;

// Infrastructure bridges
pub mod bridge_backup;
pub mod bridge_circuit;
pub mod bridge_config;
pub mod bridge_consensus;
pub mod bridge_log;
pub mod bridge_migrate;
pub mod bridge_scheduler;

// Data/Communication bridges
pub mod bridge_collab;
pub mod bridge_notify;
pub mod bridge_rate_limit;
pub mod bridge_realtime;
pub mod bridge_serial;
pub mod bridge_workflow;

// Security bridges
pub mod bridge_browser_secure;
pub mod bridge_compliance;
pub mod bridge_compliance_cross;
pub mod bridge_datashield;
pub mod bridge_fin_compliance;
pub mod bridge_fin_compliance_cross;

// Analytics/Monitoring bridges
pub mod bridge_experiment;
pub mod bridge_experiment_cross;
pub mod bridge_metrics;
pub mod bridge_metrics_cross;
pub mod bridge_observability;
pub mod bridge_test;

// Domain bridges
pub mod bridge_billing;
pub mod bridge_billing_cross;
pub mod bridge_digital_twin;
pub mod bridge_document;
pub mod bridge_geo;
pub mod bridge_graph;
pub mod bridge_graph_cross;
pub mod bridge_i18n;
pub mod bridge_legal_ai;

// SIMD bridge
pub mod bridge_simd;

// Media/Compression/Low-level bridges
pub mod bridge_ffi;
pub mod bridge_image;
pub mod bridge_text_compression;
pub mod bridge_vectordb;
pub mod bridge_vectordb_cross;
pub mod bridge_wasm;
pub mod bridge_wasm_cross;

// 2026-03 batch: Media bridges
pub mod bridge_asr;
pub mod bridge_audio;
pub mod bridge_camera;
pub mod bridge_ocr;
pub mod bridge_tts;
pub mod bridge_video;

// 2026-03 batch: Spatial bridges
pub mod bridge_drone;
pub mod bridge_medical;
pub mod bridge_navigation;
pub mod bridge_render;
pub mod bridge_slam;

// 2026-03 batch: Optimization bridges
pub mod bridge_agri;
pub mod bridge_logistics;
pub mod bridge_matchmaking;
pub mod bridge_recommend;

// 2026-03 batch: Real-time bridges
pub mod bridge_chat;
pub mod bridge_loadbalancer;
pub mod bridge_videoanalytics;

// 2026-03 batch: Compiler/Language bridges
pub mod bridge_compiler;
pub mod bridge_parser;
pub mod bridge_vm;

// 2026-03 batch: Networking bridges
pub mod bridge_grpc;
pub mod bridge_http;
pub mod bridge_proxy;
pub mod bridge_websocket;

// 2026-03 batch: Storage bridges
pub mod bridge_filesystem;
pub mod bridge_objectstore;

// 2026-03 batch: Security bridges
pub mod bridge_audit;
pub mod bridge_dlp;
pub mod bridge_waf;

// 2026-03 batch: AI/ML bridges
pub mod bridge_automl;
pub mod bridge_gan;
pub mod bridge_llm;
pub mod bridge_llm_ai;
pub mod bridge_llm_data;
pub mod bridge_llm_domain;
pub mod bridge_llm_ext;
pub mod bridge_llm_infra;
pub mod bridge_llm_media;
pub mod bridge_llm_science;
pub mod bridge_llm_spatial;

// 2026-03 batch: Swarm/IoT bridges
pub mod bridge_ble;
pub mod bridge_lora;
pub mod bridge_nfc;
pub mod bridge_swarm;

// 2026-03 batch: Infrastructure bridges
pub mod bridge_monitor;
pub mod bridge_terraform;

// 2026-03 batch: Science bridges
pub mod bridge_chemistry;
pub mod bridge_optics;
pub mod bridge_signal;

// 2026-03 batch: Business bridges
pub mod bridge_crm;
pub mod bridge_erp;
pub mod bridge_hrm;
pub mod bridge_lms;

// 2026-03 batch: XR bridges
pub mod bridge_vr;

// 2026-03 batch 2: AI/ML extended bridges
pub mod bridge_diffusion;
pub mod bridge_embedding;
pub mod bridge_nlp;
pub mod bridge_rag;
pub mod bridge_rl;
pub mod bridge_vision;

// 2026-03 batch 2: Science extended bridges
pub mod bridge_astro;
pub mod bridge_fluid;
pub mod bridge_genome;
pub mod bridge_quantum;

// 2026-03 batch 2: Financial extended bridges
pub mod bridge_market_data;
pub mod bridge_payment;
pub mod bridge_quant;

// 2026-03 batch 2: Media extended bridges
pub mod bridge_ar;
pub mod bridge_haptic;
pub mod bridge_point_cloud;
pub mod bridge_subtitle;

// 2026-03 batch 2: Security extended bridges
pub mod bridge_authz;
pub mod bridge_pki;
pub mod bridge_secret_vault;
pub mod bridge_siem;

// 2026-03 batch 2: Networking extended bridges
pub mod bridge_mqtt;
pub mod bridge_service_mesh;
pub mod bridge_vnet;

// 2026-03 batch 2: Data pipeline bridges
pub mod bridge_cdc;
pub mod bridge_etl;
pub mod bridge_lakehouse;
pub mod bridge_storage;
pub mod bridge_stream_proc;
pub mod bridge_time_series;

// 2026-03 batch 2: DevTools bridges
pub mod bridge_ci;
pub mod bridge_debug;
pub mod bridge_lint;
pub mod bridge_package_registry;
pub mod bridge_sandbox;

// 2026-03 batch 2: Application bridges
pub mod bridge_accessibility;
pub mod bridge_email;
pub mod bridge_form;
pub mod bridge_identity;
pub mod bridge_map;

// 2026-03 batch 2: Compute extended bridges
pub mod bridge_game_engine;
pub mod bridge_hypervisor;
pub mod bridge_robotics;
pub mod bridge_shader;

// 2026-03 batch 2: IoT extended bridges
pub mod bridge_iot;
pub mod bridge_ota;
pub mod bridge_sensor;

// 2026-03 batch 2: Cross-cutting bridges
pub mod bridge_blockchain;

// 2026-03: Cross-domain integration bridges (no dedicated crate required)
pub mod bridge_feature_store_cross;
pub mod bridge_graphql_cross;

// Project-ALICE V3 cognitive agent bridges
pub mod bridge_autonomy;
pub mod bridge_cognitive;
pub mod bridge_cognitive_swarm;
pub mod bridge_consciousness;
pub mod bridge_factory;
pub mod bridge_innovation;

pub mod hash;
pub mod pipeline;

// ── S9: ComposableBridge トレイト ──────────────────────────────────
/// Composable bridge trait for chaining bridge conversions.
///
/// Enables `bridge_a.then(bridge_b)` composition for multi-hop data flow.
pub trait BridgeConvert<T> {
    type Output;
    fn convert(&self, input: &T) -> Self::Output;
}

/// Chain two bridge conversions: A→B then B→C = A→C
pub struct BridgeChain<A, B> {
    pub first: A,
    pub second: B,
}

impl<A, B, Input, Mid, Output> BridgeConvert<Input> for BridgeChain<A, B>
where
    A: BridgeConvert<Input, Output = Mid>,
    B: BridgeConvert<Mid, Output = Output>,
{
    type Output = Output;
    fn convert(&self, input: &Input) -> Output {
        let mid = self.first.convert(input);
        self.second.convert(&mid)
    }
}

// Re-export pipeline API
pub use pipeline::{
    path_g_ai_inference, path_h_voice_delivery, path_i_fulltext_search, path_j_dns_api_gateway,
    AiInferenceResult, AlicePipeline, AnimeProductionResult, AssetDeliveryResult, CdnNodeConfig,
    ContainerDeployResult, DnsApiGatewayResult, EmbeddedResult, FullTextSearchResult,
    GameTickResult, MocapResult, PipelineConfig, PrintOptResult, SensorIngestResult,
    VoiceDeliveryResult,
};

// Re-export key types from constituent crates
pub use alice_cdn::content_types::ContentType;
pub use alice_physics::{Fix128, RigidBody, Vec3Fix};
pub use alice_sdf::{SdfNode, SdfTree};
pub use alice_sync::InputFrame;

// Re-export new crate key types
pub use alice_font::MetaFontParams;
pub use alice_kinematics::{ArmChain, Intent, Vec3k};
pub use alice_motion::{CubicBezier, MotionPlan, Vec3 as MotionVec3};
pub use alice_rtos::{Kernel, Task};
pub use alice_synth::{Score, Synthesizer};
pub use alice_vcs::{AstNodeKind, Repository};

// Re-export financial domain types
pub use alice_fix::{FixBuilder, FixMessage, FixSession};
pub use alice_ledger::{Fill, Order, OrderBook, OrderId, Position, PositionTracker, Side};
pub use alice_risk::{CircuitBreaker, MarginCalculator, PreTradeChecker, RiskLimits, RiskReject};
pub use alice_settlement::{
    ClearingHouse, NettingEngine, SettlementJournal, SettlementStatus, Trade,
};

// Re-export science & domain-specific types (Path L-Q)
pub use alice_bio::{AminoAcid, ProteinSdf, Residue, TotalEnergy};
pub use alice_climate::{AnomalyKind, ClimateResponse, Observation, WeatherStation};
pub use alice_energy::{BatteryState, PhaseCorrection, PowerGrid, PowerNode};
pub use alice_legal::{AuditEntry, AuditLog, Contract, StatuteTree};
pub use alice_space::{CommLink, ControlDecision, MissionEvent, ModelDifferential};

// Re-export advanced domain types (Path R-U)
pub use alice_history::{Fragment, FragmentKind, InversionConfig, RestorationResult};
pub use alice_presence::{CrossingRecord, CrossingStatus, PresenceEvent, VivaldiCoord};

// Re-export tokenizer types
pub use alice_token::{Tokenizer, Vocab, VocabBuilder};
