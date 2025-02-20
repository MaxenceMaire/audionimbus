# audionimbus

A Rust wrapper around [Steam Audio](https://valvesoftware.github.io/steam-audio/) that provides spatial audio capabilities with realistic occlusion, reflection, reverb, and HRTF effects, accounting for physical attributes and scene geometry.

## Overview

`audionimbus` simplifies the integration of Steam Audio into Rust projects by offering a safe, idiomiatic API.
It builds upon [`audionimbus-sys`](../audionimbus-sys), which provides raw bindings to the Steam Audio C API.

## Version compatibility

`audionimbus` currently tracks Steam Audio 4.6.0.

Unlike `audionimbus-sys`, which mirrors Steam Audio's versioning, `audionimbus` introduces its own abstractions and is subject to breaking changes.
As a result, it uses independent versioning.

## Installation

`audionimbus` requires linking against the Steam Audio library during compilation.

To do so, download `steamaudio_4.6.0.zip` from the [release page](https://github.com/ValveSoftware/steam-audio/releases).

Locate the relevant library for your target platform (`SDKROOT` refers to the directory in which you extracted the zip file):

| Platform | Library Directory | Library To Link |
| --- | --- | --- |
| Windows 32-bit | `SDKROOT/lib/windows-x86` | `phonon.dll` |
| Windows 64-bit | `SDKROOT/lib/windows-x64` | `phonon.dll` |
| Linux 32-bit | `SDKROOT/lib/linux-x86` | `libphonon.so` |
| Linux 64-bit | `SDKROOT/lib/linux-x64` | `libphonon.so` |
| macOS | `SDKROOT/lib/osx` | `libphonon.dylib` |
| Android ARMv7 | `SDKROOT/lib/android-armv7` | `libphonon.so` |
| Android ARMv8/AArch64 | `SDKROOT/lib/android-armv8` | `libphonon.so` |
| Android x86 | `SDKROOT/lib/android-x86` | `libphonon.so` |
| Android x64 | `SDKROOT/lib/android-x64` | `libphonon.so` |
| iOS ARMv8/AArch64 | `SDKROOT/lib/ios` | `libphonon.a` |

Ensure the library is placed in a location listed in the [dynamic library search paths](https://doc.rust-lang.org/cargo/reference/environment-variables.html#dynamic-library-paths) (e.g., `/usr/local/lib`).

Finally, add `audionimbus` to your `Cargo.toml`:

```
[dependencies]
audionimbus = "0.1.0"
```

## Example

This example demonstrates how to spatialize sound using the `audionimbus` library:

```rust
use audionimbus::*;

// Initialize the audio context.
let context = Context::try_new(&ContextSettings::default()).unwrap();

let audio_settings = AudioSettings {
    sampling_rate: 48000,
    frame_size: 1024,
};

// Set up HRTF for binaural rendering.
let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

// Create a binaural effect.
let binaural_effect = BinauralEffect::try_new(
    &context,
    &audio_settings,
    &BinauralEffectSettings { hrtf: &hrtf },
)
.unwrap();

// Generate an input frame (in thise case, a single-channel sine wave).
let input: Vec<Sample> = (0..audio_settings.frame_size)
    .map(|i| {
        (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / audio_settings.sampling_rate as f32)
            .sin()
    })
    .collect();
// Create an audio buffer over the input data.
let input_buffer = AudioBuffer::try_with_data(&input).unwrap();

let num_channels: usize = 2; // Stereo
// Allocate memory to store processed samples.
let mut output = vec![0.0; audio_settings.frame_size * num_channels];
// Create another audio buffer over the output container.
let output_buffer = AudioBuffer::try_with_data_and_settings(
    &mut output,
    &AudioBufferSettings {
        num_channels: Some(num_channels),
        ..Default::default()
    },
)
.unwrap();

// Apply a binaural audio effect.
let binaural_effect_params = BinauralEffectParams {
    direction: Direction::new(
        1.0, // Right
        0.0, // Up
        0.0, // Behind
    ),
    interpolation: HrtfInterpolation::Nearest,
    spatial_blend: 1.0,
    hrtf: &hrtf,
    peak_delays: None,
};
let _effect_state =
    binaural_effect.apply(&binaural_effect_params, &input_buffer, &output_buffer);

// `output` now contains the processed samples in a deinterleaved format (i.e., left channel
// samples followed by right channel samples).

// Note: most audio engines expect interleaved audio (alternating samples for each channel). If
// required, use the `AudioBuffer::interleave` method to convert the format.
```

To implement real-time audio processing and playback in your game, check out the [demo crate](demo) for a more comprehensive example.

For additional examples, you can explore the [tests](tests), which closely follow [Steam Audio's Programmer's Guide](https://valvesoftware.github.io/steam-audio/doc/capi/guide.html).

## Documentation

Documentation is available at [doc.rs](https://docs.rs/audionimbus/latest).

For more details on Steam Audio's concepts, see the [Steam Audio SDK documentation](https://valvesoftware.github.io/steam-audio/doc/capi/index.html).

## License

`audionimbus` is dual-licensed under the [MIT License](LICENSE-MIT) and the [Apache-2.0 License](LICENSE-APACHE).
You may choose either license when using the software.
