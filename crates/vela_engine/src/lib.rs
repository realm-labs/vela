//! Stable embedding API for installing schemas and native functions into Vela.

#![allow(clippy::result_large_err)]

pub mod args;
pub mod builder;
pub mod clock;
mod compiler_options;
pub mod context;
pub mod context_schema;
pub mod engine;
pub mod error;
pub mod host_type;
mod metadata;
pub mod method;
pub mod native;
pub mod permission;
pub mod prelude;
pub mod random;
pub mod reload;
pub mod runtime;
pub mod schema;
pub mod source;
pub mod standard;
pub mod typed;
mod validation;

#[cfg(feature = "serde")]
pub use vela_vm::serde;

#[cfg(test)]
mod tests;
