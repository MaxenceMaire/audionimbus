# audionimbus-sys

Rust bindings to the [Steam Audio](https://valvesoftware.github.io/steam-audio/) library.
This crate is not meant to be used directly; most users should use [`audionimbus`](../audionimbus), a safe wrapper built on top of `audionimbus-sys`.

## Overview

`audionimbus-sys` exposes raw bindings to the [Steam Audio C library](steam-audio).
It is inherently unsafe, as it interfaces with external C code; for a safe API, refer to [`audionimbus`](../audionimbus).

`audionimbus-sys` can also integrate with FMOD studio.

## Version compatibility

`audionimbus-sys` mirrors the version of [Steam Audio](steam-audio).

## Installation

`audionimbus-sys` requires linking against the Steam Audio library during compilation.

To do so, download `steamaudio_4.6.1.zip` from the [release page](https://github.com/ValveSoftware/steam-audio/releases).

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

Finally, add `audionimbus-sys` to your `Cargo.toml`:

```toml
[dependencies]
audionimbus-sys = "4.6.1"
```

## Steam Audio FMOD Studio Integration

`audionimbus-sys` can be used to add spatial audio to an FMOD Studio project.

It requires linking against both the Steam Audio library and the FMOD integration library during compilation, similarly to the steps described in the [Installation](#Installation) section.

Download `steamaudio_fmod_4.6.1.zip` from the [release page](https://github.com/ValveSoftware/steam-audio/releases).

Locate the two relevant libraries for your target platform (`SDKROOT` refers to the directory in which you extracted the zip file):

| Platform | Library Directory | Library To Link |
| --- | --- | --- |
| Windows 32-bit | `SDKROOT/lib/windows-x86` | `phonon.dll`, `phonon_fmod.dll` |
| Windows 64-bit | `SDKROOT/lib/windows-x64` | `phonon.dll`, `phonon_fmod.dll` |
| Linux 32-bit | `SDKROOT/lib/linux-x86` | `libphonon.so`, `libphonon_fmod.so` |
| Linux 64-bit | `SDKROOT/lib/linux-x64` | `libphonon.so`, `libphonon_fmod.so` |
| macOS | `SDKROOT/lib/osx` | `libphonon.dylib`, `libphonon_fmod.dylib` |
| Android ARMv7 | `SDKROOT/lib/android-armv7` | `libphonon.so`, `libphonon_fmod.so` |
| Android ARMv8/AArch64 | `SDKROOT/lib/android-armv8` | `libphonon.so`, `libphonon_fmod.so` |
| Android x86 | `SDKROOT/lib/android-x86` | `libphonon.so`, `libphonon_fmod.so` |
| Android x64 | `SDKROOT/lib/android-x64` | `libphonon.so`, `libphonon_fmod.so` |
| iOS ARMv8/AArch64 | `SDKROOT/lib/ios` | `libphonon.a`, `libphonon_fmod.a` |

Ensure the libraries are placed in a location listed in the [dynamic library search paths](https://doc.rust-lang.org/cargo/reference/environment-variables.html#dynamic-library-paths) (e.g., `/usr/local/lib`).

Finally, add `audionimbus-sys` with the `fmod` feature enabled to your `Cargo.toml`:

```toml
[dependencies]
audionimbus-sys = { version = "4.6.2-fmod.1", features = ["fmod"] }
```

## Documentation

Documentation is available at [docs.rs](https://docs.rs/audionimbus-sys/latest).

Since this crate strictly follows Steam Audio’s C API, you can also refer to the [Steam Audio C API reference](https://valvesoftware.github.io/steam-audio/doc/capi/reference.html) for additional details.

## License

`audionimbus-sys` is dual-licensed under the [MIT License](LICENSE-MIT) and the [Apache-2.0 License](LICENSE-APACHE).
You may choose either license when using the software.
