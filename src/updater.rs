//! Auto-update functionality for UltraLog.
//!
//! Checks GitHub releases for new versions, downloads updates,
//! and handles installation with proper extraction and replacement.

use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;

const GITHUB_API_URL: &str = "https://api.github.com/repos/ClassicMiniDIY/UltraLog/releases/latest";
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

/// File names the installer accepts for the new binary inside a downloaded archive.
///
/// `release.yml` packages `ultralog.exe` in the Windows zip (since 3.1.1; up to 3.1.0 it was
/// `ultralog-windows.exe`) and `ultralog-linux` in the Linux tarball. The Linux name stays on
/// purpose: the 3.1.0 updater finds a binary named `ultralog`, but its helper script cannot wait
/// for an app run as `ultralog-linux`, so it would copy onto the running binary, fail, and relaunch
/// the old version in a loop with no error. See the Auto-Update Archive Contract in CLAUDE.md.
/// Matching is exact (case-insensitive on Windows): a looser rule could pick up an unrelated
/// file left in the extraction directory.
#[cfg(any(target_os = "windows", target_os = "linux", test))]
fn update_binary_names(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::WindowsX64 => &[
            "ultralog.exe",
            "ultralog-windows.exe",
            "ultralog-windows-beta.exe",
        ],
        Platform::LinuxX64 => &["ultralog-linux", "ultralog-linux-beta", "ultralog"],
        Platform::MacOSIntel | Platform::MacOSArm => &[],
    }
}

/// Windows file names are case-insensitive; Linux ones are not.
#[cfg(any(target_os = "windows", target_os = "linux", test))]
fn name_matches(file_name: &str, accepted: &str, platform: Platform) -> bool {
    match platform {
        Platform::WindowsX64 => file_name.eq_ignore_ascii_case(accepted),
        _ => file_name == accepted,
    }
}

#[cfg(test)]
fn is_update_binary_name(file_name: &str, platform: Platform) -> bool {
    update_binary_names(platform)
        .iter()
        .any(|n| name_matches(file_name, n, platform))
}

/// Find the new binary in an extraction directory: the directory itself first, then up to
/// two levels of subdirectories. The first accepted name in `update_binary_names` order wins
/// within one directory, so the packaged name beats a stray file with an older name.
#[cfg(any(target_os = "windows", target_os = "linux", test))]
fn find_update_binary(dir: &std::path::Path, platform: Platform) -> Option<PathBuf> {
    fn search(dir: &std::path::Path, platform: Platform, depth: u8) -> Option<PathBuf> {
        let entries: Vec<_> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        for name in update_binary_names(platform) {
            if let Some(found) = entries.iter().find(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| name_matches(n, name, platform))
            }) {
                return Some(found.clone());
            }
        }
        if depth == 0 {
            return None;
        }
        let mut dirs: Vec<_> = entries.into_iter().filter(|p| p.is_dir()).collect();
        dirs.sort();
        dirs.iter().find_map(|d| search(d, platform, depth - 1))
    }
    search(dir, platform, 2)
}

/// Environment variables that carry the paths into the Windows update script.
#[cfg(any(target_os = "windows", test))]
const WIN_ENV_NEW_EXE: &str = "ULTRALOG_UPDATE_NEW_EXE";
#[cfg(any(target_os = "windows", test))]
const WIN_ENV_TARGET_EXE: &str = "ULTRALOG_UPDATE_TARGET_EXE";

/// Batch script that waits for this process to exit, copies the new binary over the old one and
/// restarts it.
///
/// - It waits by PID, not by image name: users run the binary under whatever name they extracted
///   or renamed it to. The wait gives up after 60 s (a reused PID must not block it forever); the
///   copy retries cover a target that is still locked.
/// - The paths arrive in environment variables (`WIN_ENV_*`), never in the script text: cmd.exe
///   reads a batch file in the OEM code page, so a UTF-8 path such as `C:\Users\José\...` would
///   turn into mojibake, and a `%` in a path would be expanded.
#[cfg(any(target_os = "windows", test))]
fn windows_update_script(pid: u32) -> String {
    format!(
        r#"@echo off
echo UltraLog Updater
echo Waiting for application to close...
set waits=0
:wait
timeout /t 1 /nobreak >nul
set /a waits+=1
if %waits% GEQ 60 goto update
tasklist /FI "PID eq {pid}" /NH 2>NUL | find "{pid}" >NUL
if "%ERRORLEVEL%"=="0" goto wait

:update
echo Updating UltraLog...
set tries=0
:copy
copy /Y "%{new}%" "%{target}%" >NUL
if not errorlevel 1 goto done
set /a tries+=1
if %tries% GEQ 10 (
    echo Update failed! Please try again or download manually.
    pause
    exit /b 1
)
timeout /t 1 /nobreak >nul
goto copy

:done
echo Update complete! Starting UltraLog...
start "" "%{target}%"
del "%~f0"
"#,
        new = WIN_ENV_NEW_EXE,
        target = WIN_ENV_TARGET_EXE,
    )
}

