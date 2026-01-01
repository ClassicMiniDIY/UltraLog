//! Parser integration tests organized by ECU type
//!
//! Each ECU format has its own test module with comprehensive tests for:
//! - Format detection
//! - Parsing of real example files
//! - Edge cases and error handling
//! - Data integrity validation

pub mod aim_tests;
pub mod ecumaster_tests;
pub mod emerald_tests;
pub mod format_detection_tests;
pub mod haltech_tests;
pub mod link_tests;
pub mod romraider_tests;
pub mod speeduino_tests;
