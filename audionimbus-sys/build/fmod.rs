use std::path::Path;

use super::{PHONON_HEADER_PATH, Version, temporary_version_header};

const PHONON_FMOD_HEADER_PATH: &str = "steam-audio/fmod/src/steamaudio_fmod.h";

pub(super) fn generate_bindings(
    output_path: &Path,
    version: &Version,
    tmp_dir: &Path,
    system_flags: &[String],
) {
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

    let fmod_sdk = std::env::var("FMODSDK").expect("env var FMODSDK not set");
    let fmod_includes = Path::new(&fmod_sdk).join("api").join("core").join("inc");

    let bindings = bindgen::Builder::default()
        .header(PHONON_FMOD_HEADER_PATH)
        .clang_args(&[
            String::from("-xc++"),
            String::from("-std=c++14"),
            format!("-I{}", tmp_dir.display()),
            format!("-I{}", fmod_includes.display()),
            format!("-I{}", phonon_header_dir.display()),
            format!("-I{}", "steam-audio/fmod/include"),
        ])
        .clang_args(system_flags)
        .rustified_enum(".*")
        .bitfield_enum(".*Flags")
        .blocklist_type("_?IPL.*")
        .allowlist_function("iplFMOD.*")
        .generate()
        .unwrap();

    bindings.write_to_file(output_path).unwrap();
}