/// Shell script, run as `bash script NEW_EXE TARGET_EXE`, that waits for this process (by PID) to
/// exit, swaps the new binary in and starts the app again.
///
/// - The paths are arguments, never script text, so `$`, backticks or quotes in a path are safe.
/// - The swap is copy-to-`TARGET.new` then `mv`: atomic, no ETXTBSY, and a failed copy (full
///   disk, no write access to the directory) leaves the old binary intact.
/// - The app is relaunched either way, so a failed update still leaves UltraLog running.
#[cfg(any(target_os = "linux", test))]
fn linux_update_script(pid: u32) -> String {
    format!(
        r#"#!/bin/bash
NEW_EXE="$1"
TARGET_EXE="$2"
echo "UltraLog Updater"
echo "Waiting for application to close..."

# Wait for the application (this PID) to exit
while kill -0 {pid} 2>/dev/null; do
    sleep 1
done

echo "Updating UltraLog..."
if cp "$NEW_EXE" "$TARGET_EXE.new" && chmod +x "$TARGET_EXE.new" && mv -f "$TARGET_EXE.new" "$TARGET_EXE"; then
    echo "Update complete! Starting UltraLog..."
else
    rm -f "$TARGET_EXE.new"
    echo "Update failed! Please try again or download manually."
fi
nohup "$TARGET_EXE" > /dev/null 2>&1 &

rm -- "$0"
"#
    )
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
                return InstallResult::Error(format!("Failed to read file from archive: {}", e));
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
    let Some(new_exe) = find_update_binary(&extract_dir, Platform::WindowsX64) else {
        return InstallResult::Error("Could not find ultralog.exe in extracted files".to_string());
    };

    create_windows_updater_script(&new_exe, &current_exe)
}

