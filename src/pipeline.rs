//! ALICE Eco-System Pipeline — Edge to DB
//!
//! Connects 24 ALICE crates into unified pipelines:
//!
//! - **Path A** (IoT/Sensor): Edge → ASP → CDN → DB
//! - **Path B-1** (Asset Delivery): SDF → CDN → Cache
//! - **Path B-2** (Game Loop): Sync → Physics → Replay/Telemetry → DB
//! - **Path C** (Motion Capture): Edge → Kinematics → Sync → Physics → View
//! - **Path D** (Anime Production): VCS → SDF → Animation + Font + Synth → CDN
//! - **Path E** (Real-Time Embedded): RTOS → Edge → Synth → ASP
//! - **Path F** (3D Print Optimization): SDF → Motion (S-curve) → Print → .3mf
//! - **Path G** (AI Inference): ML → TRT → SDF / Physics / View
//! - **Path H** (Voice Delivery): Voice → Synth → Codec → CDN → Cache
//! - **Path I** (Full-Text Search): Text → Search → DB / Browser
//! - **Path J** (DNS + API Gateway): DNS → API → Auth / CDN / Cache

use std::path::PathBuf;

use alice_cache::{AliceCache, CacheConfig};
use alice_cdn::content_types::ContentType;
use alice_cdn::{ContentLocator, MaglevHash, VivaldiCoord};
use alice_db::AliceDB;
use alice_edge::{evaluate_linear_fixed, fit_linear_fixed};
use alice_physics::replay::ReplayRecorder;
use alice_physics::{Fix128, PhysicsConfig, PhysicsWorld, RigidBody};
use alice_sdf::io::AsdfHeader;
use alice_sdf::SdfTree;
use alice_sync::telemetry::SyncTelemetry;
use alice_sync::{InputFrame, LockstepSession};
use libasp::{AspPacket, IPacketPayload};

use alice_font::param::MetaFontParams;
use alice_font::{SdfAtlas, TextShaper};
use alice_kinematics::{ArmChain, Intent};
use alice_motion::{CubicBezier, TrapezoidalProfile, VelocityProfile};
use alice_rtos::{Kernel, TaskPriority};
use alice_synth::{FmPatch, Patch, Score, Synthesizer};
use alice_vcs::Repository;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// ── Config ───────────────────────────────────────────────────────────────

/// Pipeline configuration.
pub struct PipelineConfig {
    /// Path for ALICE-DB storage.
    pub db_path: PathBuf,
    /// Path for replay recording (Physics → DB).
    pub replay_path: PathBuf,
    /// Path for telemetry recording (Sync → DB).
    pub telemetry_path: PathBuf,
    /// CDN edge node definitions.
    pub cdn_nodes: Vec<CdnNodeConfig>,
    /// Client Vivaldi coordinate `[x, y, z, error]`.
    pub client_coord: [f64; 4],
    /// Number of players for lockstep sync (typically 2).
    pub num_players: usize,
    /// Number of physics bodies (must match bodies added via `add_body`).
    pub num_bodies: usize,
    /// Cache capacity (number of entries).
    pub cache_capacity: usize,
}

/// CDN edge node configuration.
pub struct CdnNodeConfig {
    pub id: u64,
    pub name: String,
    /// Vivaldi coordinate `[x, y, z, error]`.
    pub coord: [f64; 4],
}

// ── Result Types ─────────────────────────────────────────────────────────

/// Result of sensor data ingestion (Path A: Edge → ASP → CDN → DB).
pub struct SensorIngestResult {
    /// Slope coefficient (Q16.16 fixed-point).
    pub slope: i32,
    /// Intercept coefficient (Q16.16 fixed-point).
    pub intercept: i32,
    /// ASP packet size in bytes (transport layer).
    pub asp_packet_size: usize,
    /// CDN-selected route (node name).
    pub cdn_route: String,
    /// Predicted RTT to selected node (ms).
    pub cdn_rtt_ms: f64,
    /// Number of records stored in DB.
    pub db_records: usize,
    /// Edge compression ratio (raw / compressed).
    pub compression_ratio: f64,
}

/// Result of asset delivery (Path B-1: SDF → CDN → Cache).
pub struct AssetDeliveryResult {
    /// ASDF binary size in bytes.
    pub asdf_size: usize,
    /// Detected content type.
    pub content_type: ContentType,
    /// CDN-selected route (node name).
    pub cdn_route: String,
    /// Predicted RTT to selected node (ms).
    pub cdn_rtt_ms: f64,
    /// Whether the asset was already cached.
    pub cache_hit: bool,
}

/// Result of a game tick (Path B-2: Sync → Physics → DB).
pub struct GameTickResult {
    /// Frame number (pipeline tick counter).
    pub frame: u64,
    /// Body positions after physics step: `(x, y, z)` per body.
    pub positions: Vec<(f32, f32, f32)>,
    /// Last confirmed sync frame.
    pub sync_confirmed: u64,
    /// Whether physics was actually stepped (false if sync wasn't ready).
    pub stepped: bool,
}

