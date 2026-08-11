//! Parser integration tests organized by ECU type
//!
//! This module includes comprehensive tests for each ECU format parser.

// `tests/common/mod.rs` is intentionally re-included by each submodule via
// `#[path]`, so clippy sees it loaded more than once per test binary. Switching
// the submodules to `use crate::common::*` would be the structural fix.
#![allow(clippy::duplicate_mod)]
#[path = "common/mod.rs"]
mod common;

#[path = "parsers/mod.rs"]
mod parser_tests;
