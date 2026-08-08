use std::path::Path;

use super::{TargetInfo, copy_libraries, install_archive};
use crate::version;

/// Installs the FMOD integration.
pub(super) fn install(
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

    let installed_now = install_archive(
        &zip_path,
        &extract_dir,
        &download_url,
        &install_name,
        target_info,
    )?;

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
