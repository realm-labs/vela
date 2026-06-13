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
    registers: Vec<FrameSlot>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FrameSlot {
    Value(Value),
    I64(i64),
    Bool(bool),
}

impl FrameSlot {
    #[inline]
    fn value(self) -> Value {
        match self {
            Self::Value(value) => value,
            Self::I64(value) => Value::i64(value),
            Self::Bool(value) => Value::Bool(value),
        }
    }

    #[inline]
    fn from_value(value: Value) -> Self {
        match value {
            Value::Scalar(vela_common::ScalarValue::I64(value)) => Self::I64(value),
            Value::Bool(value) => Self::Bool(value),
            value => Self::Value(value),
        }
    }
}

impl CallFrame {
    pub(crate) fn new(register_count: u16) -> Self {
        Self {
            registers: vec![FrameSlot::Value(Value::Null); usize::from(register_count)],
        }
    }

    pub(crate) fn read(&self, register: Register) -> VmResult<Value> {
        self.registers
            .get(usize::from(register.0))
            .copied()
            .map(FrameSlot::value)
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))
    }

    pub(crate) fn write(&mut self, register: Register, value: Value) -> VmResult<()> {
        let slot = self
            .registers
            .get_mut(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?;
        *slot = FrameSlot::from_value(value);
        Ok(())
    }

    pub(crate) fn read_i64(&self, register: Register, operation: &'static str) -> VmResult<i64> {
        match self
            .registers
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?
        {
            FrameSlot::I64(value) => Ok(*value),
            FrameSlot::Value(Value::Scalar(vela_common::ScalarValue::I64(value))) => Ok(*value),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        }
    }

    pub(crate) fn write_i64(&mut self, register: Register, value: i64) -> VmResult<()> {
        let slot = self
            .registers
            .get_mut(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?;
        *slot = FrameSlot::I64(value);
        Ok(())
    }

    pub(crate) fn read_bool(&self, register: Register, operation: &'static str) -> VmResult<bool> {
        match self
            .registers
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?
        {
            FrameSlot::Bool(value) | FrameSlot::Value(Value::Bool(value)) => Ok(*value),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        }
    }

    pub(crate) fn read_bool_lane(&self, register: Register) -> VmResult<Option<bool>> {
        match self
            .registers
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?
        {
            FrameSlot::Bool(value) => Ok(Some(*value)),
            _ => Ok(None),
        }
    }

    pub(crate) fn write_bool(&mut self, register: Register, value: bool) -> VmResult<()> {
        let slot = self
            .registers
            .get_mut(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))?;
        *slot = FrameSlot::Bool(value);
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn heap_roots(&self) -> Vec<GcRef> {
        let mut roots = Vec::new();
        self.extend_heap_roots(&mut roots);
        roots
    }

    pub(crate) fn extend_heap_roots(&self, roots: &mut Vec<GcRef>) {
        self.registers.iter().for_each(|slot| {
            if let FrameSlot::Value(value) = slot {
                value.trace_heap_refs(roots);
            }
        });
    }

    #[allow(dead_code)]
    pub(crate) fn heap_root_slots(&self) -> Vec<FrameHeapRoot> {
        let mut roots = Vec::new();
        self.registers
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                let FrameSlot::Value(value) = slot else {
                    return None;
                };
                Some((Register(u16::try_from(index).ok()?), value))
            })
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
