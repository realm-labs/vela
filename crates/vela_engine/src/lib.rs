#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Stable embedding API for installing schemas and native functions into Vela.

#![allow(clippy::result_large_err)]

extern crate self as vela_engine;

pub mod args;
pub mod binding;
pub mod builder;
pub mod clock;
mod compiler_options;
mod compiler_registry;
pub mod context;
pub mod context_schema;
pub mod engine;
pub mod error;
pub mod host_lease;
pub mod host_type;
pub mod interop;
pub mod io;
mod metadata;
pub mod method;
pub mod native;
pub mod permission;
pub mod prelude;
pub mod random;
pub mod reload;
pub mod runtime;
pub mod schema;
pub mod service;
pub mod source;
pub mod standard;
pub mod type_binding;
pub mod type_registration;
pub mod typed;
mod validation;

#[cfg(feature = "serde")]
pub use vela_vm::serde;

#[cfg(test)]
mod tests;
