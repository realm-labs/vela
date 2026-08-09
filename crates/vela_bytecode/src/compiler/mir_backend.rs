//! Physical bytecode lowering for verifier-proven MIR.
//!
//! This module is deliberately a leaf backend: it consumes only the complete
//! MIR handoff and bytecode-owned physical metadata. It must never inspect HIR,
//! analysis facts, syntax, or a live registry.

mod core;
mod selection;

pub(crate) use core::MirBackendError;

pub(crate) fn compile(
    handoff: vela_mir::MirBackendHandoff<'_>,
) -> Result<crate::UnlinkedCodeObject, MirBackendError> {
    let plan = selection::select(handoff).map_err(MirBackendError::InvalidSelection)?;
    selection::verify(handoff, &plan).map_err(MirBackendError::InvalidSelection)?;
    core::compile(handoff)
}
