use vela_bytecode::Register;

use crate::heap::GcRef;
use crate::{Value, VmError, VmErrorKind, VmResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameHeapRoot {
    pub register: Register,
    pub reference: GcRef,
}

#[derive(Clone, Debug)]
pub(crate) struct CallFrame {
    registers: Vec<Value>,
}

impl CallFrame {
    pub(crate) fn new(register_count: u16) -> Self {
        Self {
            registers: vec![Value::Null; usize::from(register_count)],
        }
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
