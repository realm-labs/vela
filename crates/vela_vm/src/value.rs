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
    Scalar(ScalarValue),
    Range(RangeValue),
    HeapRef(GcRef),
    HostRef(HostRef),
}

impl Value {
    #[must_use]
    pub const fn i64(value: i64) -> Self {
        Self::Scalar(ScalarValue::I64(value))
    }

    #[must_use]
    pub const fn f64(value: f64) -> Self {
        Self::Scalar(ScalarValue::F64(value))
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