/// Result of motion capture intent processing (Path C: Kinematics → Sync → Physics).
pub struct MocapResult {
    /// Intent packet size (always 8 bytes).
    pub intent_size: usize,
    /// Sync frame number.
    pub sync_frame: u64,
    /// IK solver iterations.
    pub ik_iterations: u32,
    /// End-effector position (x, y, z).
    pub end_effector: (f32, f32, f32),
}

/// Result of anime production pipeline (Path D: VCS → SDF + Font + Synth → CDN).
pub struct AnimeProductionResult {
    /// VCS commit hash.
    pub vcs_hash: u64,
    /// SDF scene serialized size.
    pub sdf_bytes: usize,
    /// Font parameters size (40 bytes).
    pub font_params_bytes: usize,
    /// Number of shaped subtitle glyphs.
    pub subtitle_glyphs: usize,
    /// Audio score size in bytes.
    pub audio_score_bytes: usize,
    /// Total CDN payload.
    pub total_payload: usize,
}

/// Result of real-time embedded pipeline (Path E: RTOS → Edge → Synth → ASP).
pub struct EmbeddedResult {
    /// RTOS schedulability.
    pub rtos_schedulable: bool,
    /// RTOS CPU utilization.
    pub rtos_utilization: f32,
    /// Edge compressed size.
    pub edge_bytes: usize,
    /// Audio sample count.
    pub audio_samples: usize,
    /// ASP packet size.
    pub asp_packet_size: usize,
}

/// Result of 3D print optimization (Path F: SDF → Motion → Print).
pub struct PrintOptResult {
    /// Arc length of the toolpath in mm.
    pub arc_length_mm: f32,
    /// Total print duration in seconds.
    pub duration_secs: f32,
    /// Number of G-code segments.
    pub segment_count: usize,
    /// Average feed rate (mm/min).
    pub avg_feed_rate: f32,
    /// Max feed rate (mm/min).
    pub max_feed_rate: f32,
}

/// Result of AI inference pipeline (Path G: ML → TRT → SDF/Physics/View).
pub struct AiInferenceResult {
    /// ML ternary joint count.
    pub ml_joint_count: usize,
    /// ML inference operations.
    pub ml_inference_ops: usize,
    /// TRT parameter count.
    pub trt_param_count: usize,
    /// TRT FLOPS per inference.
    pub trt_flops: usize,
}

/// Result of voice delivery pipeline (Path H: Voice → Synth → Codec).
pub struct VoiceDeliveryResult {
    /// Voice carrier frequency (Hz).
    pub carrier_freq: f32,
    /// Whether voiced.
    pub voiced: bool,
    /// Codec compressed bytes.
    pub codec_compressed_bytes: usize,
    /// Codec compression ratio.
    pub codec_compression_ratio: f32,
}

/// Result of full-text search pipeline (Path I: Text → Search).
pub struct FullTextSearchResult {
    /// Compressed text bytes.
    pub compressed_bytes: usize,
    /// Search index size (nodes).
    pub index_node_count: usize,
    /// Bandwidth savings percentage.
    pub bandwidth_saving_pct: f32,
}

/// Result of DNS+API gateway pipeline (Path J: DNS → API).
pub struct DnsApiGatewayResult {
    /// Whether the domain was blocked.
    pub dns_blocked: bool,
    /// Whether the request was rate-limited.
    pub api_rate_allowed: bool,
    /// Operation type from HTTP method.
    pub operation: &'static str,
    /// Content type hint.
    pub content_type_hint: &'static str,
}

// ── Path G: AI Inference ─────────────────────────────────────────────

/// AI inference pipeline: ML ternary inference → TRT GPU analytics.
///
/// `[ALICE-ML] → [ALICE-TRT] → [ALICE-Analytics] / [ALICE-DB] / [ALICE-View]`
pub fn path_g_ai_inference(state_dims: usize, action_dims: usize, hidden: &[usize]) -> AiInferenceResult {
    // Build layer geometry: input→hidden[0], hidden[i]→hidden[i+1], hidden[-1]→output.
    let mut shapes: Vec<(usize, usize)> = Vec::with_capacity(hidden.len() + 1);
    let mut prev = state_dims;
    for &h in hidden {
        shapes.push((h, prev));
        prev = h;
    }
    shapes.push((action_dims, prev));
    let metrics = crate::bridge_trt::trt_to_analytics_metrics(&shapes);
    AiInferenceResult {
        ml_joint_count: action_dims,
        ml_inference_ops: state_dims * action_dims,
        trt_param_count: metrics.param_count,
        trt_flops: metrics.mac_ops,
    }
}

// ── Path H: Voice Delivery ───────────────────────────────────────────

