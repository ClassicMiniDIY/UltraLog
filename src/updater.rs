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
    /// The filename of the release asset (needed for Windows installer versioned filenames)
    pub asset_filename: String,
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
            Some(Platform::MacOSArm)
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

    /// Get the expected asset filename prefix for this platform.
    /// For Windows and Linux, returns a prefix since the filename includes version.
    /// For macOS, returns the exact filename.
    pub fn asset_name(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "ultralog-windows-setup", // Prefix, actual name includes version
            Platform::MacOSIntel => "ultralog-macos-intel.dmg",
            Platform::MacOSArm => "ultralog-macos-arm64.dmg",
            Platform::LinuxX64 => "UltraLog-", // Prefix for AppImage (e.g., UltraLog-1.2.3-x86_64.AppImage)
        }
    }

    /// Get the file extension for downloaded asset
    pub fn extension(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "exe",
            Platform::MacOSIntel | Platform::MacOSArm => "dmg",
            Platform::LinuxX64 => "AppImage",
        }
    }

    /// Whether this platform uses prefix matching for asset detection
    /// (Windows installer and Linux AppImage include version in filename)
    pub fn uses_prefix_matching(&self) -> bool {
        matches!(self, Platform::WindowsX64 | Platform::LinuxX64)
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

    // For Windows, use prefix matching since the filename includes version
    // (e.g., "ultralog-windows-setup-1.2.3.exe")
    // For other platforms, use exact matching
    let asset = if platform.uses_prefix_matching() {
        release.assets.iter().find(|a| {
            a.name.starts_with(asset_name)
                && a.name.ends_with(&format!(".{}", platform.extension()))
        })
    } else {
        release.assets.iter().find(|a| a.name == asset_name)
    };

    let asset = match asset {
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
        asset_filename: asset.name.clone(),
    })
}

/// Download update file to temp directory.
/// This is a blocking operation - run in a background thread.
///
/// # Arguments
/// * `url` - The download URL for the update
/// * `asset_filename` - The original filename of the asset (used for Windows installer)
pub fn download_update(url: &str, asset_filename: &str) -> DownloadResult {
    let platform = match Platform::current() {
        Some(p) => p,
        None => return DownloadResult::Error("Unsupported platform".to_string()),
    };

    // Create temp file path
    // For Windows installer, preserve the original filename (includes version)
    // For other platforms, use a generic name
    let temp_dir = std::env::temp_dir();
    let filename = if platform.uses_prefix_matching() {
        asset_filename.to_string()
    } else {
        format!("ultralog-update.{}", platform.extension())
    };
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

/// Windows: Run the installer silently to update the application
#[cfg(target_os = "windows")]
fn install_windows(installer_path: &std::path::Path) -> InstallResult {
    // Get the current executable path to determine installation directory
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return InstallResult::Error(format!("Failed to get current exe path: {}", e)),
    };

    let exe_dir = match current_exe.parent() {
        Some(d) => d,
        None => return InstallResult::Error("Failed to get exe directory".to_string()),
    };

    // Create a batch script that:
    // 1. Waits for UltraLog to close
    // 2. Runs the installer silently
    // 3. Cleans up the installer and script
    let script_path = std::env::temp_dir().join("ultralog_update.bat");

    let script_content = format!(
        r#"@echo off
echo UltraLog Updater
echo Waiting for application to close...

:wait
timeout /t 1 /nobreak >nul
tasklist /FI "IMAGENAME eq ultralog.exe" 2>NUL | find /I /N "ultralog.exe">NUL
if "%ERRORLEVEL%"=="0" goto wait

echo Installing update...

REM Run installer silently
REM /VERYSILENT - No UI at all
REM /SUPPRESSMSGBOXES - Suppress message boxes
REM /NORESTART - Don't restart automatically
REM /CLOSEAPPLICATIONS - Close applications using files being installed
REM /DIR="..." - Install to the same directory as current installation
"{installer}" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /DIR="{install_dir}"

if errorlevel 1 (
    echo Update failed! Error code: %ERRORLEVEL%
    echo Please download the update manually from GitHub.
    pause
    exit /b 1
)

echo Update complete!

REM Clean up installer
del /q "{installer}" 2>nul

REM Start UltraLog
start "" "{install_dir}\ultralog.exe"

REM Clean up this script
del "%~f0"
"#,
        installer = installer_path.display(),
        install_dir = exe_dir.display(),
    );

    if let Err(e) = std::fs::write(&script_path, script_content) {
        return InstallResult::Error(format!("Failed to create update script: {}", e));
    }

    // Start the update script in a minimized window
    match std::process::Command::new("cmd")
        .args([
            "/C",
            "start",
            "",
            "/MIN",
            script_path.to_str().unwrap_or(""),
        ])
        .spawn()
    {
        Ok(_) => InstallResult::ReadyToRestart {
            message:
                "Update downloaded and ready. The application will now close to install the update."
                    .to_string(),
        },
        Err(e) => InstallResult::Error(format!("Failed to start update script: {}", e)),
    }
}

