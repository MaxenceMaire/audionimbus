# Examples

This directory contains two demos that showcase different ways to use [`audionimbus`](..).

## Core demo

The [`core`](./core) demo uses the crate's core primitives and the `wiring` module.
Its parallel `static.rs` and `dynamic.rs` entry points show the same flow with each binding.
A source orbits the listener inside a reflective room, making it easy to hear the direct path, early reflections, and late reverb in isolation.

Run it with:

```bash
cargo run --release -p core-demo --bin static
```

For runtime configuration, run:

```bash
cargo run --release -p core-demo --bin dynamic
```

## Bevy demo

The [`bevy`](./bevy) demo shows the same ideas in a Bevy app.

It uses the `audionimbus::bevy` integration to mirror scene state from ECS into the simulation, then feeds the simulation output into a custom audio node for playback.

Run it with:

```bash
cargo run --release -p bevy-demo
```
