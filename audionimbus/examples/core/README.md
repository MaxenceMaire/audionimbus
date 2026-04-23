# Audionimbus Demo

This crate demonstrates how to get started with the [`audionimbus`](../..) library.

## Running the demo

```bash
cargo run --release -p core-demo
```

## What you should hear

A sound source orbits the listener inside a large, reflective room, emitting a repeating tone burst.

In this demo, audio gets spatialized using three effects:
- Direct path: distance attenuation, air absorption, occlusion, spatialized to binaural stereo via HRTF.
- Source reflections: early reflections from the walls nearest the source, arriving quickly after the direct sound. These give a sense of the source position in the room.
- Listener reverb: the late reverb tail, i.e. a slowly decaying wash of sound that lingers after the source goes quiet. This conveys room size.

The pause between bursts makes the reverb tail audible.

## Architecture

Spatial audio simulation is compute-intensive; a reflections pass typically takes 50-500ms.
Running it on the real-time audio thread would cause glitches since the audio callback only has a few milliseconds to produce each buffer.

In addition to core `audionimbus` primitives, this demo uses the high-level `wiring` module, which handles the complexity of spawning a dedicated thread per simulation type and keeping them in sync.

The game loop publishes new source positions to the simulation threads each frame, the simulation threads run asynchronously, and the audio thread renders audio using the latest available simulation output, without ever blocking. Each operates independently at its own pace.
