use crate::option_result::{StdEnumKind, StdEnumVariant, std_enum_tag};
use crate::stored_runtime_value;
use crate::{CallFrame, HeapExecution, HeapValue, Value, VmError, VmErrorKind, VmResult};
use vela_bytecode::Register;

pub(crate) enum TryPropagation {
    Continue(Value),
    Return(Value),
}

pub(crate) fn dispatch_try_propagate(
    frame: &mut CallFrame,
    heap: Option<&HeapExecution<'_>>,
    dst: Register,
    src: Register,
) -> VmResult<Option<Value>> {
    match try_propagate_value(frame.read(src)?, heap)? {
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

    match std_enum_tag(*identity) {
        Some((StdEnumKind::Option, StdEnumVariant::Some))
        | Some((StdEnumKind::Result, StdEnumVariant::Ok)) => fields
            .get_slot(0, "0")
            .map(stored_runtime_value)
            .map(TryPropagation::Continue)
            .ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "try propagation",
                })
            }),
        Some((StdEnumKind::Option, StdEnumVariant::None))
        | Some((StdEnumKind::Result, StdEnumVariant::Err)) => Ok(TryPropagation::Return(*value)),
        None => type_error(),
        Some((StdEnumKind::Option, StdEnumVariant::Ok | StdEnumVariant::Err))
        | Some((StdEnumKind::Result, StdEnumVariant::Some | StdEnumVariant::None)) => type_error(),
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
            fields: ScriptFields::single(
                "NotResult::Definitely",
                "0",
                Value::Scalar(vela_common::ScalarValue::I64(9)),
            ),
        });
        let execution = HeapExecution::new(&mut heap);

        match try_propagate_value(&Value::HeapRef(reference), Some(&execution))
            .expect("typed try propagation")
        {
            TryPropagation::Continue(value) => {
                assert_eq!(value, Value::Scalar(vela_common::ScalarValue::I64(9)))
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
            fields: ScriptFields::single(
                "Result::Ok",
                "0",
                Value::Scalar(vela_common::ScalarValue::I64(9)),
            ),
        });
        let execution = HeapExecution::new(&mut heap);

        assert!(try_propagate_value(&Value::HeapRef(reference), Some(&execution)).is_err());
    }
}
