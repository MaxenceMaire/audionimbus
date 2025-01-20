fn main() {
    println!("cargo::rerun-if-changed=steam-audio");

    let out_dir_path = std::env::var("OUT_DIR").unwrap();
    let out_dir = std::path::Path::new(&out_dir_path);

    let major_version = std::env::var("CARGO_PKG_VERSION_MAJOR")
        .unwrap()
        .parse::<u32>()
        .unwrap();

    let minor_version = std::env::var("CARGO_PKG_VERSION_MINOR")
        .unwrap()
        .parse::<u32>()
        .unwrap();

    let patch_version = std::env::var("CARGO_PKG_VERSION_PATCH")
        .unwrap()
        .parse::<u32>()
        .unwrap();

    let packed_version = major_version << 16 | minor_version << 8 | patch_version;

    let phonon_version = format!(
        r#"
#ifndef IPL_PHONON_VERSION_H
#define IPL_PHONON_VERSION_H

#define STEAMAUDIO_VERSION_MAJOR {major_version}
#define STEAMAUDIO_VERSION_MINOR {minor_version}
#define STEAMAUDIO_VERSION_PATCH {patch_version}
#define STEAMAUDIO_VERSION       {packed_version}

#endif
"#,
    );
    let phonon_version_header = out_dir.join("phonon_version.h");
    std::fs::write(phonon_version_header.clone(), phonon_version).unwrap();

    let header_dir = std::path::Path::new("steam-audio/core/src/core");

    let bindings = bindgen::Builder::default()
        .header(header_dir.join("phonon.h").to_str().unwrap())
        .clang_arg(format!("--include-directory={}", header_dir.display()))
        .clang_arg(format!("--include-directory={}", out_dir.display()))
        .rustified_enum(".*")
        .bitfield_enum(".*Flags")
        .generate()
        .unwrap();

    std::fs::remove_file(phonon_version_header).unwrap();

    bindings.write_to_file(out_dir.join("phonon.rs")).unwrap();
}
