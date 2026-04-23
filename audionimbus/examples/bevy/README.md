# Audionimbus Bevy Demo

This crate demonstrates how to integrate [`audionimbus`](../..) into a Bevy application.

## Running the demo

```bash
cargo run --release -p bevy-demo
```

## What you should hear

An orb circles the listener inside a large room, emitting a repeating tone burst.

As it moves, the direct sound is spatialized to binaural stereo via HRTF, with distance attenuation, air absorption, and occlusion applied on top.

You should also hear early reflections from the nearby walls, followed by a longer reverb tail that lingers during the pauses between bursts.

## What you should see

The demo enables the Bevy debug overlay for spatial audio geometry.

You can move around using the freecam controls:
- Mouse: Look around
- Scroll: Adjust movement speed
- Left click: Hold to grab cursor
- W & S: Forward/backward
- A & D: Strafe left/right
- E & Q: Up/down
- M: Toggle cursor grab

## Architecture

This demo uses the [`audionimbus::bevy`](https://docs.rs/audionimbus/latest/audionimbus/bevy/index.html) module to keep the simulation state in sync with the Bevy world.

The simulation itself runs asynchronously on dedicated threads.

The demo uses [Firewheel](https://github.com/BillyDM/firewheel) as the audio backend via the [`bevy_seedling`](https://github.com/corvusprudens/bevy_seedling) integration.
A custom node reads the latest direct, reflections, and reverb outputs, applies them to a generated tone burst, and mixes the result to stereo.

Bevy owns the scene and source transforms, the simulation threads produce spatial audio data in the background, and the audio graph consumes whichever simulation output is ready without blocking.