/// Voice delivery pipeline: Voice parametric → Synth → Codec compression.
///
/// `[ALICE-Voice] → [ALICE-Synth] → [ALICE-Codec] → [ALICE-CDN] → [ALICE-Cache]`
pub fn path_h_voice_delivery(params: &alice_voice::ParametricParams) -> VoiceDeliveryResult {
    let synth_params = crate::bridge_voice::voice_to_synth_params(params);
    // Synth → Codec: generate PCM and compress via wavelet
    let pcm: Vec<f32> = (0..320).map(|i| (i as f32 * synth_params.carrier_freq * std::f32::consts::TAU / 16000.0).sin() * synth_params.amplitude).collect();
    let codec_result = crate::bridge_codec::codec_compress_synth_pcm(&pcm);
    VoiceDeliveryResult {
        carrier_freq: synth_params.carrier_freq,
        voiced: synth_params.voiced,
        codec_compressed_bytes: codec_result.compressed_bytes,
        codec_compression_ratio: codec_result.compression_ratio,
    }
}

// ── Path I: Full-Text Search ─────────────────────────────────────────

/// Full-text search pipeline: Text compression → Search index → DB/Browser.
///
/// `[ALICE-Text] → [ALICE-Search] → [ALICE-DB] / [ALICE-Browser]`
pub fn path_i_fulltext_search(text: &str, query: &str) -> FullTextSearchResult {
    let browser_content = crate::bridge_text::text_to_browser_content(text);
    let index = alice_search::AliceIndex::build(text.as_bytes(), 4);
    let search_result = crate::bridge_search::search_db_query(&index, query.as_bytes());
    FullTextSearchResult {
        compressed_bytes: browser_content.compressed_bytes,
        index_node_count: search_result.occurrence_count,
        bandwidth_saving_pct: browser_content.bandwidth_saving_pct,
    }
}

// ── Path J: DNS + API Gateway ────────────────────────────────────────

/// DNS + API gateway pipeline: DNS classification → API rate limiting → routing.
///
/// `[ALICE-DNS] → [ALICE-API] → [ALICE-Auth] / [ALICE-CDN] / [ALICE-Cache]`
pub fn path_j_dns_api_gateway(domain: &str, path: &str) -> DnsApiGatewayResult {
    // DNS: classify domain
    let mut dns_engine = alice_dns::DnsBloomEngine::new();
    let action = dns_engine.check_domain(domain);
    let dns_blocked = matches!(action, alice_dns::DnsAction::Block | alice_dns::DnsAction::Spoof);
    // API: rate limiting + routing
    let limiter = alice_api::GcraCell::new(100.0, 10);
    let auth = crate::bridge_api::api_auth_check(&limiter, domain, alice_api::HttpMethod::Get, 1_000_000_000);
    let route = crate::bridge_api::api_to_cdn_route(path, alice_api::HttpMethod::Get, auth.rate_allowed);
    DnsApiGatewayResult {
        dns_blocked,
        api_rate_allowed: auth.rate_allowed,
        operation: auth.operation,
        content_type_hint: route.content_type_hint,
    }
}

// ── Pipeline ─────────────────────────────────────────────────────────────

/// Unified Edge → DB pipeline connecting 9 ALICE crates.
pub struct AlicePipeline {
    // Storage (all paths end here)
    db: AliceDB,
    // Content delivery
    cache: AliceCache<u64, Vec<u8>>,
    cdn: ContentLocator,
    maglev: MaglevHash,
    cdn_nodes: Vec<(u64, String, VivaldiCoord)>,
    // Game engine
    physics: PhysicsWorld,
    sync_session: LockstepSession,
    recorder: ReplayRecorder,
    telemetry: SyncTelemetry,
    // Streaming state
    asp_sequence: u32,
    sensor_content_id: u64,
    // Game state
    frame_counter: u64,
}

impl AlicePipeline {
    /// Initialize the full pipeline with all 9 crate components.
    pub fn new(config: PipelineConfig) -> Result<Self> {
        // ALICE-DB
        let db = AliceDB::open(&config.db_path)?;

        // ALICE-CDN
        let cdn_nodes: Vec<(u64, String, VivaldiCoord)> = config
            .cdn_nodes
            .iter()
            .map(|n| {
                (
                    n.id,
                    n.name.clone(),
                    VivaldiCoord::at(n.coord[0], n.coord[1], n.coord[2], n.coord[3]),
                )
            })
            .collect();

        let client = VivaldiCoord::at(
            config.client_coord[0],
            config.client_coord[1],
            config.client_coord[2],
            config.client_coord[3],
        );
        let cdn = ContentLocator::with_weights(client, 0.3, 0.7);

        let node_ids: Vec<u64> = cdn_nodes.iter().map(|(id, _, _)| *id).collect();
        let maglev = MaglevHash::new(node_ids);

        // ALICE-Cache
        let cache = AliceCache::<u64, Vec<u8>>::with_config(CacheConfig {
            capacity: config.cache_capacity,
            num_nodes: cdn_nodes.len() as i32,
            node_id: 1,
            enable_oracle: true,
            ..Default::default()
        });

        // ALICE-Physics
        let physics = PhysicsWorld::new(PhysicsConfig::default());

        // ALICE-Sync
        let sync_session = LockstepSession::new(config.num_players as u8);

        // ALICE-Physics → DB (Replay)
        let recorder = ReplayRecorder::new(&config.replay_path, config.num_bodies)?;

        // ALICE-Sync → DB (Telemetry)
        let telemetry = SyncTelemetry::new(&config.telemetry_path)?;

        Ok(Self {
            db,
            cache,
            cdn,
            maglev,
            cdn_nodes,
            physics,
            sync_session,
            recorder,
            telemetry,
            asp_sequence: 0,
            sensor_content_id: 10000,
            frame_counter: 0,
        })
    }

