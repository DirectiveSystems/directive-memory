//! Directive Memory core.

pub mod chunker;
pub mod config;
pub mod core;
pub mod db;
pub mod error;
pub mod indexer;
pub mod search;
pub mod source_type;
pub mod stats;
pub mod writeback;

pub use self::core::Core;
pub use error::{CoreError, Result};
