use std::sync::Arc;

use vela_bytecode::{ScriptFunctionHandle, UnlinkedCodeObject};
use vela_host::path::HostRef;

use crate::heap::GcRef;
use crate::ranges::RangeValue;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Missing,
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Range(RangeValue),
    HeapRef(GcRef),
    HostRef(HostRef),
}

impl Value {
    pub fn trace_heap_refs(&self, refs: &mut Vec<GcRef>) {
        if let Self::HeapRef(reference) = self {
            refs.push(*reference);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClosureValue {
    pub(crate) code: ClosureCode,
    pub(crate) captures: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClosureCode {
    Unlinked(Arc<UnlinkedCodeObject>),
    Linked(ScriptFunctionHandle),
}