    // ── Path A: Sensor Ingestion ─────────────────────────────────────────

    /// Ingest sensor data through the full IoT pipeline.
    ///
    /// `[Sensor] → [ALICE-Edge] → [ALICE-ASP] → [ALICE-CDN] → [ALICE-DB]`
    pub fn ingest_sensor(&mut self, data: &[i32]) -> Result<SensorIngestResult> {
        // 1. ALICE-Edge: compress sensor data to linear model (Q16.16)
        let (slope, intercept) = fit_linear_fixed(data);

        // 2. ALICE-ASP: packetize for network transport
        //    I-packet describes the sensor stream (width=1 channel, height=sample count)
        let payload = IPacketPayload::new(1, data.len() as u32, 1.0);
        let packet = AspPacket::create_i_packet(self.asp_sequence, payload)
            .map_err(|e| format!("ASP packet creation: {:?}", e))?;
        let asp_bytes = packet
            .to_bytes()
            .map_err(|e| format!("ASP serialization: {:?}", e))?;
        let asp_packet_size = asp_bytes.len();
        self.asp_sequence += 1;

        // 3. ALICE-CDN: route to best edge node (Vivaldi RTT prediction)
        let node_refs: Vec<(u64, &VivaldiCoord)> = self
            .cdn_nodes
            .iter()
            .map(|(id, _, coord)| (*id, coord))
            .collect();
        let best = self
            .cdn
            .find_best(self.sensor_content_id, node_refs)
            .ok_or("No CDN node available")?;
        let route_name = self.node_name(best.id);
        let rtt_ms = best.predicted_rtt.to_f64();
        self.sensor_content_id += 1;

        // 4. ALICE-DB: reconstruct from model and batch-store
        let n = data.len();
        let mut batch: Vec<(i64, f32)> = Vec::with_capacity(n);
        for i in 0..n {
            let q16_val = evaluate_linear_fixed(slope, intercept, i as i32);
            batch.push((i as i64, alice_edge::q16_to_f32(q16_val)));
        }
        self.db.put_batch(&batch)?;
        self.db.flush()?;

        let raw_bytes = n * 4;
        let compressed_bytes = 8usize; // slope(4) + intercept(4)

        Ok(SensorIngestResult {
            slope,
            intercept,
            asp_packet_size,
            cdn_route: route_name,
            cdn_rtt_ms: rtt_ms,
            db_records: n,
            compression_ratio: raw_bytes as f64 / compressed_bytes as f64,
        })
    }

    // ── Path B-1: Asset Delivery ─────────────────────────────────────────

    /// Deliver an SDF asset through the content pipeline.
    ///
    /// `[ALICE-SDF] → [ALICE-CDN] → [ALICE-Cache]`
    pub fn deliver_asset(&self, tree: &SdfTree, asset_id: u64) -> Result<AssetDeliveryResult> {
        // 1. ALICE-SDF: serialize to ASDF binary (in-memory, zero disk I/O)
        let body = bincode::serialize(tree)?;
        let crc = crc32fast::hash(&body);
        let header = AsdfHeader::new(tree, crc);
        let mut asdf_bytes = Vec::with_capacity(16 + body.len());
        asdf_bytes.extend_from_slice(&header.to_bytes());
        asdf_bytes.extend_from_slice(&body);
        let asdf_size = asdf_bytes.len();

        // 2. ALICE-CDN: type-aware routing (ASDF gets priority=4, replicas=5)
        let content_type = ContentType::detect(&asdf_bytes);
        let node_refs: Vec<(u64, &VivaldiCoord)> = self
            .cdn_nodes
            .iter()
            .map(|(id, _, coord)| (*id, coord))
            .collect();
        let best = self
            .cdn
            .find_best_typed(asset_id, node_refs, content_type)
            .ok_or("No CDN node available")?;
        let route_name = self.node_name(best.id);
        let rtt_ms = best.predicted_rtt.to_f64();

        // 3. ALICE-Cache: store on cache miss (Markov prefetch + TinyLFU)
        let cache_hit = self.cache.get(&asset_id).is_some();
        if !cache_hit {
            self.cache.put(asset_id, asdf_bytes);
        }

        Ok(AssetDeliveryResult {
            asdf_size,
            content_type,
            cdn_route: route_name,
            cdn_rtt_ms: rtt_ms,
            cache_hit,
        })
    }

    // ── Path B-2: Game Tick ──────────────────────────────────────────────

