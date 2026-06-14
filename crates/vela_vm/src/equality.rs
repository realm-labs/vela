use std::cmp::Ordering;
use vela_bytecode::linked::LinkedMethodDispatchKind;
use vela_bytecode::{LinkedProgram, UnlinkedProgramCode};
use vela_def::MethodId;
use vela_reflect::registry::TypeRegistry;

use crate::heap::{GcRef, HeapValue};
use crate::linked_execution::LinkedExecutionCall;
use crate::method_runtime::CallerRoots;
use crate::numeric_ops::{
    greater_equal_numeric, greater_numeric, less_equal_numeric, less_numeric,
};
use crate::option_result::{StdEnumKind, StdEnumVariant, std_enum_tag};
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, SmallStorage, Value, Vm, VmBytecodeProfiler,
    VmError, VmErrorKind, VmInlineCaches, VmResult, store_value_in_heap_if_needed,
    stored_runtime_value,
};

const PARTIAL_EQ_METHOD: &str = "eq";
const PARTIAL_ORD_METHOD: &str = "partial_cmp";

pub(crate) fn values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    if let Some(equal) = leaf_values_equal(lhs, rhs, heap)? {
        return Ok(equal);
    }
    non_comparable("equal")
}

pub(crate) struct EqualityRuntime<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: Option<&'a dyn UnlinkedProgramCode>,
    pub(crate) linked_program: Option<&'a LinkedProgram>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) caller_roots: CallerRoots<'a>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
}

pub(crate) fn values_equal_with_traits(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    if let Some(equal) = leaf_values_equal(lhs, rhs, runtime.heap.as_deref())? {
        return Ok(equal);
    }
    call_partial_eq(lhs, rhs, runtime)?.ok_or_else(|| comparable_error("equal"))
}

pub(crate) fn values_not_equal_with_traits(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    values_equal_with_traits(lhs, rhs, runtime).map(|equal| !equal)
}

pub(crate) fn values_less_with_traits(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    values_order_with_traits(lhs, rhs, runtime, OrderingOp::Less)
}

pub(crate) fn values_less_equal_with_traits(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    values_order_with_traits(lhs, rhs, runtime, OrderingOp::LessEqual)
}

pub(crate) fn values_greater_with_traits(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    values_order_with_traits(lhs, rhs, runtime, OrderingOp::Greater)
}

pub(crate) fn values_greater_equal_with_traits(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    values_order_with_traits(lhs, rhs, runtime, OrderingOp::GreaterEqual)
}

pub(crate) fn identity_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    match (identity_key(lhs, heap)?, identity_key(rhs, heap)?) {
        (IdentityKey::Heap(lhs), IdentityKey::Heap(rhs)) => Ok(lhs == rhs),
        (IdentityKey::Host(lhs), IdentityKey::Host(rhs)) => Ok(lhs == rhs),
        (IdentityKey::Heap(_), IdentityKey::Host(_))
        | (IdentityKey::Host(_), IdentityKey::Heap(_)) => Ok(false),
    }
}

pub(crate) fn identity_not_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    identity_equal(lhs, rhs, heap).map(|equal| !equal)
}

pub(crate) fn simple_values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Option<bool>> {
    leaf_values_equal(lhs, rhs, heap)
}

fn call_partial_eq(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
) -> VmResult<Option<bool>> {
    let Some(type_name) =
        receiver_type_name(lhs, runtime.heap.as_deref(), runtime.vm.type_registry())
            .map(str::to_owned)
    else {
        return Ok(None);
    };
    let method_id = builtin_trait_method_id("PartialEq", PARTIAL_EQ_METHOD);
    let Some(result) =
        call_builtin_trait_method(lhs, rhs, runtime, &type_name, method_id, PARTIAL_EQ_METHOD)?
    else {
        return Ok(None);
    };
    let result = store_value_in_heap_if_needed(
        result,
        runtime.heap.as_deref_mut(),
        runtime.budget.as_deref_mut(),
    )?;
    match result {
        Value::Bool(value) => Ok(Some(value)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "equal",
        })),
    }
}

fn values_order_with_traits(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
    op: OrderingOp,
) -> VmResult<bool> {
    if let Ok(result) = op.numeric(lhs, rhs) {
        return Ok(result);
    }
    let Some(ordering) = call_partial_ord(lhs, rhs, runtime, op.operation())? else {
        return non_comparable(op.operation());
    };
    Ok(ordering.is_some_and(|ordering| op.matches(ordering)))
}

