/*!
# audionimbus-sys

Rust bindings to the [Steam Audio](https://valvesoftware.github.io/steam-audio/) library.
This crate is not meant to be used directly; most users should use [`audionimbus`](https://github.com/MaxenceMaire/audionimbus/tree/master/audionimbus), a safe wrapper built on top of `audionimbus-sys`.

## Overview

`audionimbus-sys` exposes raw bindings to the Steam Audio C library.
It is inherently unsafe, as it interfaces with external C code; for a safe API, refer to [`audionimbus`](https://github.com/MaxenceMaire/audionimbus/tree/master/audionimbus).

## Version compatibility

`audionimbus-sys` mirrors the version of Steam Audio.

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

## Documentation

Documentation is available at [docs.rs](https://docs.rs/audionimbus-sys/latest).

Since this crate strictly follows Steam Audio’s C API, you can also refer to the [Steam Audio C API reference](https://valvesoftware.github.io/steam-audio/doc/capi/reference.html) for additional details.

## License

`audionimbus-sys` is dual-licensed under the [MIT License](https://github.com/MaxenceMaire/audionimbus/blob/master/LICENSE-MIT) and the [Apache-2.0 License](https://github.com/MaxenceMaire/audionimbus/blob/master/LICENSE-APACHE).
You may choose either license when using the software.
*/

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
include!(concat!(env!("OUT_DIR"), "/phonon.rs"));
