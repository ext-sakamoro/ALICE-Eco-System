//! ALICE Eco-System Pipeline — Edge to DB
//!
//! Connects all 9 ALICE crates into a unified write pipeline:
//!
//! - **Path A** (IoT/Sensor): Edge → ASP → CDN → DB
//! - **Path B-1** (Asset Delivery): SDF → CDN → Cache
//! - **Path B-2** (Game Loop): Sync → Physics → Replay/Telemetry → DB
//!
//! All paths terminate at ALICE-DB (model-based compression).

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
    use alice_physics::Vec3Fix;
    use alice_sdf::SdfNode;
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
}
