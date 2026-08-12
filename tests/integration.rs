//! Integration tests for end-to-end functionality
//!
//! Tests for complete file loading cycles, format detection,
//! and cross-format data integrity.

// `tests/common/mod.rs` is intentionally re-included by each submodule via
// `#[path]`, so clippy sees it loaded more than once per test binary. Switching
// the submodules to `use crate::common::*` would be the structural fix.
#![allow(clippy::duplicate_mod)]
#[path = "common/mod.rs"]
mod common;

#[path = "integration/mod.rs"]
mod integration_tests;
