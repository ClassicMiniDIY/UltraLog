//! Auto-update functionality for UltraLog.
//!
//! Checks GitHub releases for new versions, downloads updates,
//! and handles installation with proper extraction and replacement.

use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;

const GITHUB_API_URL: &str = "https://api.github.com/repos/SomethingNew71/UltraLog/releases/latest";
const USER_AGENT: &str = concat!("UltraLog/", env!("CARGO_PKG_VERSION"));

// ============================================================================
// Data Structures
// ============================================================================

/// GitHub release information from the API
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    #[allow(dead_code)]
    pub name: String,
    pub html_url: String,
    pub body: Option<String>,
    pub assets: Vec<ReleaseAsset>,
    pub prerelease: bool,
    pub draft: bool,
}

/// A release asset (downloadable file)
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Information about an available update
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub new_version: String,
    pub release_notes: Option<String>,
    pub download_url: String,
    pub download_size: u64,
    pub release_page_url: String,
}

/// Result from update check operation
#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    /// A newer version is available
    UpdateAvailable(UpdateInfo),
    /// Current version is the latest
    UpToDate,
    /// Error occurred during check
    Error(String),
}

/// Result from download operation
#[derive(Debug, Clone)]
pub enum DownloadResult {
    /// Download completed successfully
    Success(PathBuf),
    /// Download failed
    Error(String),
}

/// Current state of the update process
#[derive(Debug, Clone, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpdateAvailable(UpdateInfo),
    Downloading,
    ReadyToInstall(PathBuf),
    Error(String),
}

/// Platform-specific asset detection
#[derive(Debug, Clone, Copy)]
pub enum Platform {
    WindowsX64,
    MacOSIntel,
    MacOSArm,
    LinuxX64,
}

impl Platform {
    /// Detect current platform at compile time
    pub fn current() -> Option<Self> {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            return Some(Platform::WindowsX64);
        }

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            return Some(Platform::MacOSIntel);
        }

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            return Some(Platform::MacOSArm);
        }

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            Some(Platform::LinuxX64)
        }

        #[cfg(not(any(
            all(target_os = "windows", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "linux", target_arch = "x86_64")
        )))]
        {
            return None;
        }
    }

    /// Get the expected asset filename for this platform
    pub fn asset_name(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "ultralog-windows.zip",
            Platform::MacOSIntel => "ultralog-macos-intel.dmg",
            Platform::MacOSArm => "ultralog-macos-arm64.dmg",
            Platform::LinuxX64 => "ultralog-linux.tar.gz",
        }
    }

    /// Get the file extension for downloaded asset
    pub fn extension(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "zip",
            Platform::MacOSIntel | Platform::MacOSArm => "dmg",
            Platform::LinuxX64 => "tar.gz",
        }
    }
}

// ============================================================================
// Core Functions
// ============================================================================

/// Check for updates by querying GitHub releases API.
/// This is a blocking operation - run in a background thread.
pub fn check_for_updates() -> UpdateCheckResult {
    let current_version = env!("CARGO_PKG_VERSION");

    // Make HTTP request to GitHub API
    let mut response = match ureq::get(GITHUB_API_URL)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github.v3+json")
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::StatusCode(status)) => {
            return UpdateCheckResult::Error(format!("GitHub API returned status {}", status));
        }
        Err(e) => {
            return UpdateCheckResult::Error(format!("Network error: {}", e));
        }
    };

    // Parse JSON response
    let release: GitHubRelease = match response.body_mut().read_json() {
        Ok(r) => r,
        Err(e) => {
            return UpdateCheckResult::Error(format!("Failed to parse response: {}", e));
        }
    };

    // Skip drafts and prereleases
    if release.draft || release.prerelease {
        return UpdateCheckResult::UpToDate;
    }

    // Parse versions for comparison
    let remote_version_str = release.tag_name.trim_start_matches('v');

    let current = match semver::Version::parse(current_version) {
        Ok(v) => v,
        Err(_) => {
            return UpdateCheckResult::Error("Invalid current version format".to_string());
        }
    };

    let remote = match semver::Version::parse(remote_version_str) {
        Ok(v) => v,
        Err(_) => {
            return UpdateCheckResult::Error(format!(
                "Invalid remote version format: {}",
                remote_version_str
            ));
        }
    };

    // Compare versions
    if remote <= current {
        return UpdateCheckResult::UpToDate;
    }

    // Find platform-specific asset
    let platform = match Platform::current() {
        Some(p) => p,
        None => {
            return UpdateCheckResult::Error("Unsupported platform for auto-update".to_string());
        }
    };

    let asset_name = platform.asset_name();
    let asset = match release.assets.iter().find(|a| a.name == asset_name) {
        Some(a) => a,
        None => {
            return UpdateCheckResult::Error(format!(
                "No release asset found for {} in version {}",
                asset_name, remote_version_str
            ));
        }
    };

    UpdateCheckResult::UpdateAvailable(UpdateInfo {
        current_version: current_version.to_string(),
        new_version: remote_version_str.to_string(),
        release_notes: release.body,
        download_url: asset.browser_download_url.clone(),
        download_size: asset.size,
        release_page_url: release.html_url,
    })
}

