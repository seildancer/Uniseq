//! Core backend domain types for Uniseq.
//!
//! The crate intentionally keeps markdown files as the only durable source of
//! truth: pages are durable file-backed objects, while blocks and references are
//! disposable parsed projections.

pub mod core;

pub use core::*;
