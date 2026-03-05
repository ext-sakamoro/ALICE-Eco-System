//! ALICE-Eco-System ブリッジ変換ベンチマーク
//!
//! `cargo bench` で実行。主要ブリッジ変換とFNV-1aハッシュのスループットを計測。

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use alice_eco_system::hash::fnv1a;

// ── SDF → View ブリッジ ─────────────────────────────────────────────────
use alice_eco_system::bridge_sdf::sdf_to_view_descriptor;
use alice_sdf::{SdfNode, SdfTree};

fn bench_sdf_to_view(c: &mut Criterion) {
    let tree = SdfTree::new(SdfNode::sphere(1.0));
    c.bench_function("sdf_to_view_descriptor", |b| {
        b.iter(|| sdf_to_view_descriptor(black_box(&tree)));
    });
}

// ── Physics → DB ブリッジ ───────────────────────────────────────────────
use alice_eco_system::bridge_physics::physics_to_db_record;
use alice_physics::{Fix128, PhysicsConfig, RigidBody, Vec3Fix};

fn bench_physics_to_db(c: &mut Criterion) {
    let config = PhysicsConfig::default();
    let bodies: Vec<RigidBody> = (0..10)
        .map(|i| RigidBody::new_dynamic(Vec3Fix::from_int(i, 10, 0), Fix128::ONE))
        .collect();
    c.bench_function("physics_to_db_record", |b| {
        b.iter(|| physics_to_db_record(black_box(&bodies), black_box(0), black_box(&config)));
    });
}

// ── Crypto Key → Analytics ブリッジ ─────────────────────────────────────
use alice_crypto::Key;
use alice_eco_system::bridge_crypto::crypto_key_to_analytics;

fn bench_crypto_key_to_analytics(c: &mut Criterion) {
    let key = Key::generate().expect("key generation");
    c.bench_function("crypto_key_to_analytics", |b| {
        b.iter(|| crypto_key_to_analytics(black_box(&key), black_box(1_000_000_000)));
    });
}

// ── Edge → Analytics ブリッジ ────────────────────────────────────────────
use alice_eco_system::bridge_edge::edge_to_analytics_pipeline_metrics;

fn bench_edge_to_analytics(c: &mut Criterion) {
    let data: Vec<i32> = (0..100).map(|i| 2500 + i * 5).collect();
    c.bench_function("edge_to_analytics_pipeline_metrics", |b| {
        b.iter(|| edge_to_analytics_pipeline_metrics(black_box(&data), black_box(100_000)));
    });
}

// ── FNV-1a スループット ─────────────────────────────────────────────────
#[allow(clippy::similar_names)]
fn bench_fnv1a_throughput(c: &mut Criterion) {
    let data_16 = [0xABu8; 16];
    let data_256 = [0xCDu8; 256];
    let data_4k = vec![0xEFu8; 4096];

    let mut group = c.benchmark_group("fnv1a_throughput");
    group.bench_function("16_bytes", |b| b.iter(|| fnv1a(black_box(&data_16))));
    group.bench_function("256_bytes", |b| b.iter(|| fnv1a(black_box(&data_256))));
    group.bench_function("4096_bytes", |b| b.iter(|| fnv1a(black_box(&data_4k))));
    group.finish();
}

criterion_group!(
    benches,
    bench_sdf_to_view,
    bench_physics_to_db,
    bench_crypto_key_to_analytics,
    bench_edge_to_analytics,
    bench_fnv1a_throughput,
);
criterion_main!(benches);
