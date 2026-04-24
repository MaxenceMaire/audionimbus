use std::string::ToString;

#[cfg(feature = "auto-install")]
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(feature = "auto-install")]
use std::sync::OnceLock;

const PHONON_HEADER_PATH: &str = "steam-audio/core/src/core/phonon.h";

fn main() {
    println!("cargo::rerun-if-changed=steam-audio");
    println!("cargo::rerun-if-env-changed=AUDIONIMBUS_AUTO_INSTALL_PROGRESS");

    let out_dir_path = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_path);

    let version = version();

    #[cfg(feature = "auto-install")]
    {
        let did_work = handle_auto_install().unwrap_or_else(|e| panic!("auto-install failed: {e}"));

        if did_work {
            force_rerun();
        }
    }

    generate_bindings_phonon(&out_dir.join("phonon.rs"), &version, out_dir);

    #[cfg(feature = "fmod")]
    generate_bindings_phonon_fmod(&out_dir.join("phonon_fmod.rs"), &version, out_dir);

    #[cfg(feature = "wwise")]
    generate_bindings_phonon_wwise(&out_dir.join("phonon_wwise.rs"), &version, out_dir);
}

/// Returns `false` if the cache was already up to date.
#[cfg(feature = "auto-install")]
fn handle_auto_install() -> Result<bool, Box<dyn std::error::Error>> {
    let target_info = get_target_info()?;

    // Create cache directory.
    let cache_dir = get_cache_dir()?;
    fs::create_dir_all(&cache_dir)?;

    let mut did_work = false;

    // Install base Steam Audio.
    did_work |= install_steam_audio(&cache_dir, &target_info)?;

    // Install FMOD integration if feature is enabled.
    #[cfg(feature = "fmod")]
    {
        did_work |= install_fmod_integration(&cache_dir, &target_info)?;
    }

    // Install Wwise integration if feature is enabled.
    #[cfg(feature = "wwise")]
    {
        did_work |= install_wwise_integration(&cache_dir, &target_info)?;
    }

    Ok(did_work)
}

#[cfg(feature = "auto-install")]
#[derive(Debug, Clone)]
struct TargetInfo {
    platform: String,
    arch: String,
    lib_dir: String,
    lib_names: Vec<String>,
    _is_static: bool,
}

#[cfg(feature = "auto-install")]
fn get_target_info() -> Result<TargetInfo, Box<dyn std::error::Error>> {
    let target = std::env::var("TARGET")?;

    let (platform, arch, lib_dir, lib_names, is_static) = match target.as_str() {
        t if t.contains("windows") && t.contains("i686") => (
            "windows".to_string(),
            "x86".to_string(),
            "windows-x86".to_string(),
            vec!["phonon.dll".to_string()],
            false,
        ),
        t if t.contains("windows") && t.contains("x86_64") => (
            "windows".to_string(),
            "x64".to_string(),
            "windows-x64".to_string(),
            vec!["phonon.dll".to_string(), "phonon.lib".to_string()],
            false,
        ),
        t if t.contains("linux") && t.contains("i686") => (
            "linux".to_string(),
            "x86".to_string(),
            "linux-x86".to_string(),
            vec!["libphonon.so".to_string()],
            false,
        ),
        t if t.contains("linux") && t.contains("x86_64") => (
            "linux".to_string(),
            "x64".to_string(),
            "linux-x64".to_string(),
            vec!["libphonon.so".to_string()],
            false,
        ),
        t if t.contains("apple-darwin") => (
            "macos".to_string(),
            "universal".to_string(),
            "osx".to_string(),
            vec!["libphonon.dylib".to_string()],
            false,
        ),
        t if t.contains("android") && t.contains("armv7") => (
            "android".to_string(),
            "armv7".to_string(),
            "android-armv7".to_string(),
            vec!["libphonon.so".to_string()],
            false,
        ),
        t if t.contains("android") && (t.contains("aarch64") || t.contains("armv8")) => (
            "android".to_string(),
            "armv8".to_string(),
            "android-armv8".to_string(),
            vec!["libphonon.so".to_string()],
            false,
        ),
        t if t.contains("android") && t.contains("i686") => (
            "android".to_string(),
            "x86".to_string(),
            "android-x86".to_string(),
            vec!["libphonon.so".to_string()],
            false,
        ),
        t if t.contains("android") && t.contains("x86_64") => (
            "android".to_string(),
            "x64".to_string(),
            "android-x64".to_string(),
            vec!["libphonon.so".to_string()],
            false,
        ),
        t if t.contains("ios") => (
            "ios".to_string(),
            "armv8".to_string(),
            "ios".to_string(),
            vec!["libphonon.a".to_string()],
            true,
        ),
        _ => return Err(format!("Unsupported target: {target}").into()),
    };

    Ok(TargetInfo {
        platform,
        arch,
        lib_dir,
        lib_names,
        _is_static: is_static,
    })
}

