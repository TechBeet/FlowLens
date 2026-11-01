//! HTTP request replay, session comparison, and fuzzing for FlowLens.
//!
//! This crate provides tools for:
//! - Replaying captured HTTP requests
//! - Comparing two sessions to find differences
//! - Generating fuzzing test cases

pub mod client;
pub mod diff;
pub mod errors;
pub mod fuzzer;

pub use client::ReplayClient;
pub use diff::{diff_sessions, DiffResult, RequestDiff, DiffKind};
pub use errors::ReplayError;
pub use fuzzer::{fuzz_param, FuzzCase, FuzzParam, FuzzResult};
