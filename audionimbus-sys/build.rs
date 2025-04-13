use std::path::{Path, PathBuf};

const PHONON_HEADER_PATH: &str = "steam-audio/core/src/core/phonon.h";

fn main() {
    println!("cargo::rerun-if-changed=steam-audio");

    let out_dir_path = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_path);

    let version = version();

    generate_bindings_phonon(&out_dir.join("phonon.rs"), &version);

    #[cfg(feature = "fmod")]
    generate_bindings_phonon_fmod(&out_dir.join("phonon_fmod.rs"), &version);
}

fn generate_bindings_phonon(output_path: &Path, version: &Version) {
    println!("cargo:rustc-link-lib=phonon");

    let phonon_header = Path::new(PHONON_HEADER_PATH);
    let phonon_header_dir = phonon_header.parent().unwrap();
    let _phonon_header_guard = temporary_version_header(
        &phonon_header_dir.join("phonon_version.h"),
        version,
        "STEAMAUDIO",
    );

    let bindings = bindgen::Builder::default()
        .header(phonon_header.to_str().unwrap())
        .clang_arg(format!("-I{}", phonon_header_dir.display()))
        .rustified_enum(".*")
        .bitfield_enum(".*Flags")
        .generate()
        .unwrap();

    bindings.write_to_file(output_path).unwrap();
}

#[cfg(feature = "fmod")]
fn generate_bindings_phonon_fmod(output_path: &Path, version: &Version) {
    const PHONON_FMOD_HEADER_PATH: &str = "steam-audio/fmod/src/phonon_mod.h";

    println!("cargo:rustc-link-lib=phonon_fmod");

    let phonon_header = Path::new(PHONON_HEADER_PATH);
    let phonon_header_dir = phonon_header.parent().unwrap();
    let _phonon_header_guard = temporary_version_header(
        &phonon_header_dir.join("phonon_version.h"),
        version,
        "STEAMAUDIO",
    );

    let phonon_fmod_header = Path::new(PHONON_FMOD_HEADER_PATH);
    let phonon_fmod_header_dir = phonon_fmod_header.parent().unwrap();
    let _phonon_fmod_header_guard = temporary_version_header(
        &phonon_fmod_header_dir.join("steamaudio_fmod_version.h"),
        version,
        "STEAMAUDIO_FMOD",
    );

    let bindings = bindgen::Builder::default()
        .header(
            phonon_fmod_header_dir
                .join("steamaudio_fmod.h")
                .to_str()
                .unwrap(),
        )
        .clang_args(&[
            String::from("-xc++"),
            String::from("-std=c++11"),
            format!("-I{}", phonon_header_dir.display()),
            format!("-I{}", phonon_fmod_header_dir.display()),
            format!("-I{}", "steam-audio/fmod/include"),
        ])
        .rustified_enum(".*")
        .bitfield_enum(".*Flags")
        .blocklist_type("_?IPL.*")
        .allowlist_function("iplFMOD.*")
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

    // TODO: remove statement upon release of Steam Audio v4.6.2.
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
