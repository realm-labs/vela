//! Deterministic Rust bindings generated from compiler-owned Vela schemas.
//!
//! This crate never reads or parses Vela source. Its only semantic input is a
//! [`vela_bytecode::RustBindingSchema`] emitted by the compiler and retained by
//! the linked artifact.

mod rust;

pub use rust::{
    GeneratedRustBindings, RustBindingDiagnostic, RustBindingGenerationError, RustBindingsBuilder,
};
