//! Core module tests for non-parser functionality
//!
//! Tests for field normalization, expressions, units, state management,
//! and computed channels.

// `tests/common/mod.rs` is intentionally re-included by each submodule via
// `#[path]`, so clippy sees it loaded more than once per test binary. Switching
// the submodules to `use crate::common::*` would be the structural fix.
#![allow(clippy::duplicate_mod)]
#[path = "common/mod.rs"]
mod common;

#[path = "core/mod.rs"]
mod core_tests;