fn call_partial_ord(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Option<Option<Ordering>>> {
    let Some(type_name) =
        receiver_type_name(lhs, runtime.heap.as_deref(), runtime.vm.type_registry())
            .map(str::to_owned)
    else {
        return Ok(None);
    };
    let method_id = builtin_trait_method_id("PartialOrd", PARTIAL_ORD_METHOD);
    let Some(result) =
        call_builtin_trait_method(lhs, rhs, runtime, &type_name, method_id, PARTIAL_ORD_METHOD)?
    else {
        return Ok(None);
    };
    partial_cmp_result(result, runtime.heap.as_deref(), operation).map(Some)
}

fn call_builtin_trait_method(
    lhs: &Value,
    rhs: &Value,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
    type_name: &str,
    method_id: MethodId,
    method_name: &'static str,
) -> VmResult<Option<Value>> {
    if let Some(program) = runtime.program {
        let Some(function) = program.script_method_by_id(type_name, method_id) else {
            return Ok(None);
        };
        let args = SmallStorage::try_from_prefix_and_slice_map(*lhs, &[*rhs], 2, |arg| {
            Ok::<_, VmError>(*arg)
        })?;
        let protected_root_len = runtime
            .heap
            .as_deref_mut()
            .map(|heap| runtime.caller_roots.push_to_heap(heap));
        let result = runtime.vm.execute_code_object(
            function,
            runtime.program,
            args.as_slice(),
            runtime.host.as_deref_mut(),
            runtime.heap.as_deref_mut(),
            runtime.budget.as_deref_mut(),
        );
        if let (Some(heap), Some(protected_root_len)) =
            (runtime.heap.as_deref_mut(), protected_root_len)
        {
            heap.truncate_protected_roots(protected_root_len);
        }
        result.map(Some)
    } else if let Some(program) = runtime.linked_program {
        let Some(target) = linked_builtin_trait_target(program, type_name, method_name, method_id)
        else {
            return Ok(None);
        };
        let function_code = program.function(target.function).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownMethod {
                method: method_name.to_owned(),
            })
        })?;
        let args = SmallStorage::try_from_prefix_and_slice_map(*lhs, &[*rhs], 2, |arg| {
            Ok::<_, VmError>(*arg)
        })?;
        let protected_root_len = runtime
            .heap
            .as_deref_mut()
            .map(|heap| runtime.caller_roots.push_to_heap(heap));
        let result = runtime.vm.execute_linked_call(
            LinkedExecutionCall {
                code: function_code,
                program,
                captures: &[],
                args: args.as_slice(),
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: runtime.inline_caches,
                bytecode_profiler: runtime.bytecode_profiler,
            },
            runtime.host.as_deref_mut(),
            runtime.heap.as_deref_mut(),
            runtime.budget.as_deref_mut(),
        );
        if let (Some(heap), Some(protected_root_len)) =
            (runtime.heap.as_deref_mut(), protected_root_len)
        {
            heap.truncate_protected_roots(protected_root_len);
        }
        result.map(Some)
    } else {
        Ok(None)
    }
}

#[derive(Clone, Copy)]
struct LinkedBuiltinTraitTarget {
    function: vela_bytecode::ScriptFunctionHandle,
}

fn linked_builtin_trait_target(
    program: &LinkedProgram,
    type_name: &str,
    method_name: &str,
    method_id: MethodId,
) -> Option<LinkedBuiltinTraitTarget> {
    let dispatch = program.script_method_dispatch(type_name, method_name)?;
    let dispatch = program.method_dispatch(dispatch)?;
    match &dispatch.kind {
        LinkedMethodDispatchKind::Script {
            method_id: actual,
            function,
        } if *actual == method_id => Some(LinkedBuiltinTraitTarget {
            function: *function,
        }),
        _ => None,
    }
}

