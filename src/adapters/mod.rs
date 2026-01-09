//! OpenECU Alliance adapter integration module.
//!
//! This module provides integration with OpenECU Alliance adapter specifications,
//! enabling spec-driven channel normalization, metadata lookup, and format detection.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ultralog::adapters::{normalize_from_spec, get_channel_metadata};
//!
//! // Normalize a channel name using spec definitions
//! if let Some(normalized) = normalize_from_spec("Engine Speed") {
//!     println!("Normalized: {}", normalized); // "Engine RPM"
//! }
//!
//! // Get full channel metadata
//! if let Some(meta) = get_channel_metadata("TPS") {
//!     println!("Category: {}", meta.category.display_name());
//!     println!("Unit: {}", meta.unit);
//! }
//! ```

pub mod registry;
pub mod types;

// Re-export commonly used types and functions
pub use registry::{
    find_adapters_by_extension, find_protocols_by_vendor, get_adapter_by_id, get_adapters,
    get_adapters_by_vendor, get_all_categories, get_channel_metadata, get_channels_by_category,
    get_protocol_by_id, get_protocols, get_spec_normalizations, has_spec_normalization,
    normalize_from_spec, ChannelMetadata,
};
pub use types::{
    AdapterSpec, ByteOrder, ChannelCategory, ChannelSpec, DataType, EnumSpec, FileFormatSpec,
    MessageSpec, ProtocolInfo, ProtocolSpec, ProtocolType, SignalDataType, SignalSpec,
};
