//! # orchestra-renderer
//!
//! Tera-based template engine that renders per-agent instruction files from
//! Orchestra registry data.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use orchestra_renderer::{Renderer, AgentKind};
//! use orchestra_core::types::Codebase;
//!
//! fn render_all(codebase: &Codebase) {
//!     if let Ok(renderer) = Renderer::new() {
//!         for agent in AgentKind::all() {
//!             if let Ok(outputs) = renderer.render(codebase, *agent) {
//!                 for (path, content) in outputs {
//!                     println!("{}: {} bytes", path.display(), content.len());
//!                 }
//!             }
//!         }
//!     }
//! }
//! ```

pub mod context;
pub mod engine;
pub mod error;

pub use context::TemplateContext;
pub use engine::{AgentKind, Renderer, TemplateEngine};
pub use error::RenderError;
