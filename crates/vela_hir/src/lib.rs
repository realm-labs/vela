#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! HIR and module graph for Vela source files.

pub mod attributes;
pub mod binding;
pub mod body;
pub mod ids;
pub mod module_graph;
pub mod script_methods;
pub mod source_ingestion;
mod top_level;
pub mod type_hint;
