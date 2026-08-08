use std::path::{Path, PathBuf};

#[cfg(feature = "auto-install")]
mod auto_install;
#[cfg(feature = "fmod")]
mod fmod;
#[cfg(feature = "wwise")]
mod wwise;

const PHONON_HEADER_PATH: &str = "steam-audio/core/src/core/phonon.h";

fn main() {
    println!("cargo::rerun-if-changed=steam-audio");
    println!("cargo::rerun-if-env-changed=AUDIONIMBUS_AUTO_INSTALL_PROGRESS");
    println!("cargo::rerun-if-env-changed=STEAMAUDIO_LIB_DIR");

    let out_dir_path = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_path);
    let target = std::env::var("TARGET").unwrap();
    let system_flags = system_flags(&target).unwrap_or_else(|error| panic!("{error}"));
    let version = version();

    #[cfg(feature = "auto-install")]
    {
        let did_work =
            auto_install::handle().unwrap_or_else(|error| panic!("auto-install failed: {error}"));

        if did_work {
            auto_install::force_rerun();
        }
    }

    emit_manual_link_search_path();
    generate_bindings_phonon(&out_dir.join("phonon.rs"), &version, out_dir, &system_flags);

    #[cfg(feature = "fmod")]
    fmod::generate_bindings(
        &out_dir.join("phonon_fmod.rs"),
        &version,
        out_dir,
        &system_flags,
    );

    #[cfg(feature = "wwise")]
    wwise::generate_bindings(
        &out_dir.join("phonon_wwise.rs"),
        &version,
        out_dir,
        &system_flags,
    );
}

/// Emits the configured manual library search path.
fn emit_manual_link_search_path() {
    if let Ok(lib_dir) = std::env::var("STEAMAUDIO_LIB_DIR") {
        println!("cargo:rustc-link-search=native={lib_dir}");
    }
}

/// Generates the core bindings.
fn generate_bindings_phonon(
    output_path: &Path,
    version: &Version,
    tmp_dir: &Path,
    system_flags: &[String],
) {
    println!("cargo:rustc-link-lib=phonon");

    let _phonon_header_guard =
        temporary_version_header(&tmp_dir.join("phonon_version.h"), version, "STEAMAUDIO");

    let bindings = bindgen::Builder::default()
        .header(PHONON_HEADER_PATH)
        .clang_arg(format!("-I{}", tmp_dir.display()))
        .clang_args(system_flags)
        .rustified_enum(".*")
        .bitfield_enum(".*Flags")
        .generate()
        .unwrap();

    bindings.write_to_file(output_path).unwrap();
}

/// The Steam Audio version used by the build.
struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Returns the pinned Steam Audio version.
fn version() -> Version {
    let major = std::env::var("CARGO_PKG_VERSION_MAJOR")
        .unwrap()
        .parse::<u32>()
        .unwrap();

    let minor = std::env::var("CARGO_PKG_VERSION_MINOR")
        .unwrap()
        .parse::<u32>()
        .unwrap();

    let _patch = std::env::var("CARGO_PKG_VERSION_PATCH")
        .unwrap()
        .parse::<u32>()
        .unwrap();

    // TODO: remove statement upon new release of Steam Audio.
    // The version of audionimbus-sys is temporarily ahead of Steam Audio's
    // to allow for the introduction of new features, so we need to explicitly
    // pin the version.
    let patch = 1;

    Version {
        major,
        minor,
        patch,
    }
}

/// Writes a temporary native version header.
fn temporary_version_header(path: &Path, version: &Version, prefix: &str) -> TemporaryFileGuard {
    let packed_version = (version.major << 16) | (version.minor << 8) | version.patch;
    let version_header = format!(
        r"
#ifndef IPL_PHONON_VERSION_H
#define IPL_PHONON_VERSION_H

#define {prefix}_VERSION_MAJOR {}
#define {prefix}_VERSION_MINOR {}
#define {prefix}_VERSION_PATCH {}
#define {prefix}_VERSION       {packed_version}

#endif
",
        version.major, version.minor, version.patch,
    );
    std::fs::write(path, version_header).unwrap();

    TemporaryFileGuard(path.to_path_buf())
}

/// Removes a temporary file when dropped.
struct TemporaryFileGuard(PathBuf);

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Returns native preprocessor flags for a target.
fn system_flags(target: &str) -> Result<Vec<String>, String> {
    let (os, cpu) = match target {
        "i686-pc-windows-msvc" => ("IPL_OS_WINDOWS", Some("IPL_CPU_X86")),
        "x86_64-pc-windows-msvc" => ("IPL_OS_WINDOWS", Some("IPL_CPU_X64")),
        "i686-unknown-linux-gnu" => ("IPL_OS_LINUX", Some("IPL_CPU_X86")),
        "x86_64-unknown-linux-gnu" => ("IPL_OS_LINUX", Some("IPL_CPU_X64")),
        "aarch64-apple-darwin" | "x86_64-apple-darwin" => ("IPL_OS_MACOSX", None),
        "armv7-linux-androideabi" => ("IPL_OS_ANDROID", Some("IPL_CPU_ARMV7")),
        "aarch64-linux-android" => ("IPL_OS_ANDROID", Some("IPL_CPU_ARMV8")),
        "i686-linux-android" => ("IPL_OS_ANDROID", Some("IPL_CPU_X86")),
        "x86_64-linux-android" => ("IPL_OS_ANDROID", Some("IPL_CPU_X64")),
        "aarch64-apple-ios" => ("IPL_OS_IOS", Some("IPL_CPU_ARMV8")),
        "wasm32-unknown-emscripten" => ("IPL_OS_WASM", Some("IPL_CPU_ARMV7")),
        _ => return Err(format!("unsupported target: {target}")),
    };

    let mut flags = vec![format!("--target={target}"), format!("-D{os}")];
    if let Some(cpu) = cpu {
        flags.push(format!("-D{cpu}"));
    }

    Ok(flags)
}