#[cfg(feature = "auto-install")]
fn get_cache_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let mut cache_dir = PathBuf::from(out_dir);
    cache_dir.push("steam_audio_cache");
    Ok(cache_dir)
}

/// Returns `false` if the cache was already up to date.
#[cfg(feature = "auto-install")]
fn install_steam_audio(
    cache_dir: &Path,
    target_info: &TargetInfo,
) -> Result<bool, Box<dyn std::error::Error>> {
    let version = version().to_string();
    let zip_name = format!("steamaudio_{version}.zip");
    let zip_path = cache_dir.join(&zip_name);
    let extract_dir = cache_dir.join("steamaudio_core");
    let install_name = format!("Steam Audio {version}");
    let download_url = format!(
        "https://github.com/ValveSoftware/steam-audio/releases/download/v{version}/steamaudio_{version}.zip"
    );

    // Check if already extracted and up to date.
    let version_marker = extract_dir.join(".version");
    let installed_now = if version_marker.exists()
        && fs::read_to_string(&version_marker)
            .unwrap_or_default()
            .trim()
            == version
    {
        log_install_progress(format!(
            "{install_name} already installed for {} ({}), using cached files.",
            target_info.platform, target_info.arch
        ));
        false
    } else {
        log_install_progress(format!(
            "{install_name} not found for {} ({}); installing.",
            target_info.platform, target_info.arch
        ));

        ensure_downloaded_zip(&zip_path, &download_url, &install_name)?;
        extract_zip(&zip_path, &extract_dir)?;

        // Mark version.
        fs::write(&version_marker, &version)?;
        true
    };

    copy_libraries(
        &extract_dir.join("steamaudio"),
        target_info,
        &target_info.lib_names,
        installed_now,
    )?;

    Ok(installed_now)
}

/// Returns `false` if the cache was already up to date.
#[cfg(all(feature = "auto-install", feature = "fmod"))]
fn install_fmod_integration(
    cache_dir: &Path,
    target_info: &TargetInfo,
) -> Result<bool, Box<dyn std::error::Error>> {
    let version = version().to_string();
    let zip_name = format!("steamaudio_fmod_{version}.zip");
    let zip_path = cache_dir.join(&zip_name);
    let extract_dir = cache_dir.join("steamaudio_fmod");
    let install_name = format!("Steam Audio FMOD integration {version}");
    let download_url = format!(
        "https://github.com/ValveSoftware/steam-audio/releases/download/v{version}/steamaudio_fmod_{version}.zip"
    );

    // Check if already extracted and up to date.
    let version_marker = extract_dir.join(".version");
    let installed_now = if version_marker.exists()
        && fs::read_to_string(&version_marker)
            .unwrap_or_default()
            .trim()
            == version
    {
        log_install_progress(format!(
            "{install_name} already installed for {} ({}), using cached files.",
            target_info.platform, target_info.arch
        ));
        false
    } else {
        log_install_progress(format!(
            "{install_name} not found for {} ({}); installing.",
            target_info.platform, target_info.arch
        ));

        ensure_downloaded_zip(&zip_path, &download_url, &install_name)?;
        extract_zip(&zip_path, &extract_dir)?;

        // Mark version.
        fs::write(&version_marker, &version)?;
        true
    };

    // Copy FMOD libraries.
    let fmod_lib_name = match target_info.platform.as_str() {
        "windows" => "phonon_fmod.dll",
        "linux" | "android" => "libphonon_fmod.so",
        "macos" => "libphonon_fmod.dylib",
        "ios" => "libphonon_fmod.a",
        _ => return Err("Unsupported platform for FMOD integration".into()),
    };

    copy_libraries(
        &extract_dir.join("steamaudio_fmod"),
        target_info,
        &[fmod_lib_name.to_string()],
        installed_now,
    )?;

    Ok(installed_now)
}

