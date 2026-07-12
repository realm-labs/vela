#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Analysis-only facts for diagnostics, completion, and stdlib metadata.

pub mod callable;
pub mod completion;
pub mod contracts;
pub mod executable;
pub mod fact_scope;
pub mod facts;
pub mod hints;
pub mod hover;
pub mod literals;
pub mod logical_records;
pub mod registry;
pub mod semantic_facts;
pub mod stdlib;
pub mod type_fact;
pub mod validation;
