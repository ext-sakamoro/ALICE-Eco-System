# Pipeline demos

This repository contains three runnable programs that connect several ALICE
crates into one pipeline. They are integration demonstrations, not
benchmarks: they print what each stage produced so the hand-off between
crates can be followed.

## Building them

`Cargo.toml` refers to every bridged crate by sibling path (`../ALICE-*`), and
`src/lib.rs` compiles the bridge modules unconditionally; some of those
crates are not public. **The demos therefore do not build from public
checkouts alone at present.** Making them buildable needs the bridge modules
for non-public crates put behind optional features; until then this page
documents what each demo does, with links to its source.

## Edge-to-database pipeline (`cargo run`)

Source: [src/main.rs](../src/main.rs). Crates: ALICE-Edge, ALICE-DB,
ALICE-View (only for `--view`).

```text
[sensor sample generator] → [ALICE-Edge: fit a model on the stack]
    → [network: send model coefficients] → [ALICE-DB: batch write] → [ALICE-View]
```

A generator produces 1000 synthetic temperature samples. ALICE-Edge fits one
linear model to the whole buffer in fixed point (`fit_linear_fixed`), only the
two coefficients cross the "network", and ALICE-DB stores the reconstructed
series and answers average / minimum / maximum queries. Passing `--view`
afterwards opens an ALICE-View window with its own procedural scenes; it does
not display the stored series.

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
ALICE-SDF, ALICE-CDN, ALICE-Physics, ALICE-Sync (ALICE-DB is used through
the `replay` feature of ALICE-Physics).

```text
ALICE-SDF      world geometry as an ASDF binary
ALICE-CDN      content routing by detected content type
ALICE-Physics  deterministic simulation on 128-bit fixed point
ALICE-Sync     input synchronization (lockstep, 2 players)
ALICE-DB       replay recording, via ALICE-Physics `replay`
```

The world geometry is delivered as SDF, simulated deterministically, kept in
step between two players by exchanging inputs, recorded through
ALICE-Physics `replay` (stored with ALICE-DB), and played back from the
recording.