fn partial_cmp_result(
    result: Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Option<Ordering>> {
    let Value::HeapRef(reference) = result else {
        return non_comparable(operation);
    };
    let Some(HeapValue::Enum {
        identity: Some(identity),
        fields,
        ..
    }) = heap.and_then(|heap| heap.heap.get(reference))
    else {
        return non_comparable(operation);
    };
    match std_enum_tag(*identity) {
        Some((StdEnumKind::Option, StdEnumVariant::None)) => Ok(None),
        Some((StdEnumKind::Option, StdEnumVariant::Some)) => {
            let payload = fields
                .get_slot(0, "0")
                .map(stored_runtime_value)
                .ok_or_else(|| comparable_error(operation))?;
            partial_cmp_payload_ordering(payload, operation).map(Some)
        }
        _ => non_comparable(operation),
    }
}

fn partial_cmp_payload_ordering(value: Value, operation: &'static str) -> VmResult<Ordering> {
    let Value::I64(value) = value else {
        return non_comparable(operation);
    };
    Ok(value.cmp(&0))
}

#[derive(Clone, Copy)]
enum OrderingOp {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl OrderingOp {
    fn operation(self) -> &'static str {
        match self {
            Self::Less => "less",
            Self::LessEqual => "less_equal",
            Self::Greater => "greater",
            Self::GreaterEqual => "greater_equal",
        }
    }

    fn numeric(self, lhs: &Value, rhs: &Value) -> VmResult<bool> {
        match self {
            Self::Less => less_numeric(lhs, rhs),
            Self::LessEqual => less_equal_numeric(lhs, rhs),
            Self::Greater => greater_numeric(lhs, rhs),
            Self::GreaterEqual => greater_equal_numeric(lhs, rhs),
        }
    }

    fn matches(self, ordering: Ordering) -> bool {
        match self {
            Self::Less => ordering == Ordering::Less,
            Self::LessEqual => matches!(ordering, Ordering::Less | Ordering::Equal),
            Self::Greater => ordering == Ordering::Greater,
            Self::GreaterEqual => matches!(ordering, Ordering::Greater | Ordering::Equal),
        }
    }
}

fn receiver_type_name<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
    registry: Option<&'a TypeRegistry>,
) -> Option<&'a str> {
    match receiver {
        Value::HostRef(reference) => registry
            .and_then(|registry| registry.type_of_host(*reference))
            .map(|desc| desc.key.name.as_str()),
        Value::HeapRef(reference) => match heap?.heap.get(*reference)? {
            HeapValue::Record { type_name, .. } => Some(type_name.as_str()),
            HeapValue::Enum { enum_name, .. } => Some(enum_name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn builtin_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    MethodId::new(u128::from(vela_common::stable_id(
        "trait_method",
        trait_name,
        method_name,
    )))
}

fn leaf_values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Option<bool>> {
    if let Some(equal) = immediate_leaf_values_equal(lhs, rhs) {
        return Ok(Some(equal));
    }

    match (heap_leaf(lhs, heap)?, heap_leaf(rhs, heap)?) {
        (Some(HeapLeaf::String(lhs)), Some(HeapLeaf::String(rhs))) => Ok(Some(lhs == rhs)),
        (Some(HeapLeaf::Bytes(lhs)), Some(HeapLeaf::Bytes(rhs))) => Ok(Some(lhs == rhs)),
        (Some(_), Some(_)) => Ok(Some(false)),
        (Some(_), None) | (None, Some(_)) if is_immediate_comparable_leaf(lhs, rhs) => {
            Ok(Some(false))
        }
        (Some(_), None) | (None, Some(_)) => Ok(None),
        (None, None) => Ok(None),
    }
}

fn immediate_leaf_values_equal(lhs: &Value, rhs: &Value) -> Option<bool> {
    match (lhs, rhs) {
        (Value::Missing, _) | (_, Value::Missing) => None,
        (Value::Null, Value::Null) => Some(true),
        (Value::Bool(lhs), Value::Bool(rhs)) => Some(lhs == rhs),
        (Value::Char(lhs), Value::Char(rhs)) => Some(lhs == rhs),
        (Value::Range(lhs), Value::Range(rhs)) => Some(lhs == rhs),
        (lhs, rhs) if lhs.is_scalar() && rhs.is_scalar() => {
            Some(lhs.as_scalar() == rhs.as_scalar())
        }
        (lhs, rhs)
            if is_immediate_comparable_leaf(lhs, rhs)
                && (is_immediate_leaf(lhs) || is_immediate_leaf(rhs)) =>
        {
            Some(false)
        }
        _ => None,
    }
}

fn is_immediate_comparable_leaf(lhs: &Value, rhs: &Value) -> bool {
    is_immediate_leaf(lhs) || is_immediate_leaf(rhs)
}

fn is_immediate_leaf(value: &Value) -> bool {
    matches!(
        value,
        Value::Null
            | Value::Bool(_)
            | Value::Char(_)
            | Value::I8(_)
            | Value::I16(_)
            | Value::I32(_)
            | Value::I64(_)
            | Value::U8(_)
            | Value::U16(_)
            | Value::U32(_)
            | Value::U64(_)
            | Value::F32(_)
            | Value::F64(_)
            | Value::Range(_)
    )
}

fn heap_leaf<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> VmResult<Option<HeapLeaf<'a>>> {
    let Value::HeapRef(reference) = value else {
        return Ok(None);
    };
    let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return non_comparable("equal");
    };
    match heap_value {
        HeapValue::String(value) => Ok(Some(HeapLeaf::String(value))),
        HeapValue::Bytes(value) => Ok(Some(HeapLeaf::Bytes(value))),
        HeapValue::PathProxy(_) => non_comparable("equal"),
        HeapValue::Array(_)
        | HeapValue::Map(_)
        | HeapValue::Set(_)
        | HeapValue::Record { .. }
        | HeapValue::Enum { .. }
        | HeapValue::Closure(_)
        | HeapValue::Iterator(_) => Ok(None),
    }
}

