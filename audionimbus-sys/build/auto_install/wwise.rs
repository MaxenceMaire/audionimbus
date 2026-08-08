use std::path::Path;

use super::{TargetInfo, copy_libraries, install_archive};
use crate::version;

/// Installs the Wwise integration.
pub(super) fn install(
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

    let installed_now = install_archive(
        &zip_path,
        &extract_dir,
        &download_url,
        &install_name,
        target_info,
    )?;

    let archive_root = extract_dir.join("steamaudio_wwise");
    let lib_names = [
        "libSteamAudioWwise.so",
        "libSteamAudioWwise.dylib",
        "SteamAudioWwise.dll",
        "libSteamAudioWwise.a",
    ]
    .map(String::from);

    let lib_name = find_supported_library(&archive_root, &target_info.lib_dir, &lib_names)?;
    copy_libraries(
        &archive_root,
        target_info,
        &[lib_name.to_string()],
        installed_now,
    )?;

    Ok(installed_now)
}

/// Returns the first available Wwise library.
fn find_supported_library<'a>(
    archive_root: &Path,
    lib_dir: &str,
    lib_names: &'a [String],
) -> Result<&'a str, String> {
    let lib_root = archive_root.join("lib").join(lib_dir);

    lib_names
        .iter()
        .find(|lib_name| lib_root.join(lib_name).is_file())
        .map(String::as_str)
        .ok_or_else(|| {
            format!(
                "No supported Wwise library found in {} (expected one of: {})",
                lib_root.display(),
                lib_names.join(", ")
            )
        })
}
