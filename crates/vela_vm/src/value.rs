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
    Char(char),
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

macro_rules! impl_scalar_value_helpers {
    (
        $(
            $value_variant:ident($scalar_variant:ident)
        ),* $(,)?
    ) => {
        #[must_use]
        pub const fn from_scalar(value: ScalarValue) -> Self {
            match value {
                $(
                    ScalarValue::$scalar_variant(value) => Self::$value_variant(value),
                )*
            }
        }

        #[must_use]
        pub const fn as_scalar(self) -> Option<ScalarValue> {
            match self {
                $(
                    Self::$value_variant(value) => Some(ScalarValue::$scalar_variant(value)),
                )*
                _ => None,
            }
        }

        #[must_use]
        pub const fn is_scalar(self) -> bool {
            matches!(
                self,
                $(
                    Self::$value_variant(_)
                )|*
            )
        }
    };
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

    impl_scalar_value_helpers!(
        I8(I8),
        I16(I16),
        I32(I32),
        I64(I64),
        U8(U8),
        U16(U16),
        U32(U32),
        U64(U64),
        F32(F32),
        F64(F64),
    );

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
