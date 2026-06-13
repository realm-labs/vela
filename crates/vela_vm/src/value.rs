use std::sync::Arc;

use vela_bytecode::{ScriptFunctionHandle, UnlinkedCodeObject};
use vela_common::ScalarValue;
use vela_host::path::HostRef;

use crate::heap::GcRef;
use crate::ranges::RangeValue;
use crate::small_storage::SmallStorage;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Missing,
    Null,
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Range(RangeValue),
    HeapRef(GcRef),
    HostRef(HostRef),
}

impl Value {
    #[must_use]
    pub const fn i64(value: i64) -> Self {
        Self::I64(value)
    }

    #[must_use]
    pub const fn f64(value: f64) -> Self {
        Self::F64(value)
    }

    #[must_use]
    pub const fn from_scalar(value: ScalarValue) -> Self {
        match value {
            ScalarValue::I8(value) => Self::I8(value),
            ScalarValue::I16(value) => Self::I16(value),
            ScalarValue::I32(value) => Self::I32(value),
            ScalarValue::I64(value) => Self::I64(value),
            ScalarValue::U8(value) => Self::U8(value),
            ScalarValue::U16(value) => Self::U16(value),
            ScalarValue::U32(value) => Self::U32(value),
            ScalarValue::U64(value) => Self::U64(value),
            ScalarValue::F32(value) => Self::F32(value),
            ScalarValue::F64(value) => Self::F64(value),
        }
    }

    #[must_use]
    pub const fn as_scalar(self) -> Option<ScalarValue> {
        match self {
            Self::I8(value) => Some(ScalarValue::I8(value)),
            Self::I16(value) => Some(ScalarValue::I16(value)),
            Self::I32(value) => Some(ScalarValue::I32(value)),
            Self::I64(value) => Some(ScalarValue::I64(value)),
            Self::U8(value) => Some(ScalarValue::U8(value)),
            Self::U16(value) => Some(ScalarValue::U16(value)),
            Self::U32(value) => Some(ScalarValue::U32(value)),
            Self::U64(value) => Some(ScalarValue::U64(value)),
            Self::F32(value) => Some(ScalarValue::F32(value)),
            Self::F64(value) => Some(ScalarValue::F64(value)),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_scalar(self) -> bool {
        self.as_scalar().is_some()
    }

    pub fn trace_heap_refs(&self, refs: &mut Vec<GcRef>) {
        if let Self::HeapRef(reference) = self {
            refs.push(*reference);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClosureValue {
    pub(crate) code: ClosureCode,
    pub(crate) captures: SmallStorage<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClosureCode {
    Unlinked(Arc<UnlinkedCodeObject>),
    Linked(ScriptFunctionHandle),
}