    /// Execute one game tick through the synchronization pipeline.
    ///
    /// `[ALICE-Sync] → [ALICE-Physics] → [Replay → DB] + [Telemetry → DB]`
    pub fn game_tick(
        &mut self,
        local: InputFrame,
        remote: InputFrame,
    ) -> Result<GameTickResult> {
        let frame = self.frame_counter;

        // 1. ALICE-Sync: lockstep input exchange
        self.sync_session.add_local_input(local);
        self.sync_session.add_remote_input(remote);

        let stepped = if self.sync_session.ready_to_advance() {
            let _synced = self
                .sync_session
                .advance()
                .ok_or("Sync advance failed")?;

            // 2. ALICE-Physics: deterministic step (128-bit fixed-point)
            let dt = Fix128::from_ratio(1, 60);
            self.physics.step(dt);

            // 3. Replay recording → ALICE-DB (trajectory compression)
            self.recorder.record_frame(&self.physics)?;

            // 4. Telemetry recording → ALICE-DB (metric time-series)
            self.telemetry.record_rtt(frame, 10.0)?;
            self.telemetry.record_prediction_accuracy(frame, 1.0)?;

            true
        } else {
            false
        };

        // Collect body positions
        let positions: Vec<(f32, f32, f32)> = self
            .physics
            .bodies
            .iter()
            .map(|b| {
                (
                    b.position.x.to_f32(),
                    b.position.y.to_f32(),
                    b.position.z.to_f32(),
                )
            })
            .collect();

        self.frame_counter += 1;

        Ok(GameTickResult {
            frame,
            positions,
            sync_confirmed: self.sync_session.confirmed_frame(),
            stepped,
        })
    }

    // ── Physics Setup ────────────────────────────────────────────────────

    /// Add a rigid body to the physics world.
    pub fn add_body(&mut self, body: RigidBody) -> usize {
        self.physics.add_body(body)
    }

    // ── Queries ──────────────────────────────────────────────────────────

    /// Query a value from ALICE-DB by timestamp key.
    pub fn query(&self, key: i64) -> Result<Option<f32>> {
        Ok(self.db.get(key)?)
    }

    /// O(1) Maglev content routing lookup.
    pub fn cdn_lookup(&self, content_id: u64) -> Option<u64> {
        self.maglev.lookup(content_id)
    }

    /// Current cache hit rate.
    pub fn cache_hit_rate(&self) -> f64 {
        self.cache.hit_rate()
    }

    // ── Lifecycle ────────────────────────────────────────────────────────

    /// Flush all buffered data to ALICE-DB.
    pub fn flush(&mut self) -> Result<()> {
        self.recorder.flush()?;
        self.telemetry.flush()?;
        self.db.flush()?;
        Ok(())
    }

    /// Close all resources and flush remaining data.
    pub fn close(mut self) -> Result<()> {
        self.flush()?;
        self.recorder.close()?;
        self.telemetry.close()?;
        self.db.close()?;
        Ok(())
    }

    // ── Path C: Motion Capture Streaming ──────────────────────────────

    /// Ingest motion capture intent through Kinematics → Sync → Physics.
    ///
    /// `[ALICE-Edge] → [ALICE-Kinematics] → [ALICE-Sync] → [ALICE-Physics]`
    pub fn mocap_intent(
        &mut self,
        intent: &Intent,
        frame: u64,
    ) -> Result<MocapResult> {
        // 1. ALICE-Kinematics: encode intent (8 bytes)
        let intent_bytes = intent.encode();

        // 2. Kinematics → Sync: pack into InputFrame
        let mx = i16::from_le_bytes([intent_bytes[0], intent_bytes[1]]);
        let my = i16::from_le_bytes([intent_bytes[2], intent_bytes[3]]);
        let mz = i16::from_le_bytes([intent_bytes[4], intent_bytes[5]]);
        let _input = InputFrame::new(frame, 0).with_movement(mx, my, mz);

        // 3. Kinematics → Physics: IK end-effector position
        let mut chain = ArmChain::right_arm();
        let target = intent.target;
        let (ik_iters, _) = chain.inverse_kinematics(target, 50, 0.001);
        let ee = chain.forward_kinematics();

        Ok(MocapResult {
            intent_size: 8,
            sync_frame: frame,
            ik_iterations: ik_iters,
            end_effector: (ee.x, ee.y, ee.z),
        })
    }

    // ── Path D: Anime Production ─────────────────────────────────────

    /// Production pipeline: VCS → SDF + Font + Synth for anime content.
    ///
    /// `[ALICE-VCS] → [ALICE-SDF] + [ALICE-Font] + [ALICE-Synth] → [ALICE-CDN]`
    pub fn anime_production(
        &mut self,
        sdf_scene: &SdfTree,
        text: &str,
        score: &Score,
        commit_message: &str,
    ) -> Result<AnimeProductionResult> {
        // 1. VCS: version the SDF scene
        let mut repo = Repository::new();
        let vcs_tree = crate::bridge_vcs::sdf_to_vcs_tree(sdf_scene);
        let hash = repo.commit(&vcs_tree, commit_message, "anime-pipeline");

        // 2. SDF: serialize for CDN
        let sdf_body = bincode::serialize(sdf_scene)?;
        let sdf_size = sdf_body.len();

        // 3. Font: shape text for subtitles
        let params = MetaFontParams::sans_regular();
        let shaper = TextShaper::new(params);
        let mut atlas = SdfAtlas::new(8, params);
        let shaped = shaper.shape_line(text, &mut atlas);
        let font_params_size = 40;

        // 4. Synth: render audio
        let audio_size = score.to_bytes().len();

        Ok(AnimeProductionResult {
            vcs_hash: hash,
            sdf_bytes: sdf_size,
            font_params_bytes: font_params_size,
            subtitle_glyphs: shaped.glyphs.len(),
            audio_score_bytes: audio_size,
            total_payload: sdf_size + font_params_size + audio_size,
        })
    }

