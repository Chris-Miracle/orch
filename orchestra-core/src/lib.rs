//! Orchestra core library — domain types, registry persistence, errors.
//!
//! Public API surface for Phase 01:
//! - [`types`] — newtypes and domain structs
//! - [`error`] — [`RegistryError`]
//! - [`registry`] — load / save / init

pub mod error;
pub mod registry;
pub mod types;

pub use error::RegistryError;
pub use types::{
    AgentConfig, Codebase, CodebaseName, Project, ProjectName, ProjectType, Registry, Task,
    TaskId, TaskStatus,
};