/// Download update file to temp directory.
/// This is a blocking operation - run in a background thread.
pub fn download_update(url: &str) -> DownloadResult {
    let platform = match Platform::current() {
        Some(p) => p,
        None => return DownloadResult::Error("Unsupported platform".to_string()),
    };

    // Create temp file path
    let temp_dir = std::env::temp_dir();
    let filename = format!("ultralog-update.{}", platform.extension());
    let download_path = temp_dir.join(&filename);

    // Download file
    let response = match ureq::get(url).header("User-Agent", USER_AGENT).call() {
        Ok(resp) => resp,
        Err(e) => return DownloadResult::Error(format!("Download failed: {}", e)),
    };

    // Create output file
    let mut file = match std::fs::File::create(&download_path) {
        Ok(f) => f,
        Err(e) => return DownloadResult::Error(format!("Failed to create file: {}", e)),
    };

    // Read response body into file
    let mut reader = response.into_body().into_reader();
    let mut buffer = [0u8; 8192];

    loop {
        match std::io::Read::read(&mut reader, &mut buffer) {
            Ok(0) => break, // EOF
            Ok(n) => {
                if let Err(e) = file.write_all(&buffer[..n]) {
                    return DownloadResult::Error(format!("Write error: {}", e));
                }
            }
            Err(e) => return DownloadResult::Error(format!("Read error: {}", e)),
        }
    }

    // Ensure all data is written
    if let Err(e) = file.flush() {
        return DownloadResult::Error(format!("Failed to flush file: {}", e));
    }

    DownloadResult::Success(download_path)
}

/// Result of the installation preparation
#[derive(Debug, Clone)]
pub enum InstallResult {
    /// Installation is ready - app should exit so the updater script can run
    ReadyToRestart { message: String },
    /// macOS: DMG opened, user needs to complete installation manually
    ManualInstallRequired { message: String },
    /// Error during installation
    Error(String),
}

/// Prepare and execute the update installation.
/// On Windows/Linux: Extracts the archive and creates an update script.
/// On macOS: Opens the DMG for manual installation.
pub fn install_update(archive_path: &std::path::Path) -> InstallResult {
    let platform = match Platform::current() {
        Some(p) => p,
        None => return InstallResult::Error("Unsupported platform".to_string()),
    };

    match platform {
        Platform::WindowsX64 => install_windows(archive_path),
        Platform::LinuxX64 => install_linux(archive_path),
        Platform::MacOSIntel | Platform::MacOSArm => install_macos(archive_path),
    }
}