fn identity_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<IdentityKey> {
    match value {
        Value::HeapRef(reference) => heap_identity_key(*reference, heap),
        Value::HostRef(reference) => Ok(IdentityKey::Host(*reference)),
        Value::Missing => non_comparable("identity equal"),
        Value::Null
        | Value::Bool(_)
        | Value::Char(_)
        | Value::I8(_)
        | Value::I16(_)
        | Value::I32(_)
        | Value::I64(_)
        | Value::U8(_)
        | Value::U16(_)
        | Value::U32(_)
        | Value::U64(_)
        | Value::F32(_)
        | Value::F64(_)
        | Value::Range(_) => non_comparable("identity equal"),
    }
}

fn heap_identity_key(reference: GcRef, heap: Option<&HeapExecution<'_>>) -> VmResult<IdentityKey> {
    let Some(heap_value) = heap.and_then(|heap| heap.heap.get(reference)) else {
        return non_comparable("identity equal");
    };
    match heap_value {
        HeapValue::Array(_)
        | HeapValue::Map(_)
        | HeapValue::Set(_)
        | HeapValue::Record { .. }
        | HeapValue::Enum { .. }
        | HeapValue::Closure(_)
        | HeapValue::Iterator(_) => Ok(IdentityKey::Heap(reference)),
        HeapValue::String(_) | HeapValue::Bytes(_) | HeapValue::PathProxy(_) => {
            non_comparable("identity equal")
        }
    }
}

fn non_comparable<T>(operation: &'static str) -> VmResult<T> {
    Err(comparable_error(operation))
}

fn comparable_error(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}

