//! Disk cache for OpenECUAlliance adapter and protocol specs.
//!
//! This module provides local caching of specs fetched from the API,
//! reducing network requests and providing offline access.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use thiserror::Error;

use super::types::{AdapterSpec, ProtocolSpec};

/// Cache directory name within app data
const CACHE_DIR_NAME: &str = "oecua_specs";

/// Adapters subdirectory
const ADAPTERS_DIR: &str = "adapters";

/// Protocols subdirectory
const PROTOCOLS_DIR: &str = "protocols";

/// Metadata file name
const METADATA_FILE: &str = "metadata.json";

/// Default cache staleness threshold (24 hours)
const DEFAULT_CACHE_MAX_AGE_SECS: u64 = 24 * 60 * 60;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during cache operations
#[derive(Debug, Error)]
pub enum CacheError {
    /// Failed to create cache directory
    #[error("Failed to create cache directory: {0}")]
    CreateDirError(String),

    /// Failed to read cache file
    #[error("Failed to read cache file: {0}")]
    ReadError(String),

    /// Failed to write cache file
    #[error("Failed to write cache file: {0}")]
    WriteError(String),

    /// Failed to parse cached data
    #[error("Failed to parse cached data: {0}")]
    ParseError(String),

    /// Cache directory not found
    #[error("Cache directory not available")]
    NoCacheDir,
}

// ============================================================================
// Cache Metadata
// ============================================================================

/// Metadata about the cache state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// Timestamp of last successful fetch (Unix epoch seconds)
    pub last_fetch_timestamp: u64,

    /// Version of the cache format
    pub cache_version: u32,

    /// Number of cached adapters
    pub adapter_count: u32,

    /// Number of cached protocols
    pub protocol_count: u32,
}

impl Default for CacheMetadata {
    fn default() -> Self {
        Self {
            last_fetch_timestamp: 0,
            cache_version: 1,
            adapter_count: 0,
            protocol_count: 0,
        }
    }
}

// ============================================================================
// Cache Directory Functions
// ============================================================================

/// Get the cache directory path
/// Returns None if the app data directory cannot be determined
pub fn get_cache_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|base| base.join("UltraLog").join(CACHE_DIR_NAME))
}

/// Ensure the cache directory structure exists
fn ensure_cache_dirs() -> Result<PathBuf, CacheError> {
    let cache_dir = get_cache_dir().ok_or(CacheError::NoCacheDir)?;

    let adapters_dir = cache_dir.join(ADAPTERS_DIR);
    let protocols_dir = cache_dir.join(PROTOCOLS_DIR);

    fs::create_dir_all(&adapters_dir)
        .map_err(|e| CacheError::CreateDirError(format!("adapters: {}", e)))?;

    fs::create_dir_all(&protocols_dir)
        .map_err(|e| CacheError::CreateDirError(format!("protocols: {}", e)))?;

    Ok(cache_dir)
}

// ============================================================================
// Metadata Operations
// ============================================================================

/// Load cache metadata
pub fn load_metadata() -> Option<CacheMetadata> {
    let cache_dir = get_cache_dir()?;
    let metadata_path = cache_dir.join(METADATA_FILE);

    let content = fs::read_to_string(&metadata_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save cache metadata
fn save_metadata(metadata: &CacheMetadata) -> Result<(), CacheError> {
    let cache_dir = ensure_cache_dirs()?;
    let metadata_path = cache_dir.join(METADATA_FILE);

    let content = serde_json::to_string_pretty(metadata)
        .map_err(|e| CacheError::WriteError(e.to_string()))?;

    fs::write(&metadata_path, content).map_err(|e| CacheError::WriteError(e.to_string()))
}

/// Check if the cache is stale (older than max age)
pub fn is_cache_stale() -> bool {
    is_cache_stale_with_max_age(DEFAULT_CACHE_MAX_AGE_SECS)
}

/// Check if the cache is stale with a custom max age
pub fn is_cache_stale_with_max_age(max_age_secs: u64) -> bool {
    let Some(metadata) = load_metadata() else {
        return true; // No metadata means cache is stale
    };

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let age = now.saturating_sub(metadata.last_fetch_timestamp);
    age > max_age_secs
}

/// Get the age of the cache in seconds, or None if no cache
pub fn get_cache_age() -> Option<Duration> {
    let metadata = load_metadata()?;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?;

    let fetch_time = Duration::from_secs(metadata.last_fetch_timestamp);
    now.checked_sub(fetch_time)
}

// ============================================================================
// Adapter Cache Operations
// ============================================================================

/// Load all cached adapters
pub fn load_cached_adapters() -> Option<Vec<AdapterSpec>> {
    let cache_dir = get_cache_dir()?;
    let adapters_dir = cache_dir.join(ADAPTERS_DIR);

    if !adapters_dir.exists() {
        return None;
    }

    let mut adapters = Vec::new();

    let entries = fs::read_dir(&adapters_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(adapter) = serde_json::from_str::<AdapterSpec>(&content) {
                    adapters.push(adapter);
                } else {
                    tracing::warn!("Failed to parse cached adapter: {:?}", path);
                }
            }
        }
    }

    if adapters.is_empty() {
        None
    } else {
        Some(adapters)
    }
}

