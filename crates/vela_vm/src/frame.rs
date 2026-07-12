use std::sync::Arc;

use vela_bytecode::{LinkedArtifact, Register};

use crate::heap::GcRef;
use crate::{Value, VmError, VmErrorKind, VmResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameHeapRoot {
    pub register: Register,
    pub reference: GcRef,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ClosureCode;
    use vela_bytecode::{Linker, ScriptFunctionHandle, UnlinkedCodeObject, UnlinkedProgram};

    #[test]
    fn linked_frames_and_closures_clone_one_artifact_owner() {
        let mut program = UnlinkedProgram::new();
        program.insert_function(UnlinkedCodeObject::new("main", 1));
        let owner = Linker::new()
            .link_test_program(&program)
            .expect("fixture links");
        let entry = CallFrame::new_linked(1, &owner);
        assert!(Arc::ptr_eq(
            entry.linked_owner().expect("entry owner"),
            &owner
        ));

        let closure = ClosureCode::Linked {
            owner: Arc::clone(entry.linked_owner().expect("closure owner")),
            function: ScriptFunctionHandle::new(0),
        };
        let ClosureCode::Linked {
            owner: closure_owner,
            ..
        } = closure
        else {
            unreachable!();
        };
        let nested = CallFrame::new_linked(1, &closure_owner);
        assert!(Arc::ptr_eq(&owner, &closure_owner));
        assert!(Arc::ptr_eq(
            nested.linked_owner().expect("nested owner"),
            &owner
        ));

        let weak = Arc::downgrade(&owner);
        drop(entry);
        drop(nested);
        drop(closure_owner);
        drop(owner);
        assert!(weak.upgrade().is_none());
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CallFrame {
    registers: Vec<Value>,
    linked_owner: Option<Arc<LinkedArtifact>>,
}

impl CallFrame {
    pub(crate) fn new(register_count: u16) -> Self {
        Self {
            registers: vec![Value::Unit; usize::from(register_count)],
            linked_owner: None,
        }
    }

    pub(crate) fn new_linked(register_count: u16, owner: &Arc<LinkedArtifact>) -> Self {
        Self {
            registers: vec![Value::Unit; usize::from(register_count)],
            linked_owner: Some(Arc::clone(owner)),
        }
    }

    pub(crate) fn linked_owner(&self) -> Option<&Arc<LinkedArtifact>> {
        self.linked_owner.as_ref()
    }

    #[inline(always)]
    pub(crate) fn read(&self, register: Register) -> VmResult<Value> {
        self.registers
            .get(usize::from(register.0))
            .copied()
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))
    }

    #[inline(always)]
    pub(crate) fn write(&mut self, register: Register, value: Value) -> VmResult<()> {
        let slot = self
            .registers
            .get_mut(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?;
        *slot = value;
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn read_i64(&self, register: Register, operation: &'static str) -> VmResult<i64> {
        match self
            .registers
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?
        {
            Value::I64(value) => Ok(*value),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        }
    }

    #[inline(always)]
    pub(crate) fn write_i64(&mut self, register: Register, value: i64) -> VmResult<()> {
        let slot = self
            .registers
            .get_mut(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?;
        *slot = Value::I64(value);
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn read_bool(&self, register: Register, operation: &'static str) -> VmResult<bool> {
        match self
            .registers
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?
        {
            Value::Bool(value) => Ok(*value),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        }
    }

    #[inline(always)]
    pub(crate) fn read_bool_lane(&self, register: Register) -> VmResult<Option<bool>> {
        match self
            .registers
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?
        {
            Value::Bool(value) => Ok(Some(*value)),
            _ => Ok(None),
        }
    }

    #[inline(always)]
    pub(crate) fn write_bool(&mut self, register: Register, value: bool) -> VmResult<()> {
        let slot = self
            .registers
            .get_mut(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?;
        *slot = Value::Bool(value);
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn heap_roots(&self) -> Vec<GcRef> {
        let mut roots = Vec::new();
        self.extend_heap_roots(&mut roots);
        roots
    }

    pub(crate) fn extend_heap_roots(&self, roots: &mut Vec<GcRef>) {
        self.registers
            .iter()
            .for_each(|value| value.trace_heap_refs(roots));
    }

    #[allow(dead_code)]
    pub(crate) fn heap_root_slots(&self) -> Vec<FrameHeapRoot> {
        let mut roots = Vec::new();
        self.registers
            .iter()
            .enumerate()
            .filter_map(|(index, value)| Some((Register(u16::try_from(index).ok()?), value)))
            .for_each(|(register, value)| {
                let mut references = Vec::new();
                value.trace_heap_refs(&mut references);
                roots.extend(references.into_iter().map(|reference| FrameHeapRoot {
                    register,
                    reference,
                }));
            });
        roots
    }
}