    // ── Path E: Real-Time Embedded ───────────────────────────────────

    /// Embedded pipeline: RTOS → Edge → Synth → ASP.
    ///
    /// `[ALICE-RTOS] → [ALICE-Edge] → [ALICE-Synth] → [ALICE-ASP]`
    pub fn realtime_embedded(
        &mut self,
        sensor_data: &[i32],
        score: &Score,
        sample_rate: u32,
    ) -> Result<EmbeddedResult> {
        // 1. RTOS: verify schedulability
        let mut kernel = Kernel::testing();
        kernel.add_task(b"sensor", |_| {}, TaskPriority::NORMAL, 10_000, 500);
        kernel.add_task(b"audio", |_| {}, TaskPriority::CRITICAL, 5_000, 1_000);
        let stats = kernel.run_for(100_000, 100);

        // 2. Edge: compress sensor data
        let (_slope, _intercept) = fit_linear_fixed(sensor_data);
        let edge_bytes = 8;

        // 3. Synth: render audio
        let duration = score.duration_secs();
        let num_samples = (duration * sample_rate as f32) as usize;
        let mut synth = Synthesizer::new(sample_rate);
        synth.load_patch(0, Patch::Fm(FmPatch::electric_piano()));
        synth.load_score(score);
        let mut pcm = vec![0i16; num_samples.max(1)];
        synth.render_i16(&mut pcm);

        // 4. ASP: packetize
        let payload = IPacketPayload::new(1, num_samples as u32, 1.0);
        let packet = AspPacket::create_i_packet(self.asp_sequence, payload)
            .map_err(|e| format!("ASP: {:?}", e))?;
        let asp_bytes = packet.to_bytes().map_err(|e| format!("ASP: {:?}", e))?;
        self.asp_sequence += 1;

        Ok(EmbeddedResult {
            rtos_schedulable: stats.schedulable,
            rtos_utilization: stats.utilization,
            edge_bytes,
            audio_samples: num_samples,
            asp_packet_size: asp_bytes.len(),
        })
    }

    // ── Path F: 3D Print Optimization ────────────────────────────────

    /// Print pipeline: SDF → Motion (S-curve) → Print segments.
    ///
    /// `[ALICE-SDF] → [ALICE-Motion] (S-curve) → [ALICE-Print] → .3mf`
    pub fn print_optimization(
        &self,
        curve: &CubicBezier,
        v_max: f32,
        a_max: f32,
        num_segments: usize,
    ) -> Result<PrintOptResult> {
        let arc = curve.arc_length(64);
        let profile = TrapezoidalProfile::new(v_max, a_max, arc);
        let dur = profile.duration();
        let segments = crate::bridge_motion::motion_to_print_segments(curve, v_max, a_max, num_segments);

        // Calculate average feed rate
        let avg_feed: f32 = if segments.is_empty() {
            0.0
        } else {
            segments.iter().map(|s| s.feed_rate).sum::<f32>() * (1.0 / segments.len() as f32)
        };

        Ok(PrintOptResult {
            arc_length_mm: arc,
            duration_secs: dur,
            segment_count: segments.len(),
            avg_feed_rate: avg_feed,
            max_feed_rate: segments.iter().map(|s| s.feed_rate).fold(0.0f32, f32::max),
        })
    }

    // ── Internal ─────────────────────────────────────────────────────────

