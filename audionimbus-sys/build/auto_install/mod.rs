use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(feature = "fmod")]
mod fmod;
#[cfg(feature = "wwise")]
mod wwise;

use super::version;

/// Installs enabled Steam Audio components.
pub(super) fn handle() -> Result<bool, Box<dyn std::error::Error>> {
    let target_info = get_target_info()?;
    let cache_dir = get_cache_dir()?;
    fs::create_dir_all(&cache_dir)?;

    let did_work = install_steam_audio(&cache_dir, &target_info)?;

    #[cfg(feature = "fmod")]
    let did_work = did_work | fmod::install(&cache_dir, &target_info)?;

    #[cfg(feature = "wwise")]
    let did_work = did_work | wwise::install(&cache_dir, &target_info)?;

    Ok(did_work)
}

#[derive(Debug, Clone)]
/// Information about Cargo's target.
struct TargetInfo {
    platform: String,
    arch: String,
    lib_dir: String,
    lib_names: Vec<String>,
    _is_static: bool,
}

/// Returns library information for Cargo's target.
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

/// Returns the installation cache path.
fn get_cache_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let mut cache_dir = PathBuf::from(out_dir);
    cache_dir.push("steam_audio_cache");
    Ok(cache_dir)
}

/// Installs the core Steam Audio library.
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

    let installed_now = install_archive(
        &zip_path,
        &extract_dir,
        &download_url,
        &install_name,
        target_info,
    )?;

    copy_libraries(
        &extract_dir.join("steamaudio"),
        target_info,
        &target_info.lib_names,
        installed_now,
    )?;

    Ok(installed_now)
}

/// Downloads and extracts an archive when its cache is stale.
fn install_archive(
    zip_path: &Path,
    extract_dir: &Path,
    download_url: &str,
    install_name: &str,
    target_info: &TargetInfo,
) -> Result<bool, Box<dyn std::error::Error>> {
    let version = version().to_string();
    let version_marker = extract_dir.join(".version");

    if version_marker.exists()
        && fs::read_to_string(&version_marker)
            .unwrap_or_default()
            .trim()
            == version
    {
        log_install_progress(format!(
            "{install_name} already installed for {} ({}), using cached files.",
            target_info.platform, target_info.arch
        ));
        return Ok(false);
    }

    log_install_progress(format!(
        "{install_name} not found for {} ({}); installing.",
        target_info.platform, target_info.arch
    ));
    ensure_downloaded_zip(zip_path, download_url, install_name)?;
    extract_zip(zip_path, extract_dir)?;
    fs::write(version_marker, version)?;

    Ok(true)
}

/// Downloads a file with an available system client.
fn download_file(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    if dest.exists() {
        fs::remove_file(dest)?;
    }

    let curl_result = Command::new("curl")
        .args([
            "-L",
            "--progress-bar",
            "-f",
            "--retry",
            "3",
            "--retry-delay",
            "1",
            "-o",
            dest.to_str().unwrap(),
            url,
        ])
        .status();

    match curl_result {
        Ok(status) if status.success() => {
            validate_download(dest)?;
            Ok(())
        }
        _ => {
            let _ = fs::remove_file(dest);
            log_install_progress("curl failed, trying wget...");
            let wget_result = Command::new("wget")
                .args([
                    "--tries=3",
                    "--waitretry=1",
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
                Err(error) => Err(format!("Neither curl nor wget available: {error}").into()),
            }
        }
    }
}

/// Verifies every entry in a ZIP archive.
fn test_zip(zip_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::io;

    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        io::copy(&mut entry, &mut io::sink())?;
    }
    Ok(())
}

/// Validates a downloaded archive.
fn validate_download(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    test_zip(path).map_err(|error| format!("Downloaded archive is invalid: {error}").into())
}

/// Ensures a valid archive exists in the cache.
fn ensure_downloaded_zip(
    zip_path: &Path,
    url: &str,
    install_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if zip_path.exists() {
        match validate_download(zip_path) {
            Ok(()) => return Ok(()),
            Err(error) => {
                log_install_progress(format!(
                    "Cached archive {} is incomplete or corrupted ({}). Removing it and downloading again.",
                    zip_path.display(),
                    error
                ));
                fs::remove_file(zip_path)?;
            }
        }
    }

    log_install_progress(format!("Downloading {install_name}..."));
    download_file(url, zip_path).map_err(|error| {
        format!(
            "{install_name} download failed. If the cache is stuck, delete {} and try again: {error}",
            zip_path.display()
        )
        .into()
    })
}

/// Extracts an archive without allowing path traversal.
fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::io;

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

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let rel_path = match entry.enclosed_name() {
            Some(path) => path.to_owned(),
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

/// Copies target libraries into Cargo's output directory.
fn copy_libraries(
    extract_dir: &Path,
    target_info: &TargetInfo,
    lib_names: &[String],
    force_copy: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let lib_src_dir = extract_dir.join("lib").join(&target_info.lib_dir);
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

    println!("cargo:rustc-link-search=native={}", lib_dest_dir.display());

    Ok(())
}

/// Logs installation progress when enabled.
fn log_install_progress(message: impl AsRef<str>) {
    if install_progress_enabled() {
        println!("cargo:warning=[auto-install] {}", message.as_ref());
    }
}

/// Returns whether installation progress is enabled.
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

/// Forces the build script to run again on the next build.
pub(super) fn force_rerun() {
    println!("cargo::rerun-if-changed=RERUN");
}
