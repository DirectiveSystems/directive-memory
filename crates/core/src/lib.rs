//! Directive Memory core.
pub mod chunker;
pub mod db;
pub mod error;
pub mod indexer;
pub mod search;
pub mod source_type;
pub mod writeback;
pub use error::{CoreError, Result};
