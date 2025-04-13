# audionimbus

A Rust wrapper around [Steam Audio](https://valvesoftware.github.io/steam-audio/), bringing powerful spatial audio capabilities to the Rust ecosystem.

## What is Steam Audio?

Steam Audio is a toolkit for spatial audio, developed by Valve. It simulates realistic sound propagation, including effects like directionality, distance attenuation, and reflections.

## What is AudioNimbus?

AudioNimbus provides a safe and ergonomic Rust interface to Steam Audio, enabling developers to integrate immersive spatial audio into their Rust projects. It consists of two crates:

* [`audionimbus`](audionimbus): A high-level, safe wrapper around Steam Audio.
* [`audionimbus-sys`](audionimbus-sys): Automatically generated raw bindings to the Steam Audio C API.

Both can integrate with FMOD Studio.

## Features

AudioNimbus supports a variety of spatial audio effects, including:

* **Head-Related Transfer Function (HRTF)**: Simulates how the listener’s ears, head, and shoulders shape sound perception, providing the acoustic cues the brain uses to infer direction and distance.
* **Ambisonics and surround sound**: Uses multiple audio channels to create the sensation of sound coming from specific directions.
* **Sound propagation**: Models how sound is affected as it travels through its environment, including effects like distance attenuation and interaction with physical obstacles of varying materials.
* **Reflections**: Simulates how sound waves reflect off surrounding geometry, mimicking real-world acoustic behavior.

For a demonstration of AudioNimbus' capabilities, watch [the walkthrough video](https://www.youtube.com/watch?v=zlhW1maG0Is).

## Get Started

To get started using [`audionimbus`](audionimbus), check out the [`demo`](audionimbus/demo) for a practical example of how to integrate and use the library in your project.

## License

This repository is dual-licensed under the [MIT License](LICENSE-MIT) and the [Apache-2.0 License](LICENSE-APACHE).
You may choose either license when using the software.
