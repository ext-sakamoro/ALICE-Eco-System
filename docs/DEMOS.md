# Pipeline demos

This repository contains three runnable programs that connect several ALICE
crates into one pipeline. They are integration demonstrations, not
benchmarks: they print what each stage produced so the hand-off between
crates can be followed.

## Setting up

`Cargo.toml` refers to the crates by sibling path, so the repositories have to
be cloned next to this one:

```sh
mkdir alice && cd alice
for r in ALICE-Eco-System ALICE-Edge ALICE-DB ALICE-View ALICE-Streaming-Protocol \
         ALICE-SDF ALICE-CDN ALICE-Cache ALICE-Physics ALICE-Sync; do
  git clone "https://github.com/ext-sakamoro/$r.git"
done
cd ALICE-Eco-System
```

The full `Cargo.toml` also lists bridge targets that are not public, so a
build of the whole hub crate needs those paths removed first. The demos below
only use the public crates listed with them.

## Edge-to-database pipeline (`cargo run`)

Source: [src/main.rs](../src/main.rs). Crates: ALICE-Edge, ALICE-DB,
ALICE-View.

```text
[sensor sample generator] → [ALICE-Edge: fit a model on the stack]
    → [network: send model coefficients] → [ALICE-DB: batch write] → [ALICE-View]
```

A generator produces synthetic temperature samples. ALICE-Edge fits a linear
model to each window in fixed point, only the model coefficients cross the
"network", and ALICE-DB stores them and answers aggregation queries. Passing
`--view` opens the ALICE-View window on the stored series.

## SDF asset delivery (`cargo run --example sdf_delivery`)

Source: [examples/sdf_delivery.rs](../examples/sdf_delivery.rs). Crates:
ALICE-SDF, ALICE-CDN, ALICE-Cache.

```text
client request (asset id)
    → ALICE-CDN   nearest edge node by Vivaldi coordinates, Maglev assignment
    → ALICE-Cache Markov prefetch, TinyLFU eviction
    → ALICE-SDF   on a cache miss, the asset is served from the origin as an ASDF binary
```

Assets are SDF trees serialized in the ASDF binary format instead of meshes;
the demo routes requests through the CDN, fills the cache and reports the
cache hit rate.

## Game engine pipeline (`cargo run --example game_pipeline`)

Source: [examples/game_pipeline.rs](../examples/game_pipeline.rs). Crates:
ALICE-SDF, ALICE-CDN, ALICE-Physics, ALICE-Sync, ALICE-DB.

```text
ALICE-SDF      world geometry as an ASDF binary
ALICE-CDN      content routing by detected content type
ALICE-Physics  deterministic simulation on 128-bit fixed point
ALICE-Sync     input synchronization (lockstep, 2 players)
ALICE-DB       replay recording and telemetry
```

The world geometry is delivered as SDF, simulated deterministically, kept in
step between two players by exchanging inputs, recorded into ALICE-DB, and
played back from the recording.
