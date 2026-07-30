//! Scripting — Rust game modules first; visual scripting later.
//!
//! Prefer native Rust plugins over embedding a second language until needed.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod module;
pub mod registry;
pub mod visual_graph;

pub use module::{ScriptContext, ScriptModule};
pub use registry::ScriptRegistry;
pub use visual_graph::{
    VisualExecStep, VisualGraph, VisualLink, VisualNode, VisualNodeKind,
};
