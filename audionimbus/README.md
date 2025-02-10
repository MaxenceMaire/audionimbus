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

## Documentation

Documentation is available at [doc.rs](https://docs.rs/audionimbus/latest).

For more details on Steam Audio's concepts, see the [Steam Audio SDK documentation](https://valvesoftware.github.io/steam-audio/doc/capi/index.html).

## License

`audionimbus` is dual-licensed under the [MIT License](LICENSE-MIT) and the [Apache-2.0 License](LICENSE-APACHE).
You may choose either license when using the software.
