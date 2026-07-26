//! Fixed-size register storage for linked call frames.
//!
//! Access stays bounds-checked even though linked-program verification already
//! proves every register operand in range. That is a measured decision, not an
//! oversight.
//!
//! Verification does establish the invariant: `verify_linked_instruction`
//! matches exhaustively over `InstructionKind` and routes every register
//! operand through `verify_register_count`, frames are built with exactly the
//! `register_count` of the code object that indexes them, and this type never
//! resizes. Unchecked indexing would therefore be sound.
//!
//! It is also slower. An A/B of `get_unchecked` against the checked path on
//! identical sources (macOS/aarch64, release, interleaved runs to cancel
//! thermal drift) measured the unchecked build consistently behind:
//! `scalar_branch_loop` +24%, `recursive_countdown` +9%, `function_calls` +8%,
//! `object_field_methods` +1%. Both an `if cfg!(debug_assertions)` split and a
//! `#[cfg]`-attribute split reproduced it, so the cause is code layout in the
//! dispatch loop rather than the branch itself. The bounds check predicts
//! perfectly and costs nothing measurable; removing it perturbs the giant match
//! and loses more than it saves.
//!
//! Do not reintroduce unchecked access here without a fresh interleaved
//! measurement showing a win on these same rows.

use vela_bytecode::Register;

use crate::{Value, VmError, VmErrorKind, VmResult};

/// Fixed-size register storage owned by one call frame.
#[derive(Clone, Debug)]
pub(crate) struct RegisterFile {
    slots: Vec<Value>,
}

/// Recycles register buffers between the calls of one execution session.
///
/// Every script call used to allocate its register vector; the malloc/free
/// pair dominated call-heavy profiles. Buffers released here keep their
/// capacity, so a call/return loop reaches a steady state with no allocator
/// traffic at all.
///
/// The pool needs no size cap: every buffer it holds came from a popped frame,
/// so `pooled + live <= peak call depth` and the retained memory stays in the
/// same order as the deepest call stack the session actually reached.
///
/// Pooled buffers may contain stale values, including dangling `GcRef`s. That
/// is sound because the pool is never traced as GC roots and `acquire` clears
/// a buffer before handing it out.
#[derive(Debug, Default)]
pub(crate) struct FramePool {
    buffers: Vec<Vec<Value>>,
}

impl FramePool {
    pub(crate) fn acquire(&mut self, register_count: u16) -> Vec<Value> {
        match self.buffers.pop() {
            Some(mut buffer) => {
                buffer.clear();
                buffer.resize(usize::from(register_count), Value::Unit);
                buffer
            }
            None => vec![Value::Unit; usize::from(register_count)],
        }
    }

    pub(crate) fn release(&mut self, buffer: Vec<Value>) {
        self.buffers.push(buffer);
    }
}

impl RegisterFile {
    pub(crate) fn new(register_count: u16) -> Self {
        Self {
            slots: vec![Value::Unit; usize::from(register_count)],
        }
    }

    pub(crate) fn from_buffer(buffer: Vec<Value>) -> Self {
        Self { slots: buffer }
    }

    pub(crate) fn into_buffer(self) -> Vec<Value> {
        self.slots
    }

    /// Copies `values` into the slots starting at `offset`.
    #[inline]
    pub(crate) fn write_window(&mut self, offset: usize, values: &[Value]) -> VmResult<()> {
        let window = self
            .slots
            .get_mut(offset..offset.saturating_add(values.len()))
            .ok_or_else(|| register_window_error(offset, values.len()))?;
        window.copy_from_slice(values);
        Ok(())
    }

    /// Fills `len` slots starting at `offset` with `value`.
    #[inline]
    pub(crate) fn fill_window(&mut self, offset: usize, len: usize, value: Value) -> VmResult<()> {
        let window = self
            .slots
            .get_mut(offset..offset.saturating_add(len))
            .ok_or_else(|| register_window_error(offset, len))?;
        window.fill(value);
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn get(&self, register: Register) -> VmResult<&Value> {
        self.slots
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))
    }

    #[inline(always)]
    pub(crate) fn get_mut(&mut self, register: Register) -> VmResult<&mut Value> {
        self.slots
            .get_mut(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))
    }

    #[inline(always)]
    pub(crate) fn values(&self) -> &[Value] {
        &self.slots
    }
}

/// Reports the first out-of-range slot of a rejected window operation.
fn register_window_error(offset: usize, len: usize) -> VmError {
    let end = offset.saturating_add(len);
    VmError::new(VmErrorKind::RegisterOutOfBounds {
        register: Register(u16::try_from(end.saturating_sub(1)).unwrap_or(u16::MAX)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_round_trip_through_verified_indexes() {
        let mut registers = RegisterFile::new(3);
        *registers.get_mut(Register(1)).expect("slot 1 is in range") = Value::I64(7);
        assert_eq!(
            *registers.get(Register(1)).expect("slot 1 is in range"),
            Value::I64(7)
        );
        assert_eq!(registers.values().len(), 3);
    }

    #[test]
    fn an_out_of_range_register_is_reported_rather_than_read() {
        let registers = RegisterFile::new(1);
        let error = registers
            .get(Register(4))
            .expect_err("an out-of-range register must not be read");
        assert!(matches!(
            error.kind(),
            VmErrorKind::RegisterOutOfBounds { .. }
        ));
    }
}
