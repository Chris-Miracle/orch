//! # orchestra-sync
//!
//! Hash-gated atomic writer and sync orchestration.
//!
//! Call [`sync_codebase`] to render and write all agent files for a single
//! registered codebase, or [`sync_all`] to process every registered codebase.

pub mod diff;
pub mod error;
pub mod hash_store;
pub mod pipeline;
pub mod staleness;
pub mod writer;

pub use diff::{diff_codebase, DiffCodebaseResult, FileDiff};
pub use error::SyncError;
pub use pipeline::SyncScope;
pub use staleness::StalenessSignal;
pub use writer::{sync_all, sync_codebase, SyncCodebaseResult, WriteResult};