/// Windows: Extract ZIP and create a batch script to replace the executable
#[cfg(target_os = "windows")]
fn install_windows(archive_path: &std::path::Path) -> InstallResult {
    use std::fs::File;

    // Get the current executable path
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return InstallResult::Error(format!("Failed to get current exe path: {}", e)),
    };

    let exe_dir = match current_exe.parent() {
        Some(d) => d,
        None => return InstallResult::Error("Failed to get exe directory".to_string()),
    };

    // Create extraction directory in temp
    let temp_dir = std::env::temp_dir();
    let extract_dir = temp_dir.join("ultralog-update");

    // Clean up any previous extraction
    let _ = std::fs::remove_dir_all(&extract_dir);
    if let Err(e) = std::fs::create_dir_all(&extract_dir) {
        return InstallResult::Error(format!("Failed to create extraction directory: {}", e));
    }

    // Open and extract ZIP
    let file = match File::open(archive_path) {
        Ok(f) => f,
        Err(e) => return InstallResult::Error(format!("Failed to open archive: {}", e)),
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => return InstallResult::Error(format!("Failed to read ZIP archive: {}", e)),
    };

    // Extract all files
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(e) => {
                return InstallResult::Error(format!("Failed to read file from archive: {}", e))
            }
        };

        let outpath = match file.enclosed_name() {
            Some(p) => extract_dir.join(p),
            None => continue,
        };

        if file.is_dir() {
            let _ = std::fs::create_dir_all(&outpath);
        } else {
            if let Some(parent) = outpath.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut outfile = match File::create(&outpath) {
                Ok(f) => f,
                Err(e) => return InstallResult::Error(format!("Failed to create file: {}", e)),
            };
            if let Err(e) = std::io::copy(&mut file, &mut outfile) {
                return InstallResult::Error(format!("Failed to extract file: {}", e));
            }
        }
    }

    // Find the new executable in extracted files
    let new_exe = extract_dir.join("ultralog.exe");
    if !new_exe.exists() {
        // Try looking in a subdirectory
        let entries: Vec<_> = std::fs::read_dir(&extract_dir)
            .ok()
            .map(|rd| rd.filter_map(|e| e.ok()).collect())
            .unwrap_or_default();

        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                let nested_exe = path.join("ultralog.exe");
                if nested_exe.exists() {
                    return create_windows_updater_script(&nested_exe, &current_exe, exe_dir);
                }
            }
        }
        return InstallResult::Error("Could not find ultralog.exe in extracted files".to_string());
    }

    create_windows_updater_script(&new_exe, &current_exe, exe_dir)
}

#[cfg(target_os = "windows")]
fn create_windows_updater_script(
    new_exe: &std::path::Path,
    current_exe: &std::path::Path,
    exe_dir: &std::path::Path,
) -> InstallResult {
    let script_path = std::env::temp_dir().join("ultralog_update.bat");

    // Create batch script that waits for the app to close, then replaces files
    let script_content = format!(
        r#"@echo off
echo UltraLog Updater
echo Waiting for application to close...
:wait
timeout /t 1 /nobreak >nul
tasklist /FI "IMAGENAME eq ultralog.exe" 2>NUL | find /I /N "ultralog.exe">NUL
if "%ERRORLEVEL%"=="0" goto wait

echo Updating UltraLog...
copy /Y "{new_exe}" "{target_exe}"
if errorlevel 1 (
    echo Update failed! Please try again or download manually.
    pause
    exit /b 1
)

echo Update complete! Starting UltraLog...
start "" "{target_exe}"
del "%~f0"
"#,
        new_exe = new_exe.display(),
        target_exe = current_exe.display(),
    );

    if let Err(e) = std::fs::write(&script_path, script_content) {
        return InstallResult::Error(format!("Failed to create update script: {}", e));
    }

    // Start the update script
    match std::process::Command::new("cmd")
        .args(["/C", "start", "", "/MIN", script_path.to_str().unwrap_or("")])
        .spawn()
    {
        Ok(_) => InstallResult::ReadyToRestart {
            message: "Update downloaded and ready. The application will now close to complete the update.".to_string(),
        },
        Err(e) => InstallResult::Error(format!("Failed to start update script: {}", e)),
    }
}

#[cfg(not(target_os = "windows"))]
fn install_windows(_archive_path: &std::path::Path) -> InstallResult {
    InstallResult::Error("Windows installation not supported on this platform".to_string())
}

