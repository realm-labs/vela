use std::sync::Arc;

use vela_bytecode::ScriptFunctionHandle;
use vela_common::ScalarValue;
use vela_host::path::HostSlotRef;

use crate::heap::GcRef;
use crate::small_storage::SmallStorage;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Missing,
    Unit,
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
    HeapRef(GcRef),
    HostRef(HostSlotRef),
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
    pub(crate) owner: Arc<vela_bytecode::LinkedArtifact>,
    pub(crate) function: ScriptFunctionHandle,
    pub(crate) captures: SmallStorage<Value>,
}

#[cfg(test)]
mod layout_tests {
    /// The register slot is copied on every read, argument pass, array element
    /// access, and frame operation, so its size is a whole-interpreter memory
    /// traffic multiplier. Sixteen bytes matches Lua 5.4 and Luau; the move
    /// from 24 came from taking `Range` out of the inline variants, so any
    /// new variant payload must stay at or below eight bytes.
    #[test]
    fn value_slot_stays_at_sixteen_bytes() {
        assert_eq!(
            std::mem::size_of::<super::Value>(),
            16,
            "Value grew; a register slot must stay two words"
        );
        assert_eq!(std::mem::size_of::<Option<super::Value>>(), 16);
    }
}