enum HeapLeaf<'a> {
    String(&'a str),
    Bytes(&'a [u8]),
}

enum IdentityKey {
    Heap(GcRef),
    Host(vela_host::path::HostRef),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use vela_common::{HostObjectId, HostTypeId, ShapeId};
    use vela_def::TypeId;
    use vela_host::path::HostRef;
    use vela_host::proxy::PathProxy;
    use vela_host::target::HostTargetPlan;

    use crate::heap::{RecordIdentity, ScriptHeap};
    use crate::ranges::RangeValue;
    use crate::script_object::ScriptFields;

    use super::*;

    #[test]
    fn semantic_equality_is_tag_exact_for_leaf_values() {
        assert_eq!(equal(Value::Null, Value::Null), Ok(true));
        assert_eq!(equal(Value::Bool(true), Value::Bool(false)), Ok(false));
        assert_eq!(equal(Value::Char('v'), Value::Char('v')), Ok(true));
        assert_eq!(equal(Value::I64(1), Value::I64(1)), Ok(true));
        assert_eq!(equal(Value::I64(1), Value::U64(1)), Ok(false));
        assert_eq!(equal(Value::F64(f64::NAN), Value::F64(f64::NAN)), Ok(false));
        assert_eq!(equal(Value::F64(-0.0), Value::F64(0.0)), Ok(true));
        assert_eq!(
            equal(
                Value::Range(RangeValue::new(0, 10, false)),
                Value::Range(RangeValue::new(0, 10, false))
            ),
            Ok(true)
        );
    }

    #[test]
    fn semantic_equality_compares_string_and_bytes_payloads() {
        let mut heap = ScriptHeap::new();
        let left = Value::HeapRef(heap.allocate(HeapValue::String("gold".to_owned())));
        let right = Value::HeapRef(heap.allocate(HeapValue::String("gold".to_owned())));
        let bytes = Value::HeapRef(heap.allocate(HeapValue::Bytes(vec![1, 2, 3])));
        let same_bytes = Value::HeapRef(heap.allocate(HeapValue::Bytes(vec![1, 2, 3])));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(values_equal(&left, &right, Some(&heap)), Ok(true));
        assert_eq!(values_equal(&bytes, &same_bytes, Some(&heap)), Ok(true));
        assert_eq!(values_equal(&left, &bytes, Some(&heap)), Ok(false));
        assert_eq!(values_equal(&left, &Value::I64(1), Some(&heap)), Ok(false));
    }

    #[test]
    fn semantic_equality_rejects_objects_without_partial_eq() {
        let mut heap = ScriptHeap::new();
        let array = Value::HeapRef(heap.allocate(HeapValue::Array(Vec::new())));
        let record = Value::HeapRef(heap.allocate(record("Reward")));
        let heap = HeapExecution::new(&mut heap);

        assert_type_mismatch(values_equal(&array, &array, Some(&heap)), "equal");
        assert_type_mismatch(values_equal(&record, &record, Some(&heap)), "equal");
    }

    #[test]
    fn semantic_equality_rejects_missing_and_path_proxy() {
        assert_type_mismatch(
            values_equal(&Value::Missing, &Value::Missing, None),
            "equal",
        );

        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let plan = HostTargetPlan::new(host_ref.type_id);
        let mut heap = ScriptHeap::new();
        let proxy =
            Value::HeapRef(heap.allocate(HeapValue::PathProxy(PathProxy::new(host_ref, plan))));
        let heap = HeapExecution::new(&mut heap);

        assert_type_mismatch(values_equal(&proxy, &proxy, Some(&heap)), "equal");
    }

    #[test]
    fn identity_equality_accepts_only_identity_values() {
        let mut heap = ScriptHeap::new();
        let first = Value::HeapRef(heap.allocate(record("Reward")));
        let second = Value::HeapRef(heap.allocate(record("Reward")));
        let string = Value::HeapRef(heap.allocate(HeapValue::String("Reward".to_owned())));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(identity_equal(&first, &first, Some(&heap)), Ok(true));
        assert_eq!(identity_equal(&first, &second, Some(&heap)), Ok(false));
        assert_type_mismatch(
            identity_equal(&string, &string, Some(&heap)),
            "identity equal",
        );
        assert_type_mismatch(
            identity_equal(&Value::I64(1), &Value::I64(1), Some(&heap)),
            "identity equal",
        );
    }

    #[test]
    fn identity_equality_compares_host_refs_without_host_reads() {
        let first = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let same = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let stale = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 2);

        assert_eq!(
            identity_equal(&Value::HostRef(first), &Value::HostRef(same), None),
            Ok(true)
        );
        assert_eq!(
            identity_equal(&Value::HostRef(first), &Value::HostRef(stale), None),
            Ok(false)
        );
    }

    fn equal(lhs: Value, rhs: Value) -> VmResult<bool> {
        values_equal(&lhs, &rhs, None)
    }

    fn assert_type_mismatch(result: VmResult<bool>, operation: &'static str) {
        let error = result.expect_err("operation should reject non-comparable value");
        assert_eq!(error.kind(), VmErrorKind::TypeMismatch { operation });
    }

    fn record(type_name: &str) -> HeapValue {
        HeapValue::Record {
            type_name: type_name.to_owned(),
            identity: Some(RecordIdentity::new(TypeId::new(1), ShapeId::new(1))),
            fields: ScriptFields::from(BTreeMap::from([("id".to_owned(), Value::I64(1))])),
        }
    }
}
