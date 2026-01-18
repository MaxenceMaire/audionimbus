/*!
Rust bindings to the [Steam Audio](https://valvesoftware.github.io/steam-audio/) library.
This crate is not meant to be used directly; most users should use [`audionimbus`](https://github.com/MaxenceMaire/audionimbus/tree/master/audionimbus), a safe wrapper built on top of `audionimbus-sys`.

## Overview

`audionimbus-sys` exposes raw bindings to the Steam Audio C library.
It is inherently unsafe, as it interfaces with external C code; for a safe API, refer to [`audionimbus`](https://github.com/MaxenceMaire/audionimbus/tree/master/audionimbus).

`audionimbus-sys` can also integrate with FMOD and Wwise.

## Version compatibility

`audionimbus-sys` mirrors the version of Steam Audio.

## Installation

### Automatic Installation (Recommended)

The easiest way to use `audionimbus-sys` is with automatic installation. This will automatically download and set up the required Steam Audio libraries for your target platform.

#### Requirements

- **curl** or **wget** (for downloading)
- **Clang 9.0 or later**

#### Basic Usage

Add `audionimbus-sys` to your `Cargo.toml` with the `auto-install` feature:

```toml
[dependencies]
audionimbus-sys = { version = "4.8.0-rc.2", features = ["auto-install"] }
```

#### With FMOD Studio Integration

```toml
[dependencies]
audionimbus-sys = { version = "4.8.0-rc.2", features = ["auto-install", "fmod"] }
```

You also need to set the `FMODSDK` environment variable to the path of the FMOD SDK installed on your system (e.g. `export FMOD="/path/to/FMOD"`).

#### With Wwise Integration

```toml
[dependencies]
audionimbus-sys = { version = "4.8.0-rc.2", features = ["auto-install", "wwise"] }
```

You also need to set the `WWISESDK` environment variable to the path of the Wwise SDK installed on your system (e.g. `export WWISESDK="/path/to/Audiokinetic/Wwise2024.1.3.8749/SDK"`).

#### How It Works

When you build your project with the `auto-install` feature, the build script:

1. Automatically detects your target platform and architecture
2. Downloads the appropriate Steam Audio release zip file (cached to avoid re-downloading)
3. Extracts only the required shared libraries for your platform
4. Sets up the library search paths automatically

The downloaded files are cached in `$OUT_DIR/steam_audio_cache` and won't be re-downloaded unless the version changes.
If you need to force a re-download, you can delete this directory.

> **Note**
> The initial download can be quite large (≈180 MB for Steam Audio, ≈140 MB for the FMOD integration ≈52MB for the Wwise integration).
> During this step, cargo build may look like it is stuck - it’s just downloading in the background.
> The files are cached, so this only happens the first time (or when the version changes).

### Manual Installation

If you prefer manual installation or the automatic installation doesn't work for your setup, you can still install Steam Audio manually.

#### Requirements

Before installation, make sure that Clang 9.0 or later is installed on your system.

#### Steps

`audionimbus-sys` requires linking against the Steam Audio library during compilation.

To do so, download `steamaudio_4.8.0.zip` from the [release page](https://github.com/ValveSoftware/steam-audio/releases).

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
audionimbus-sys = "4.8.0-rc.2"
```

#### Manual FMOD Studio Integration

`audionimbus-sys` can be used to add spatial audio to an FMOD Studio project.

It requires linking against both the Steam Audio library and the FMOD integration library during compilation:

1. Download `steamaudio_fmod_4.8.0.zip` from the [release page](https://github.com/ValveSoftware/steam-audio/releases).

2. Locate the two relevant libraries for your target platform (`SDKROOT` refers to the directory in which you extracted the zip file):

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

3. Ensure the libraries are placed in a location listed in the [dynamic library search paths](https://doc.rust-lang.org/cargo/reference/environment-variables.html#dynamic-library-paths) (e.g., `/usr/local/lib`).

4. Set the `FMODSDK` environment variable to the path of the FMOD SDK installed on your system (e.g. `export FMOD="/path/to/FMOD"`).

5. Finally, add `audionimbus-sys` with the `fmod` feature enabled to your `Cargo.toml`:

```toml
[dependencies]
audionimbus-sys = { version = "4.8.0-rc.2", features = ["fmod"] }
```

#### Manual Wwise Integration

`audionimbus-sys` can be used to add spatial audio to a Wwise project.

It requires linking against both the Steam Audio library and the Wwise integration library during compilation:

1. Download `steamaudio_wwise_4.8.0.zip` from the [release page](https://github.com/ValveSoftware/steam-audio/releases).

2. Locate the two relevant libraries for your target platform and place them in a location listed in [dynamic library search paths](https://doc.rust-lang.org/cargo/reference/environment-variables.html#dynamic-library-paths) (e.g., `/usr/local/lib`).

3. Set the `WWISESDK` environment variable to the path of the Wwise SDK installed on your system (e.g. `export WWISESDK="/path/to/Audiokinetic/Wwise2024.1.3.8749/SDK"`).

4. Finally, add `audionimbus-sys` with the `wwise` feature enabled to your `Cargo.toml`:

```toml
[dependencies]
audionimbus-sys = { version = "4.8.0-rc.2", features = ["wwise"] }
```

## Documentation

Documentation is available at [docs.rs](https://docs.rs/audionimbus-sys/latest).

Since this crate strictly follows Steam Audio’s C API, you can also refer to the [Steam Audio C API reference](https://valvesoftware.github.io/steam-audio/doc/capi/reference.html) for additional details.

Note that because the FMOD and Wwise integrations depend on files that are local to your system, documentation for the `fmod` and `wwise` modules is not available on docs.rs.
However, it can be generated locally using `cargo doc --open --features fmod` (resp. `wwise`).

## License

`audionimbus-sys` is dual-licensed under the [MIT License](https://github.com/MaxenceMaire/audionimbus/blob/master/LICENSE-MIT) and the [Apache-2.0 License](https://github.com/MaxenceMaire/audionimbus/blob/master/LICENSE-APACHE).
You may choose either license when using the software.
*/

mod phonon;
pub use phonon::*;

#[cfg(feature = "fmod")]
pub mod fmod;
#[cfg(feature = "fmod")]
pub use fmod::*;

#[cfg(feature = "wwise")]
pub mod wwise;
#[cfg(feature = "wwise")]
pub use wwise::*;