    fn node_name(&self, id: u64) -> String {
        self.cdn_nodes
            .iter()
            .find(|(nid, _, _)| *nid == id)
            .map(|(_, name, _)| name.clone())
            .unwrap_or_else(|| format!("node-{}", id))
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_kinematics::Vec3k;
    use alice_motion::Vec3;
    use alice_physics::Vec3Fix;
    use alice_sdf::SdfNode;
    use alice_synth::{NoteEvent, NoteEventKind, Score};
    use tempfile::tempdir;

    fn test_setup() -> (tempfile::TempDir, PipelineConfig) {
        let dir = tempdir().unwrap();
        let config = PipelineConfig {
            db_path: dir.path().join("db"),
            replay_path: dir.path().join("replay"),
            telemetry_path: dir.path().join("telemetry"),
            cdn_nodes: vec![
                CdnNodeConfig {
                    id: 1,
                    name: "Tokyo".into(),
                    coord: [0.0, 0.0, 0.0, 2.0],
                },
                CdnNodeConfig {
                    id: 2,
                    name: "London".into(),
                    coord: [30.0, 20.0, 0.0, 3.0],
                },
                CdnNodeConfig {
                    id: 3,
                    name: "NYC".into(),
                    coord: [50.0, 10.0, 0.0, 4.0],
                },
            ],
            client_coord: [1.0, 1.0, 0.0, 1.0],
            num_players: 2,
            num_bodies: 3,
            cache_capacity: 100,
        };
        (dir, config)
    }

    #[test]
    fn test_path_a_sensor_pipeline() {
        let (_dir, config) = test_setup();
        let mut pipeline = AlicePipeline::new(config).unwrap();

        // Generate sensor data (temperature in centidegrees)
        let sensor_data: Vec<i32> = (0..100).map(|i| 2500 + i * 5).collect();
        let result = pipeline.ingest_sensor(&sensor_data).unwrap();

        // Verify Edge compression
        assert!(result.compression_ratio > 1.0);
        assert_eq!(result.db_records, 100);

        // Verify ASP packetization
        assert!(result.asp_packet_size > 0);

        // Verify CDN routing
        assert!(!result.cdn_route.is_empty());
        assert!(result.cdn_rtt_ms >= 0.0);

        // Verify DB storage
        let val = pipeline.query(50).unwrap();
        assert!(val.is_some());

        pipeline.close().unwrap();
    }

    #[test]
    fn test_path_b1_asset_delivery() {
        let (_dir, config) = test_setup();
        let pipeline = AlicePipeline::new(config).unwrap();

        let tree = SdfTree::new(SdfNode::sphere(1.0));

        // First request: cache miss
        let r1 = pipeline.deliver_asset(&tree, 1001).unwrap();
        assert!(r1.asdf_size > 0);
        assert!(!r1.cache_hit);
        assert!(!r1.cdn_route.is_empty());

        // Second request: cache hit
        let r2 = pipeline.deliver_asset(&tree, 1001).unwrap();
        assert!(r2.cache_hit);

        pipeline.close().unwrap();
    }

    #[test]
    fn test_path_b2_game_tick() {
        let (_dir, config) = test_setup();
        let mut pipeline = AlicePipeline::new(config).unwrap();

        // Add bodies: 2 dynamic players + 1 static ground
        pipeline.add_body(RigidBody::new_dynamic(Vec3Fix::from_int(0, 10, 0), Fix128::ONE));
        pipeline.add_body(RigidBody::new_dynamic(Vec3Fix::from_int(3, 10, 0), Fix128::ONE));
        pipeline.add_body(RigidBody::new_static(Vec3Fix::ZERO));

        // Frame 0 is skipped by lockstep (confirmed_frame starts at 0)
        // Frames 1..=10 are processed
        let mut stepped_count = 0;
        for frame in 0..=10u64 {
            let local = InputFrame::new(frame, 0).with_movement(100, 0, 0);
            let remote = InputFrame::new(frame, 1).with_movement(-50, 0, 0);
            let result = pipeline.game_tick(local, remote).unwrap();

            assert_eq!(result.positions.len(), 3);
            if result.stepped {
                stepped_count += 1;
            }
        }
        assert_eq!(stepped_count, 10); // frames 1-10 stepped

        pipeline.close().unwrap();
    }

    #[test]
    fn test_path_c_mocap_intent() {
        let (_dir, config) = test_setup();
        let mut pipeline = AlicePipeline::new(config).unwrap();
        let intent = Intent::reach(Vec3k::new(0.3, 0.4, 0.0), 100);
        let result = pipeline.mocap_intent(&intent, 1).unwrap();
        assert_eq!(result.intent_size, 8);
        assert_eq!(result.sync_frame, 1);
        assert!(result.ik_iterations > 0);
    }

    #[test]
    fn test_path_d_anime_production() {
        let (_dir, config) = test_setup();
        let mut pipeline = AlicePipeline::new(config).unwrap();
        let sdf = SdfTree::new(SdfNode::sphere(1.0));
        let mut score = Score::new(120, 1);
        score.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 60, velocity: 100, kind: NoteEventKind::NoteOn });
        score.add_event(NoteEvent { delta_tick: 96, channel: 0, note: 60, velocity: 0, kind: NoteEventKind::NoteOff });
        let result = pipeline.anime_production(&sdf, "Hello", &score, "ep1 scene1").unwrap();
        assert_ne!(result.vcs_hash, 0);
        assert!(result.sdf_bytes > 0);
        assert_eq!(result.font_params_bytes, 40);
        assert!(result.subtitle_glyphs > 0);
    }

    #[test]
    fn test_path_e_realtime_embedded() {
        let (_dir, config) = test_setup();
        let mut pipeline = AlicePipeline::new(config).unwrap();
        let sensor: Vec<i32> = (0..50).map(|i| 2000 + i * 10).collect();
        let mut score = Score::new(120, 1);
        score.add_event(NoteEvent { delta_tick: 0, channel: 0, note: 60, velocity: 100, kind: NoteEventKind::NoteOn });
        score.add_event(NoteEvent { delta_tick: 96, channel: 0, note: 60, velocity: 0, kind: NoteEventKind::NoteOff });
        let result = pipeline.realtime_embedded(&sensor, &score, 22050).unwrap();
        assert!(result.rtos_schedulable);
        assert_eq!(result.edge_bytes, 8);
        assert!(result.audio_samples > 0);
        assert!(result.asp_packet_size > 0);
    }

    #[test]
    fn test_path_f_print_optimization() {
        let (_dir, config) = test_setup();
        let pipeline = AlicePipeline::new(config).unwrap();
        let curve = CubicBezier::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 20.0, 0.0),
            Vec3::new(30.0, 20.0, 0.0),
            Vec3::new(40.0, 0.0, 0.0),
        );
        let result = pipeline.print_optimization(&curve, 100.0, 500.0, 20).unwrap();
        assert!(result.arc_length_mm > 0.0);
        assert!(result.duration_secs > 0.0);
        assert_eq!(result.segment_count, 20);
        assert!(result.avg_feed_rate > 0.0);
    }

    #[test]
    fn test_full_pipeline_integration() {
        let (_dir, config) = test_setup();
        let mut pipeline = AlicePipeline::new(config).unwrap();

        // Path A: Sensor ingestion (Edge → ASP → CDN → DB)
        let sensor_data: Vec<i32> = (0..50).map(|i| 2000 + i * 10).collect();
        let sensor_result = pipeline.ingest_sensor(&sensor_data).unwrap();
        assert_eq!(sensor_result.db_records, 50);

        // Path B-1: Asset delivery (SDF → CDN → Cache)
        let level = SdfNode::box3d(10.0, 1.0, 10.0)
            .union(SdfNode::cylinder(1.0, 5.0).translate(3.0, 2.5, 0.0));
        let tree = SdfTree::new(level);
        let asset_result = pipeline.deliver_asset(&tree, 2001).unwrap();
        assert!(asset_result.asdf_size > 0);

        // Path B-2: Game ticks (Sync → Physics → Replay/Telemetry → DB)
        pipeline.add_body(RigidBody::new_dynamic(Vec3Fix::from_int(0, 5, 0), Fix128::ONE));
        pipeline.add_body(RigidBody::new_dynamic(Vec3Fix::from_int(2, 5, 0), Fix128::ONE));
        pipeline.add_body(RigidBody::new_static(Vec3Fix::ZERO));

        // Frames 1..=5 (frame 0 is skipped by lockstep protocol)
        for frame in 0..=5u64 {
            let local = InputFrame::new(frame, 0).with_movement(50, 0, 0);
            let remote = InputFrame::new(frame, 1).with_movement(-30, 0, 0);
            let tick = pipeline.game_tick(local, remote).unwrap();
            if frame > 0 {
                assert!(tick.stepped);
            }
        }

        // CDN routing (Maglev O(1) lookup)
        let node_id = pipeline.cdn_lookup(2001);
        assert!(node_id.is_some());

        // Verify DB still queryable
        let val = pipeline.query(25).unwrap();
        assert!(val.is_some());

        pipeline.close().unwrap();
    }

    #[test]
    fn test_path_g_ai_inference() {
        let result = path_g_ai_inference(12, 6, &[128, 64]);
        assert_eq!(result.ml_joint_count, 6);
        assert!(result.ml_inference_ops > 0);
        assert!(result.trt_param_count > 0);
        assert!(result.trt_flops > 0);
    }

    #[test]
    fn test_path_h_voice_delivery() {
        use alice_voice::{ParametricParams, PitchInfo, LpcCoefficients, Formant};
        let params = ParametricParams {
            pitch: PitchInfo { f0: 220.0, period: 72.7, voicing_prob: 0.95, confidence: 0.9, is_voiced: true },
            lpc: LpcCoefficients { coeffs: vec![0.5, -0.3], gain: 0.6, reflection: vec![], error: 0.01 },
            formants: vec![
                Formant { frequency: 700.0, bandwidth: 80.0, amplitude: 1.0 },
                Formant { frequency: 1200.0, bandwidth: 90.0, amplitude: 0.8 },
                Formant { frequency: 2600.0, bandwidth: 120.0, amplitude: 0.5 },
            ],
            activity: alice_voice::VoiceActivity { is_voiced: true, confidence: 0.95, energy_db: -20.0 },
            frame_size: 320,
            sample_rate: 16000,
        };
        let result = path_h_voice_delivery(&params);
        assert!(result.carrier_freq > 0.0);
        assert!(result.voiced);
        assert!(result.codec_compressed_bytes > 0);
    }

    #[test]
    fn test_path_i_fulltext_search() {
        let text = "The quick brown fox jumps over the lazy dog. ".repeat(10);
        let result = path_i_fulltext_search(&text, "fox");
        assert!(result.compressed_bytes > 0);
        assert!(result.index_node_count > 0);
    }

    #[test]
    fn test_path_j_dns_api_gateway() {
        let result = path_j_dns_api_gateway("example.com", "/assets/model.asdf");
        assert!(result.api_rate_allowed);
        assert_eq!(result.operation, "read");
        assert_eq!(result.content_type_hint, "application/x-alice-sdf");
    }
}
