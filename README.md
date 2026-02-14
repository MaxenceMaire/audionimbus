# AudioNimbus

[![Crates.io](https://img.shields.io/crates/v/audionimbus.svg)](https://crates.io/crates/audionimbus)
[![Documentation](https://docs.rs/audionimbus/badge.svg)](https://docs.rs/audionimbus)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)

A Rust wrapper around [Steam Audio](https://valvesoftware.github.io/steam-audio/) that provides spatial audio capabilities for games and VR applications. It simulates realistic sound propagation, including physics-based occlusion, reflections, reverb, HRTF, and more.

## Features

* **Sound propagation**: Models how sound is affected as it travels through its environment. Includes effects like distance attenuation and interaction with physical obstacles of varying materials.
* **Reflections & reverb**: Simulates how sound waves reflect off surrounding geometry to create realistic acoustics.
* **Head-Related Transfer Function (HRTF)**: Simulates how the listener's head and ears shape incoming sound to convey direction and distance.
* **Ambisonics & surround sound**: Encodes spatial information across multiple audio channels to reproduce directional sound fields.

AudioNimbus can integrate with FMOD Studio and Wwise.

For a demonstration of AudioNimbus' capabilities, watch [the walkthrough video](https://www.youtube.com/watch?v=zlhW1maG0Is).

## Get Started

Add `audionimbus` to your dependencies:

```toml
[dependencies]
audionimbus = { version = "0.12.0", features = ["auto-install"] }
```

The `auto-install` feature automatically downloads and installs Steam Audio for you.

For more information, refer to the detailed [installation guide](audionimbus/README.md#installation).

To get started integrating `audionimbus` into your project, check out [the example](audionimbus/README.md#example) or run the [`demo`](audionimbus/demo).

## Documentation

Documentation is available at [docs.rs](https://docs.rs/audionimbus/latest).

## Project Structure

AudioNimbus consists of two crates:

* [`audionimbus`](audionimbus): A high-level, safe wrapper around Steam Audio. You typically only need to use this crate.
* [`audionimbus-sys`](audionimbus-sys): Automatically generated raw bindings to the Steam Audio C API. Used internally by `audionimbus`. You typically don't need to interact directly with this crate.

## License

This repository is dual-licensed under the [MIT License](LICENSE-MIT) and the [Apache-2.0 License](LICENSE-APACHE).
You may choose either license when using the software.
