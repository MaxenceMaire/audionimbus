use std::path::Path;

use super::{PHONON_HEADER_PATH, Version, temporary_version_header};

const PHONON_WWISE_HEADER_PATH: &str = "steam-audio/wwise/src/SoundEnginePlugin/SteamAudioCommon.h";

pub(super) fn generate_bindings(
    output_path: &Path,
    version: &Version,
    tmp_dir: &Path,
    system_flags: &[String],
) {
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
        .clang_args(system_flags)
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