/// Returns `false` if the cache was already up to date.
#[cfg(all(feature = "auto-install", feature = "wwise"))]
fn install_wwise_integration(
    cache_dir: &Path,
    target_info: &TargetInfo,
) -> Result<bool, Box<dyn std::error::Error>> {
    let version = version().to_string();
    let zip_name = format!("steamaudio_wwise_{version}.zip");
    let zip_path = cache_dir.join(&zip_name);
    let extract_dir = cache_dir.join("steamaudio_wwise");
    let install_name = format!("Steam Audio Wwise integration {version}");
    let download_url = format!(
        "https://github.com/ValveSoftware/steam-audio/releases/download/v{version}/steamaudio_wwise_{version}.zip"
    );

    // Check if already extracted and up to date.
    let version_marker = extract_dir.join(".version");
    let installed_now = if version_marker.exists()
        && fs::read_to_string(&version_marker)
            .unwrap_or_default()
            .trim()
            == version
    {
        log_install_progress(format!(
            "{install_name} already installed for {} ({}), using cached files.",
            target_info.platform, target_info.arch
        ));
        false
    } else {
        log_install_progress(format!(
            "{install_name} not found for {} ({}); installing.",
            target_info.platform, target_info.arch
        ));

        ensure_downloaded_zip(&zip_path, &download_url, &install_name)?;
        extract_zip(&zip_path, &extract_dir)?;

        // Mark version.
        fs::write(&version_marker, &version)?;
        true
    };

    // Copy Wwise libraries - the actual library name might vary.
    let wwise_lib_name = "SteamAudioWwise"; // This might need adjustment based on actual file names.
    let lib_names = vec![
        format!("lib{}.so", wwise_lib_name),
        format!("lib{}.dylib", wwise_lib_name),
        format!("{}.dll", wwise_lib_name),
        format!("lib{}.a", wwise_lib_name),
    ];

    // Find which one exists and copy it.
    for lib_name in lib_names {
        let src = extract_dir
            .join("lib")
            .join(&target_info.lib_dir)
            .join(&lib_name);
        if src.exists() {
            copy_libraries(
                &extract_dir.join("steamaudio_wwise"),
                target_info,
                &[lib_name],
                installed_now,
            )?;
            break;
        }
    }

    Ok(installed_now)
}

#[cfg(feature = "auto-install")]
fn download_file(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    // Remove any existing partial download.
    if dest.exists() {
        fs::remove_file(dest)?;
    }

    // Try to use curl first with progress bar.
    let curl_result = Command::new("curl")
        .args([
            "-L",             // Follow redirects
            "--progress-bar", // Show progress bar
            "-f",             // Fail on HTTP errors
            "--retry",
            "3", // Retry on transient errors
            "--retry-delay",
            "1", // Wait 1 second between retries
            "-o",
            dest.to_str().unwrap(),
            url,
        ])
        .status();

    match curl_result {
        Ok(status) if status.success() => {
            // Verify the downloaded file is valid.
            validate_download(dest)?;
            Ok(())
        }
        _ => {
            // Clean up failed download.
            let _ = fs::remove_file(dest);

            // Try wget as fallback with progress.
            log_install_progress("curl failed, trying wget...");
            let wget_result = Command::new("wget")
                .args([
                    "--tries=3",     // Retry on failure
                    "--waitretry=1", // Wait between retries
                    "-O",
                    dest.to_str().unwrap(),
                    url,
                ])
                .status();

            match wget_result {
                Ok(status) if status.success() => {
                    validate_download(dest)?;
                    Ok(())
                }
                Ok(_) => Err("wget failed to download file".into()),
                Err(e) => Err(format!("Neither curl nor wget available: {e}").into()),
            }
        }
    }
}

