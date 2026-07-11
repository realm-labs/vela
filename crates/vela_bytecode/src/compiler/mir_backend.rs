//! Physical bytecode lowering for verifier-proven MIR.
//!
//! This module is deliberately a leaf backend: it consumes only the complete
//! MIR handoff and bytecode-owned physical metadata. It must never inspect HIR,
//! analysis facts, syntax, or a live registry.

include!("mir_backend/core.rs");
include!("mir_backend/operations.rs");
include!("mir_backend/physical.rs");
include!("mir_backend/support.rs");
