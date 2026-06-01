use vela_bytecode::{CodeObject, Register};

use crate::heap::GcRef;
use crate::{Value, VmError, VmErrorKind, VmResult};

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

    pub(crate) fn read(&self, register: Register) -> VmResult<&Value> {
        self.registers
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))
    }

    pub(crate) fn write(&mut self, register: Register, value: Value) -> VmResult<()> {
        let slot = self
            .registers
            .get_mut(usize::from(register.0))
            .ok_or(VmError {
                kind: VmErrorKind::RegisterOutOfBounds { register },
                source_span: None,
                call_stack: Default::default(),
            })?;
        *slot = value;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn heap_roots(&self) -> Vec<GcRef> {
        let mut roots = Vec::new();
        self.registers
            .iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        roots
    }
}

pub(crate) fn normalized_param_defaults(code: &CodeObject) -> Vec<bool> {
    let mut defaults = code.param_defaults.clone();
    defaults.resize(code.params.len(), false);
    defaults
}
