use std::path::{Path, PathBuf};

const PHONON_HEADER_PATH: &str = "steam-audio/core/src/core/phonon.h";

fn main() {
    println!("cargo::rerun-if-changed=steam-audio");

    let out_dir_path = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_path);

    let version = version();

    generate_bindings_phonon(&out_dir.join("phonon.rs"), &version, out_dir);

    #[cfg(feature = "fmod")]
    generate_bindings_phonon_fmod(&out_dir.join("phonon_fmod.rs"), &version, out_dir);

    #[cfg(feature = "wwise")]
    generate_bindings_phonon_wwise(&out_dir.join("phonon_wwise.rs"), &version, out_dir);
}

fn generate_bindings_phonon(output_path: &Path, version: &Version, tmp_dir: &Path) {
    println!("cargo:rustc-link-lib=phonon");

    let _phonon_header_guard =
        temporary_version_header(&tmp_dir.join("phonon_version.h"), version, "STEAMAUDIO");

    let bindings = bindgen::Builder::default()
        .header(PHONON_HEADER_PATH)
        .clang_arg(format!("-I{}", tmp_dir.display()))
        .clang_args(system_flags())
        .rustified_enum(".*")
        .bitfield_enum(".*Flags")
        .generate()
        .unwrap();

    bindings.write_to_file(output_path).unwrap();
}

#[cfg(feature = "fmod")]
fn generate_bindings_phonon_fmod(output_path: &Path, version: &Version, tmp_dir: &Path) {
    const PHONON_FMOD_HEADER_PATH: &str = "steam-audio/fmod/src/steamaudio_fmod.h";

    println!("cargo:rustc-link-lib=phonon_fmod");

    let _phonon_header_guard =
        temporary_version_header(&tmp_dir.join("phonon_version.h"), version, "STEAMAUDIO");

    let _phonon_fmod_header_guard = temporary_version_header(
        &tmp_dir.join("steamaudio_fmod_version.h"),
        version,
        "STEAMAUDIO_FMOD",
    );

    let phonon_header = Path::new(PHONON_HEADER_PATH);
    let phonon_header_dir = phonon_header.parent().unwrap();

    let bindings = bindgen::Builder::default()
        .header(PHONON_FMOD_HEADER_PATH)
        .clang_args(&[
            String::from("-xc++"),
            String::from("-std=c++14"),
            format!("-I{}", tmp_dir.display()),
            format!("-I{}", phonon_header_dir.display()),
            format!("-I{}", "steam-audio/fmod/include"),
        ])
        .clang_args(system_flags())
        .rustified_enum(".*")
        .bitfield_enum(".*Flags")
        .blocklist_type("_?IPL.*")
        .allowlist_function("iplFMOD.*")
        .generate()
        .unwrap();

    bindings.write_to_file(output_path).unwrap();
}

#[cfg(feature = "wwise")]
fn generate_bindings_phonon_wwise(output_path: &Path, version: &Version, tmp_dir: &Path) {
    const PHONON_WWISE_HEADER_PATH: &str =
        "steam-audio/wwise/src/SoundEnginePlugin/SteamAudioCommon.h";

    let wwise_sdk = std::env::var("WWISESDK").expect("env var WWISESDK not set");
    let wwise_includes = Path::new(&wwise_sdk).join("include");

    println!("cargo:rustc-link-lib=SteamAudioWwise");

    let _phonon_header_guard =
        temporary_version_header(&tmp_dir.join("phonon_version.h"), version, "STEAMAUDIO");

    let _phonon_wwise_header_guard = temporary_version_header(
        &tmp_dir.join("SteamAudioVersion.h"),
        version,
        "STEAMAUDIO_WWISE",
    );

    let phonon_header = Path::new(PHONON_HEADER_PATH);
    let phonon_header_dir = phonon_header.parent().unwrap();

    let bindings = bindgen::Builder::default()
        .header(PHONON_WWISE_HEADER_PATH)
        .clang_args(&[
            String::from("-xc++"),
            String::from("-std=c++14"),
            format!("-I{}", tmp_dir.display()),
            format!("-I{}", phonon_header_dir.display()),
            format!("-I{}", wwise_includes.display()),
        ])
        .clang_args(system_flags())
        .rustified_enum(".*")
        .bitfield_enum(".*Flags")
        .allowlist_recursively(false)
        .allowlist_type("IPLWwise.*")
        .allowlist_type("AkGameObjectID")
        .allowlist_type("AkUInt64")
        .allowlist_function("iplWwise.*")
        .generate()
        .unwrap();

    bindings.write_to_file(output_path).unwrap();
}

struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

fn version() -> Version {
    let major = std::env::var("CARGO_PKG_VERSION_MAJOR")
        .unwrap()
        .parse::<u32>()
        .unwrap();

    let minor = std::env::var("CARGO_PKG_VERSION_MINOR")
        .unwrap()
        .parse::<u32>()
        .unwrap();

    let patch = std::env::var("CARGO_PKG_VERSION_PATCH")
        .unwrap()
        .parse::<u32>()
        .unwrap();

    Version {
        major,
        minor,
        patch,
    }
}

fn temporary_version_header(path: &Path, version: &Version, prefix: &str) -> TemporaryFileGuard {
    let packed_version = (version.major << 16) | (version.minor << 8) | version.patch;
    let version_header = format!(
        r#"
#ifndef IPL_PHONON_VERSION_H
#define IPL_PHONON_VERSION_H

#define {prefix}_VERSION_MAJOR {}
#define {prefix}_VERSION_MINOR {}
#define {prefix}_VERSION_PATCH {}
#define {prefix}_VERSION       {packed_version}

#endif
"#,
        version.major, version.minor, version.patch,
    );
    std::fs::write(path, version_header).unwrap();

    TemporaryFileGuard(path.to_path_buf())
}

// The file this guard points to gets removed when the guard goes out of scope.
struct TemporaryFileGuard(PathBuf);

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn system_flags() -> Vec<String> {
    let mut flags = vec![];

    if cfg!(target_os = "windows") {
        flags.push("-DIPL_OS_WINDOWS");
    } else if cfg!(target_os = "linux") {
        flags.push("-DIPL_OS_LINUX");
    } else if cfg!(target_os = "macos") {
        flags.push("-DIPL_OS_MACOSX");
    } else if cfg!(target_os = "android") {
        flags.push("-DIPL_OS_ANDROID");
    } else if cfg!(target_os = "ios") {
        flags.push("-DIPL_OS_IOS");
    } else if cfg!(target_family = "wasm") {
        flags.push("-DIPL_OS_WASM");
    }

    if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
        if cfg!(target_pointer_width = "64") {
            flags.push("-DIPL_CPU_X64");
        } else if cfg!(target_pointer_width = "32") {
            flags.push("-DIPL_CPU_X86");
        }
    } else if cfg!(target_os = "macos") {
    } else if cfg!(target_os = "android") {
        if std::env::var("TARGET").unwrap().contains("armv8") {
            flags.push("-DIPL_CPU_ARMV8");
        } else if cfg!(target_arch = "arm") {
            flags.push("-DIPL_CPU_ARMV7");
        } else if cfg!(target_arch = "x86") {
            flags.push("-DIPL_CPU_X86");
        } else if cfg!(target_arch = "x86_64") {
            flags.push("-DIPL_CPU_X64");
        }
    } else if cfg!(target_os = "ios") {
        flags.push("-DIPL_CPU_ARMV8");
    } else if cfg!(target_family = "wasm") {
        flags.push("-DIPL_CPU_ARMV7");
    }

    flags.into_iter().map(|s| s.to_string()).collect()
}