/// Linux: Extract tar.gz and create a shell script to replace the executable
#[cfg(target_os = "linux")]
fn install_linux(archive_path: &std::path::Path) -> InstallResult {
    use flate2::read::GzDecoder;
    use std::fs::File;

    // Get the current executable path
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return InstallResult::Error(format!("Failed to get current exe path: {}", e)),
    };

    // Create extraction directory in temp
    let temp_dir = std::env::temp_dir();
    let extract_dir = temp_dir.join("ultralog-update");

    // Clean up any previous extraction
    let _ = std::fs::remove_dir_all(&extract_dir);
    if let Err(e) = std::fs::create_dir_all(&extract_dir) {
        return InstallResult::Error(format!("Failed to create extraction directory: {}", e));
    }

    // Open and extract tar.gz
    let file = match File::open(archive_path) {
        Ok(f) => f,
        Err(e) => return InstallResult::Error(format!("Failed to open archive: {}", e)),
    };

    let gz = GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);

    if let Err(e) = archive.unpack(&extract_dir) {
        return InstallResult::Error(format!("Failed to extract archive: {}", e));
    }

    // Find the new executable
    let new_exe = find_executable_in_dir(&extract_dir, "ultralog");
    let new_exe = match new_exe {
        Some(p) => p,
        None => {
            return InstallResult::Error(
                "Could not find ultralog executable in extracted files".to_string(),
            )
        }
    };

    // Create shell script to replace the executable
    let script_path = temp_dir.join("ultralog_update.sh");
    let script_content = format!(
        r#"#!/bin/bash
echo "UltraLog Updater"
echo "Waiting for application to close..."

# Wait for the application to exit
while pgrep -x "ultralog" > /dev/null; do
    sleep 1
done

echo "Updating UltraLog..."
cp "{new_exe}" "{target_exe}"
chmod +x "{target_exe}"

if [ $? -eq 0 ]; then
    echo "Update complete! Starting UltraLog..."
    nohup "{target_exe}" > /dev/null 2>&1 &
else
    echo "Update failed! Please try again or download manually."
fi

rm -- "$0"
"#,
        new_exe = new_exe.display(),
        target_exe = current_exe.display(),
    );

    if let Err(e) = std::fs::write(&script_path, &script_content) {
        return InstallResult::Error(format!("Failed to create update script: {}", e));
    }

    // Make script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
    }

    // Start the update script
    match std::process::Command::new("bash")
        .arg(&script_path)
        .spawn()
    {
        Ok(_) => InstallResult::ReadyToRestart {
            message: "Update downloaded and ready. The application will now close to complete the update.".to_string(),
        },
        Err(e) => InstallResult::Error(format!("Failed to start update script: {}", e)),
    }
}

#[cfg(not(target_os = "linux"))]
fn install_linux(_archive_path: &std::path::Path) -> InstallResult {
    InstallResult::Error("Linux installation not supported on this platform".to_string())
}

#[cfg(target_os = "linux")]
fn find_executable_in_dir(dir: &std::path::Path, name: &str) -> Option<PathBuf> {
    // First check directly in the directory
    let direct = dir.join(name);
    if direct.exists() && direct.is_file() {
        return Some(direct);
    }

    // Check subdirectories
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_executable_in_dir(&path, name) {
                    return Some(found);
                }
            } else if path.file_name().map(|n| n == name).unwrap_or(false) {
                return Some(path);
            }
        }
    }
    None
}

/// macOS: Open the DMG for manual installation
fn install_macos(archive_path: &std::path::Path) -> InstallResult {
    // On macOS, we open the DMG which will mount it
    // The user needs to drag the app to Applications
    match open::that(archive_path) {
        Ok(_) => InstallResult::ManualInstallRequired {
            message: "The update disk image has been opened. Please drag UltraLog to your Applications folder to complete the update, then restart the application.".to_string(),
        },
        Err(e) => InstallResult::Error(format!("Failed to open update file: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        // Should return Some on supported platforms
        let platform = Platform::current();

        #[cfg(any(
            all(target_os = "windows", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "linux", target_arch = "x86_64")
        ))]
        assert!(platform.is_some());

        if let Some(p) = platform {
            assert!(!p.asset_name().is_empty());
            assert!(!p.extension().is_empty());
        }
    }

    #[test]
    fn test_asset_names() {
        assert_eq!(Platform::WindowsX64.asset_name(), "ultralog-windows.zip");
        assert_eq!(
            Platform::MacOSIntel.asset_name(),
            "ultralog-macos-intel.dmg"
        );
        assert_eq!(Platform::MacOSArm.asset_name(), "ultralog-macos-arm64.dmg");
        assert_eq!(Platform::LinuxX64.asset_name(), "ultralog-linux.tar.gz");
    }
}