/// Save adapters to cache
pub fn save_adapters_to_cache(adapters: &[AdapterSpec]) -> Result<(), CacheError> {
    let cache_dir = ensure_cache_dirs()?;
    let adapters_dir = cache_dir.join(ADAPTERS_DIR);

    // Clear existing cached adapters
    if let Ok(entries) = fs::read_dir(&adapters_dir) {
        for entry in entries.flatten() {
            let _ = fs::remove_file(entry.path());
        }
    }

    // Save each adapter
    for adapter in adapters {
        let filename = format!("{}-{}.json", adapter.vendor, adapter.id);
        let path = adapters_dir.join(&filename);

        let content = serde_json::to_string_pretty(adapter)
            .map_err(|e| CacheError::WriteError(format!("{}: {}", filename, e)))?;

        fs::write(&path, content)
            .map_err(|e| CacheError::WriteError(format!("{}: {}", filename, e)))?;
    }

    // Update metadata
    let mut metadata = load_metadata().unwrap_or_default();
    metadata.adapter_count = adapters.len() as u32;
    metadata.last_fetch_timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    save_metadata(&metadata)?;

    Ok(())
}

// ============================================================================
// Protocol Cache Operations
// ============================================================================

/// Load all cached protocols
pub fn load_cached_protocols() -> Option<Vec<ProtocolSpec>> {
    let cache_dir = get_cache_dir()?;
    let protocols_dir = cache_dir.join(PROTOCOLS_DIR);

    if !protocols_dir.exists() {
        return None;
    }

    let mut protocols = Vec::new();

    let entries = fs::read_dir(&protocols_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(protocol) = serde_json::from_str::<ProtocolSpec>(&content) {
                    protocols.push(protocol);
                } else {
                    tracing::warn!("Failed to parse cached protocol: {:?}", path);
                }
            }
        }
    }

    if protocols.is_empty() {
        None
    } else {
        Some(protocols)
    }
}

/// Save protocols to cache
pub fn save_protocols_to_cache(protocols: &[ProtocolSpec]) -> Result<(), CacheError> {
    let cache_dir = ensure_cache_dirs()?;
    let protocols_dir = cache_dir.join(PROTOCOLS_DIR);

    // Clear existing cached protocols
    if let Ok(entries) = fs::read_dir(&protocols_dir) {
        for entry in entries.flatten() {
            let _ = fs::remove_file(entry.path());
        }
    }

    // Save each protocol
    for protocol in protocols {
        let filename = format!("{}-{}.json", protocol.vendor, protocol.id);
        let path = protocols_dir.join(&filename);

        let content = serde_json::to_string_pretty(protocol)
            .map_err(|e| CacheError::WriteError(format!("{}: {}", filename, e)))?;

        fs::write(&path, content)
            .map_err(|e| CacheError::WriteError(format!("{}: {}", filename, e)))?;
    }

    // Update metadata
    let mut metadata = load_metadata().unwrap_or_default();
    metadata.protocol_count = protocols.len() as u32;
    metadata.last_fetch_timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    save_metadata(&metadata)?;

    Ok(())
}

/// Clear the entire cache
pub fn clear_cache() -> Result<(), CacheError> {
    let cache_dir = get_cache_dir().ok_or(CacheError::NoCacheDir)?;

    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)
            .map_err(|e| CacheError::WriteError(format!("Failed to clear cache: {}", e)))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cache_dir() {
        let cache_dir = get_cache_dir();
        assert!(cache_dir.is_some(), "Cache directory should be available");

        let path = cache_dir.unwrap();
        assert!(
            path.to_string_lossy().contains("UltraLog"),
            "Cache path should contain UltraLog"
        );
        assert!(
            path.to_string_lossy().contains("oecua_specs"),
            "Cache path should contain oecua_specs"
        );
    }

    #[test]
    fn test_cache_staleness_with_no_metadata() {
        // With no metadata file, cache should be considered stale
        // This test doesn't create any files, so it checks default behavior
        // Note: This may pass or fail depending on whether metadata exists from other runs
        let stale = is_cache_stale_with_max_age(0);
        assert!(stale, "Cache with no metadata should be stale");
    }
}
