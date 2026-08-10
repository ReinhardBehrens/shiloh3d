//! Scripting — Rust game modules, Rhai, and visual graphs.
//!
//! Prefer native Rust plugins for performance-critical logic; use Rhai / visual
//! graphs for gameplay authoring (Phase 5).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod module;
pub mod registry;
pub mod rhai_host;
pub mod script_component;
pub mod visual_graph;

pub use module::{ScriptContext, ScriptModule};
pub use registry::ScriptRegistry;
pub use rhai_host::{RhaiHost, RhaiHostError, ScriptCommand};
pub use script_component::{ScriptComponent, ScriptKind};
pub use visual_graph::{
    actions_from_steps, VisualAction, VisualExecStep, VisualGraph, VisualLink, VisualNode,
    VisualNodeKind,
};
