//! API client for fetching adapter and protocol specs from OpenECUAlliance.
//!
//! This module provides functions to fetch specs from the OpenECUAlliance API
//! at runtime, enabling dynamic spec updates without recompiling.

use serde::Deserialize;
use thiserror::Error;

use super::types::{AdapterSpec, ProtocolSpec};

/// Base URL for the OpenECUAlliance API
const OECUA_API_BASE: &str = "https://openecualliance.org";

/// User agent for API requests
const USER_AGENT: &str = concat!("UltraLog/", env!("CARGO_PKG_VERSION"));

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur when fetching specs from the API
#[derive(Debug, Error)]
pub enum ApiError {
    /// Network error during request
    #[error("Network error: {0}")]
    NetworkError(String),

    /// API returned an error response
    #[error("API error (status {status}): {message}")]
    ApiResponseError { status: u16, message: String },

    /// Failed to parse API response
    #[error("Parse error: {0}")]
    ParseError(String),
}

// ============================================================================
// API Response Types
// ============================================================================

/// Summary of an adapter from the list endpoint
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub vendor: String,
    pub channel_count: u32,
    #[serde(default)]
    pub description: Option<String>,
}

/// Summary of a protocol from the list endpoint
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub vendor: String,
    pub message_count: u32,
    #[serde(default)]
    pub description: Option<String>,
}

/// Response wrapper for list endpoints
#[derive(Debug, Clone, Deserialize)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub count: Option<u32>,
}

// ============================================================================
// API Client Functions
// ============================================================================

/// Fetch list of all adapters (summary only)
pub fn fetch_adapter_list() -> Result<Vec<AdapterSummary>, ApiError> {
    let url = format!("{}/api/adapters", OECUA_API_BASE);

    let mut response = ureq::get(&url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(status) => ApiError::ApiResponseError {
                status,
                message: format!("HTTP {}", status),
            },
            _ => ApiError::NetworkError(e.to_string()),
        })?;

    let list: ListResponse<AdapterSummary> = response
        .body_mut()
        .read_json()
        .map_err(|e| ApiError::ParseError(e.to_string()))?;

    Ok(list.data)
}

/// Fetch a single adapter with full details
pub fn fetch_adapter(vendor: &str, id: &str) -> Result<AdapterSpec, ApiError> {
    let url = format!("{}/api/adapters/{}/{}", OECUA_API_BASE, vendor, id);

    let mut response = ureq::get(&url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(status) => ApiError::ApiResponseError {
                status,
                message: format!("HTTP {}", status),
            },
            _ => ApiError::NetworkError(e.to_string()),
        })?;

    response
        .body_mut()
        .read_json()
        .map_err(|e| ApiError::ParseError(e.to_string()))
}

/// Fetch all adapters with full details
/// This fetches the list first, then fetches each adapter individually
pub fn fetch_all_adapters() -> Result<Vec<AdapterSpec>, ApiError> {
    let summaries = fetch_adapter_list()?;

    let mut adapters = Vec::with_capacity(summaries.len());
    for summary in summaries {
        match fetch_adapter(&summary.vendor, &summary.id) {
            Ok(adapter) => adapters.push(adapter),
            Err(e) => {
                tracing::warn!(
                    "Failed to fetch adapter {}/{}: {}",
                    summary.vendor,
                    summary.id,
                    e
                );
                // Continue with other adapters
            }
        }
    }

    Ok(adapters)
}

/// Fetch list of all protocols (summary only)
pub fn fetch_protocol_list() -> Result<Vec<ProtocolSummary>, ApiError> {
    let url = format!("{}/api/protocols", OECUA_API_BASE);

    let mut response = ureq::get(&url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(status) => ApiError::ApiResponseError {
                status,
                message: format!("HTTP {}", status),
            },
            _ => ApiError::NetworkError(e.to_string()),
        })?;

    let list: ListResponse<ProtocolSummary> = response
        .body_mut()
        .read_json()
        .map_err(|e| ApiError::ParseError(e.to_string()))?;

    Ok(list.data)
}

/// Fetch a single protocol with full details
pub fn fetch_protocol(vendor: &str, id: &str) -> Result<ProtocolSpec, ApiError> {
    let url = format!("{}/api/protocols/{}/{}", OECUA_API_BASE, vendor, id);

    let mut response = ureq::get(&url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(status) => ApiError::ApiResponseError {
                status,
                message: format!("HTTP {}", status),
            },
            _ => ApiError::NetworkError(e.to_string()),
        })?;

    response
        .body_mut()
        .read_json()
        .map_err(|e| ApiError::ParseError(e.to_string()))
}

/// Fetch all protocols with full details
/// This fetches the list first, then fetches each protocol individually
pub fn fetch_all_protocols() -> Result<Vec<ProtocolSpec>, ApiError> {
    let summaries = fetch_protocol_list()?;

    let mut protocols = Vec::with_capacity(summaries.len());
    for summary in summaries {
        match fetch_protocol(&summary.vendor, &summary.id) {
            Ok(protocol) => protocols.push(protocol),
            Err(e) => {
                tracing::warn!(
                    "Failed to fetch protocol {}/{}: {}",
                    summary.vendor,
                    summary.id,
                    e
                );
                // Continue with other protocols
            }
        }
    }

    Ok(protocols)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require network access and a running API server
    // They are marked as ignored by default for CI/CD pipelines

    #[test]
    #[ignore]
    fn test_fetch_adapter_list() {
        let result = fetch_adapter_list();
        assert!(result.is_ok(), "Failed to fetch adapter list: {:?}", result);

        let adapters = result.unwrap();
        assert!(!adapters.is_empty(), "Adapter list should not be empty");
    }

    #[test]
    #[ignore]
    fn test_fetch_protocol_list() {
        let result = fetch_protocol_list();
        assert!(
            result.is_ok(),
            "Failed to fetch protocol list: {:?}",
            result
        );

        let protocols = result.unwrap();
        assert!(!protocols.is_empty(), "Protocol list should not be empty");
    }
}