#[cfg(target_os = "windows")]
fn create_windows_updater_script(
    new_exe: &std::path::Path,
    current_exe: &std::path::Path,
) -> InstallResult {
    let script_path = std::env::temp_dir().join("ultralog_update.bat");
    let script_content = windows_update_script(std::process::id());

    if let Err(e) = std::fs::write(&script_path, script_content) {
        return InstallResult::Error(format!("Failed to create update script: {}", e));
    }

    // Start the update script
    match std::process::Command::new("cmd")
        .args(["/C", "start", "", "/MIN", script_path.to_str().unwrap_or("")])
        .env(WIN_ENV_NEW_EXE, new_exe)
        .env(WIN_ENV_TARGET_EXE, current_exe)
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
    let Some(new_exe) = find_update_binary(&extract_dir, Platform::LinuxX64) else {
        return InstallResult::Error(
            "Could not find ultralog executable in extracted files".to_string(),
        );
    };

    // Create shell script to replace the executable
    let script_path = temp_dir.join("ultralog_update.sh");
    let script_content = linux_update_script(std::process::id());

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
        .arg(&new_exe)
        .arg(&current_exe)
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

    /// A unique scratch directory under the system temp dir, removed on drop.
    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!(
                "ultralog-updater-test-{tag}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            ScratchDir(dir)
        }

        fn touch(&self, rel: &str) -> PathBuf {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"binary").unwrap();
            path
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_update_binary_names_cover_release_packaging() {
        // What release.yml packages now, and what it packaged up to 3.1.0 (stable and beta).
        for name in [
            "ultralog.exe",
            "ultralog-windows.exe",
            "ultralog-windows-beta.exe",
            "UltraLog.EXE",
        ] {
            assert!(is_update_binary_name(name, Platform::WindowsX64), "{name}");
        }
        for name in ["ultralog", "ultralog-linux", "ultralog-linux-beta"] {
            assert!(is_update_binary_name(name, Platform::LinuxX64), "{name}");
        }
    }

    #[test]
    fn test_update_binary_names_reject_other_files() {
        for name in [
            "ultralog-windows.zip",
            "ultralog",
            "readme.txt",
            "ultralog.exe.bak",
        ] {
            assert!(!is_update_binary_name(name, Platform::WindowsX64), "{name}");
        }
        for name in [
            "ultralog-linux.tar.gz",
            "ultralog.exe",
            "UltraLog",
            "ultralog-update.sh",
        ] {
            assert!(!is_update_binary_name(name, Platform::LinuxX64), "{name}");
        }
        assert!(!is_update_binary_name("ultralog", Platform::MacOSArm));
    }

    #[test]
    fn test_find_update_binary_new_and_legacy_layouts() {
        let new_win = ScratchDir::new("win-new");
        let exe = new_win.touch("ultralog.exe");
        assert_eq!(
            find_update_binary(&new_win.0, Platform::WindowsX64),
            Some(exe)
        );

        // The 3.1.0 zip: one entry named ultralog-windows.exe, which the old lookup missed.
        let legacy_win = ScratchDir::new("win-legacy");
        let exe = legacy_win.touch("ultralog-windows.exe");
        assert_eq!(
            find_update_binary(&legacy_win.0, Platform::WindowsX64),
            Some(exe)
        );

        let legacy_linux = ScratchDir::new("linux-legacy");
        let bin = legacy_linux.touch("ultralog-linux");
        assert_eq!(
            find_update_binary(&legacy_linux.0, Platform::LinuxX64),
            Some(bin)
        );
    }

    #[test]
    fn test_find_update_binary_prefers_canonical_name_and_searches_subdirs() {
        let both = ScratchDir::new("both");
        both.touch("ultralog-windows.exe");
        let canonical = both.touch("ultralog.exe");
        assert_eq!(
            find_update_binary(&both.0, Platform::WindowsX64),
            Some(canonical)
        );

        let nested = ScratchDir::new("nested");
        nested.touch("README.txt");
        let bin = nested.touch("ultralog-3.1.1/ultralog");
        assert_eq!(find_update_binary(&nested.0, Platform::LinuxX64), Some(bin));

        let empty = ScratchDir::new("empty");
        empty.touch("notes.txt");
        assert_eq!(find_update_binary(&empty.0, Platform::LinuxX64), None);
    }

    #[test]
    fn test_windows_script_waits_for_pid_and_reads_paths_from_env() {
        let bat = windows_update_script(4242);
        assert!(bat.contains(r#"tasklist /FI "PID eq 4242""#));
        assert!(!bat.contains("IMAGENAME"));
        assert!(bat.contains("if %waits% GEQ 60 goto update"));
        assert!(bat.contains(&format!(
            r#"copy /Y "%{WIN_ENV_NEW_EXE}%" "%{WIN_ENV_TARGET_EXE}%""#
        )));
        assert!(bat.contains(&format!(r#"start "" "%{WIN_ENV_TARGET_EXE}%""#)));
        // cmd.exe reads the file in the OEM code page: keep the script itself ASCII.
        assert!(bat.is_ascii());
    }

    #[test]
    fn test_linux_script_waits_for_pid_and_swaps_atomically() {
        let sh = linux_update_script(4242);
        assert!(sh.contains("while kill -0 4242 2>/dev/null"));
        assert!(!sh.contains("pgrep"));
        assert!(sh.contains(r#"NEW_EXE="$1""#) && sh.contains(r#"TARGET_EXE="$2""#));
        assert!(sh.contains(
            r#"if cp "$NEW_EXE" "$TARGET_EXE.new" && chmod +x "$TARGET_EXE.new" && mv -f "$TARGET_EXE.new" "$TARGET_EXE"; then"#
        ));
        // Relaunch outside the if: a failed update still leaves the app running.
        let relaunch = sh.find(r#"nohup "$TARGET_EXE""#).unwrap();
        assert!(relaunch > sh.find("fi\n").unwrap());
    }

    /// Runs the real Linux script against a fake app: it must wait for the PID, swap the binary
    /// and leave no `.new` behind. Unix only (needs bash, sleep, kill).
    #[cfg(unix)]
    #[test]
    fn test_linux_script_swaps_binary_after_pid_exits() {
        let dir = ScratchDir::new("script-run");
        let new_exe = dir.touch("new/ultralog-linux");
        std::fs::write(&new_exe, b"new").unwrap();
        let target = dir.touch("install dir/ultralog $weird");
        std::fs::write(&target, b"old").unwrap();

        // The fake app is reaped on its own thread: an unreaped child stays a zombie, and
        // `kill -0` succeeds on a zombie, so the script would wait forever.
        let mut app = std::process::Command::new("sleep")
            .arg("1")
            .spawn()
            .unwrap();
        let pid = app.id();
        let reaper = std::thread::spawn(move || app.wait());
        // The script relaunches TARGET; make it a harmless shell script.
        std::fs::write(&new_exe, b"#!/bin/sh\nexit 0\n").unwrap();
        let script = dir.0.join("update.sh");
        std::fs::write(&script, linux_update_script(pid)).unwrap();
        let status = std::process::Command::new("bash")
            .arg(&script)
            .arg(&new_exe)
            .arg(&target)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        let _ = reaper.join();
        assert!(status.success());
        assert_eq!(std::fs::read(&target).unwrap(), b"#!/bin/sh\nexit 0\n");
        assert!(!dir.0.join("install dir/ultralog $weird.new").exists());
        assert!(!script.exists(), "script removes itself");
    }
}