#[cfg(not(target_os = "windows"))]
fn install_windows(_archive_path: &std::path::Path) -> InstallResult {
    InstallResult::Error("Windows installation not supported on this platform".to_string())
}

/// Linux: Replace AppImage with new version
#[cfg(target_os = "linux")]
fn install_linux(appimage_path: &std::path::Path) -> InstallResult {
    // Get the current executable path
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return InstallResult::Error(format!("Failed to get current exe path: {}", e)),
    };

    let temp_dir = std::env::temp_dir();

    // Create shell script to replace the AppImage
    // AppImage is a single executable file, so we just need to replace it
    let script_path = temp_dir.join("ultralog_update.sh");
    let script_content = format!(
        r#"#!/bin/bash
echo "UltraLog Updater"
echo "Waiting for application to close..."

# Wait for the application to exit (check for both ultralog and AppImage name)
while pgrep -f "ultralog|UltraLog.*AppImage" > /dev/null; do
    sleep 1
done

echo "Updating UltraLog..."

# Replace the AppImage
cp "{new_appimage}" "{target_exe}"
chmod +x "{target_exe}"

if [ $? -eq 0 ]; then
    echo "Update complete!"

    # Clean up the downloaded AppImage
    rm -f "{new_appimage}"

    echo "Starting UltraLog..."
    nohup "{target_exe}" > /dev/null 2>&1 &
else
    echo "Update failed! Please try again or download manually."
fi

# Clean up this script
rm -- "$0"
"#,
        new_appimage = appimage_path.display(),
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
    match std::process::Command::new("bash").arg(&script_path).spawn() {
        Ok(_) => InstallResult::ReadyToRestart {
            message:
                "Update downloaded and ready. The application will now close to install the update."
                    .to_string(),
        },
        Err(e) => InstallResult::Error(format!("Failed to start update script: {}", e)),
    }
}

#[cfg(not(target_os = "linux"))]
fn install_linux(_archive_path: &std::path::Path) -> InstallResult {
    InstallResult::Error("Linux installation not supported on this platform".to_string())
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
        // Windows uses prefix matching since filename includes version
        assert_eq!(Platform::WindowsX64.asset_name(), "ultralog-windows-setup");
        assert_eq!(Platform::WindowsX64.extension(), "exe");
        assert!(Platform::WindowsX64.uses_prefix_matching());

        // macOS uses exact matching
        assert_eq!(
            Platform::MacOSIntel.asset_name(),
            "ultralog-macos-intel.dmg"
        );
        assert_eq!(Platform::MacOSArm.asset_name(), "ultralog-macos-arm64.dmg");
        assert!(!Platform::MacOSIntel.uses_prefix_matching());
        assert!(!Platform::MacOSArm.uses_prefix_matching());

        // Linux uses prefix matching for AppImage (filename includes version)
        assert_eq!(Platform::LinuxX64.asset_name(), "UltraLog-");
        assert_eq!(Platform::LinuxX64.extension(), "AppImage");
        assert!(Platform::LinuxX64.uses_prefix_matching());
    }
}
