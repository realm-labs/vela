use crate::option_result::{StdEnumKind, StdEnumVariant, std_enum_tag};
use crate::stored_runtime_value;
use crate::{CallFrame, HeapExecution, HeapValue, Value, VmError, VmErrorKind, VmResult};
use vela_bytecode::{Register, TryPropagateFamily};

pub(crate) enum TryPropagation {
    Continue(Value),
    Return(Value),
}

pub(crate) fn dispatch_try_propagate(
    frame: &mut CallFrame,
    heap: Option<&HeapExecution<'_>>,
    dst: Register,
    src: Register,
    expected: Option<TryPropagateFamily>,
) -> VmResult<Option<Value>> {
    match try_propagate_value(&frame.read(src)?, heap, expected)? {
        TryPropagation::Continue(value) => {
            frame.write(dst, value)?;
            Ok(None)
        }
        TryPropagation::Return(value) => Ok(Some(value)),
    }
}

pub(crate) fn try_propagate_value(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    expected: Option<TryPropagateFamily>,
) -> VmResult<TryPropagation> {
    let Value::HeapRef(reference) = value else {
        return type_error();
    };
    let Some(HeapValue::Enum {
        identity: Some(identity),
        fields,
        ..
    }) = heap.and_then(|heap| heap.heap.get(*reference))
    else {
        return type_error();
    };

    let Some((kind, variant)) = std_enum_tag(*identity) else {
        return type_error();
    };
    if let Some(expected) = expected
        && expected != try_family(kind)
    {
        return type_error();
    }

    match (kind, variant) {
        (StdEnumKind::Option, StdEnumVariant::Some) | (StdEnumKind::Result, StdEnumVariant::Ok) => {
            fields
                .get_slot(0, "0")
                .map(stored_runtime_value)
                .map(TryPropagation::Continue)
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::TypeMismatch {
                        operation: "try propagation",
                    })
                })
        }
        (StdEnumKind::Option, StdEnumVariant::None)
        | (StdEnumKind::Result, StdEnumVariant::Err) => Ok(TryPropagation::Return(*value)),
        (StdEnumKind::Option, StdEnumVariant::Ok | StdEnumVariant::Err)
        | (StdEnumKind::Result, StdEnumVariant::Some | StdEnumVariant::None) => type_error(),
    }
}

const fn try_family(kind: StdEnumKind) -> TryPropagateFamily {
    match kind {
        StdEnumKind::Option => TryPropagateFamily::Option,
        StdEnumKind::Result => TryPropagateFamily::Result,
    }
}

fn type_error<T>() -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch {
        operation: "try propagation",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heap::{HeapValue, ScriptHeap};
    use crate::option_result::{StdEnumVariant, std_enum_identity};
    use crate::script_object::ScriptFields;

    #[test]
    fn try_propagation_uses_identity_not_debug_names() {
        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Enum {
            enum_name: "NotResult".to_owned(),
            variant: "Definitely".to_owned(),
            identity: Some(std_enum_identity(StdEnumVariant::Ok)),
            fields: ScriptFields::single("NotResult::Definitely", "0", Value::I64(9)),
        });
        let execution = HeapExecution::new(&mut heap);

        match try_propagate_value(&Value::HeapRef(reference), Some(&execution), None)
            .expect("typed try propagation")
        {
            TryPropagation::Continue(value) => {
                assert_eq!(value, Value::I64(9))
            }
            TryPropagation::Return(value) => panic!("expected continue, got return {value:?}"),
        }
    }

    #[test]
    fn try_propagation_rejects_name_only_values() {
        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Ok".to_owned(),
            identity: None,
            fields: ScriptFields::single("Result::Ok", "0", Value::I64(9)),
        });
        let execution = HeapExecution::new(&mut heap);

        assert!(try_propagate_value(&Value::HeapRef(reference), Some(&execution), None).is_err());
    }
}