#[cfg(feature = "auto-install")]
fn test_zip(zip_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, io};

    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        // Read fully to verify CRC.
        io::copy(&mut entry, &mut io::sink())?;
    }
    Ok(())
}

#[cfg(feature = "auto-install")]
fn validate_download(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    test_zip(path).map_err(|e| format!("Downloaded archive is invalid: {e}").into())
}

#[cfg(feature = "auto-install")]
fn ensure_downloaded_zip(
    zip_path: &Path,
    url: &str,
    install_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if zip_path.exists() {
        match validate_download(zip_path) {
            Ok(()) => return Ok(()),
            Err(err) => {
                log_install_progress(format!(
                    "Cached archive {} is incomplete or corrupted ({}). Removing it and downloading again.",
                    zip_path.display(),
                    err
                ));
                fs::remove_file(zip_path)?;
            }
        }
    }

    log_install_progress(format!("Downloading {install_name}..."));
    download_file(url, zip_path).map_err(|err| {
        format!(
            "{install_name} download failed. If the cache is stuck, delete {} and try again: {err}",
            zip_path.display()
        )
        .into()
    })
}

#[cfg(feature = "auto-install")]
fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, io};

    // Remove existing directory if it exists.
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir)?;
    }
    fs::create_dir_all(dest_dir)?;

    log_install_progress(format!(
        "Extracting {} to {}...",
        zip_path.file_name().unwrap().to_string_lossy(),
        dest_dir.file_name().unwrap().to_string_lossy()
    ));

    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;

        // Prevent Zip-Slip; skip dangerous paths.
        let rel_path = match entry.enclosed_name() {
            Some(p) => p.to_owned(),
            None => continue,
        };
        let outpath = dest_dir.join(rel_path);

        if entry.is_dir() || entry.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut entry, &mut outfile)?;

            // Preserve UNIX perms when present (no-op on Windows).
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(feature = "auto-install")]
fn copy_libraries(
    extract_dir: &Path,
    target_info: &TargetInfo,
    lib_names: &[String],
    force_copy: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let lib_src_dir = extract_dir.join("lib").join(&target_info.lib_dir);

    // Create a lib directory in OUT_DIR for the libraries.
    let out_dir = std::env::var("OUT_DIR")?;
    let lib_dest_dir = Path::new(&out_dir).join("lib");
    fs::create_dir_all(&lib_dest_dir)?;

    for lib_name in lib_names {
        let src = lib_src_dir.join(lib_name);
        let dest = lib_dest_dir.join(lib_name);

        if src.exists() {
            if force_copy || !dest.exists() {
                log_install_progress(format!("Copying {} to {}", src.display(), dest.display()));
                fs::copy(&src, &dest)?;
            }
        } else {
            return Err(format!("Required library not found: {}", src.display()).into());
        }
    }

    // Tell cargo where to find the libraries.
    println!("cargo:rustc-link-search=native={}", lib_dest_dir.display());

    Ok(())
}

#[cfg(feature = "auto-install")]
fn log_install_progress(message: impl AsRef<str>) {
    if install_progress_enabled() {
        println!("cargo:warning=[auto-install] {}", message.as_ref());
    }
}

#[cfg(feature = "auto-install")]
fn install_progress_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();

    *ENABLED.get_or_init(|| {
        std::env::var("AUDIONIMBUS_AUTO_INSTALL_PROGRESS")
            .map(|value| {
                !matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "0" | "false" | "no" | "off"
                )
            })
            .unwrap_or(true)
    })
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

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
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

    flags.into_iter().map(ToString::to_string).collect()
}

/// Forces to re-run the build script on the next build.
fn force_rerun() {
    println!("cargo::rerun-if-changed=RERUN");
}
