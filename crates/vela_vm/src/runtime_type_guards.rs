use vela_bytecode::{
    GuardKind, LinkedCodeObject, LinkedProgram, Register, StandardTypeGuard, TypeGuard,
    TypeGuardPlan, TypeGuardPlanId, UnlinkedTypeGuard, UnlinkedTypeGuardPlan,
};
use vela_common::PrimitiveTag;

use crate::budget::ExecutionBudget;
use crate::container_contracts::{
    ContainerContractStamp, ContainerSummaryProof, ContainerTypeSummary, ShallowTypeKey,
};
use crate::heap::HeapValue;
use crate::iteration::IteratorItemGuard;
use crate::method_runtime::MethodRuntime;
use crate::option_result::{StdEnumKind, StdEnumVariant, std_enum_tag};
use crate::stored_runtime_value;
use crate::{CallFrame, HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) struct GuardExecutionContext<'a, 'heap> {
    heap: Option<&'a mut HeapExecution<'heap>>,
    budget: Option<&'a mut ExecutionBudget>,
}

impl<'a, 'heap> GuardExecutionContext<'a, 'heap> {
    pub(crate) fn new(
        heap: Option<&'a mut HeapExecution<'heap>>,
        budget: Option<&'a mut ExecutionBudget>,
    ) -> Self {
        Self { heap, budget }
    }

    fn heap(&self) -> Option<&HeapExecution<'heap>> {
        self.heap.as_deref()
    }

    fn heap_mut(&mut self) -> Option<&mut HeapExecution<'heap>> {
        self.heap.as_deref_mut()
    }

    fn charge_scan_item(&mut self) -> VmResult<()> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.charge_instructions(1)?;
        }
        Ok(())
    }
}

pub(crate) fn execute_unlinked_guard(
    value: &Value,
    guard: &UnlinkedTypeGuard,
    context: &mut GuardExecutionContext<'_, '_>,
) -> VmResult<()> {
    // The interpreter is the generic fallback path for specialization misses.
    if guard.context.kind == GuardKind::Specialization {
        return Ok(());
    }

    let heap = context.heap();
    match guard.plan {
        UnlinkedTypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, expected, heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Standard(expected) => {
            execute_standard_guard(value, expected, heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Array { ref element } => execute_array_guard(
            value,
            element.as_deref(),
            context,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Map {
            ref key,
            value: ref value_plan,
        } => execute_map_guard(
            value,
            key.as_deref(),
            value_plan.as_deref(),
            context,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Set { ref element } => execute_set_guard(
            value,
            element.as_deref(),
            context,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Iterator { ref item } => execute_iterator_guard(
            value,
            item.as_deref()
                .cloned()
                .map(|plan| IteratorItemGuard::unlinked(plan, guard.context.debug_name.clone())),
            context,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Option { ref some } => {
            execute_option_guard(value, some.as_deref(), context, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Result { ref ok, ref err } => execute_result_guard(
            value,
            ok.as_deref(),
            err.as_deref(),
            context,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Type(ref expected) => {
            execute_unlinked_type_guard(value, expected, heap, &guard.context.debug_name)
        }
        UnlinkedTypeGuardPlan::Variant {
            ref enum_name,
            ref variant,
        } => execute_unlinked_variant_guard(
            value,
            enum_name,
            variant,
            heap,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::Shape {
            ref type_name,
            shape_id,
        } => execute_unlinked_shape_guard(
            value,
            type_name,
            shape_id,
            heap,
            &guard.context.debug_name,
        ),
        UnlinkedTypeGuardPlan::HostType(_) => Ok(()),
    }
}

pub(crate) fn execute_linked_guard(
    value: &Value,
    guard: &TypeGuard,
    program: &LinkedProgram,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    // The interpreter is the generic fallback path for specialization misses.
    if guard.context.kind == GuardKind::Specialization {
        return Ok(());
    }

    let heap = context.heap();
    match guard.plan {
        TypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, expected, heap, debug_name)
        }
        TypeGuardPlan::Standard(expected) => {
            execute_standard_guard(value, expected, heap, debug_name)
        }
        TypeGuardPlan::Array { ref element } => {
            execute_linked_array_guard(value, element.as_deref(), program, context, debug_name)
        }
        TypeGuardPlan::Map {
            ref key,
            value: ref value_plan,
        } => execute_linked_map_guard(
            value,
            key.as_deref(),
            value_plan.as_deref(),
            program,
            context,
            debug_name,
        ),
        TypeGuardPlan::Set { ref element } => {
            execute_linked_set_guard(value, element.as_deref(), program, context, debug_name)
        }
        TypeGuardPlan::Iterator { ref item } => execute_iterator_guard(
            value,
            item.as_deref()
                .cloned()
                .map(|plan| IteratorItemGuard::linked(plan, debug_name.to_owned())),
            context,
            debug_name,
        ),
        TypeGuardPlan::Option { ref some } => {
            execute_linked_option_guard(value, some.as_deref(), program, context, debug_name)
        }
        TypeGuardPlan::Result { ref ok, ref err } => execute_linked_result_guard(
            value,
            ok.as_deref(),
            err.as_deref(),
            program,
            context,
            debug_name,
        ),
        TypeGuardPlan::Type(expected) => {
            let expected = program.ty(expected).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "type_guard",
                })
            })?;
            execute_type_id_guard(
                value,
                expected.id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::Variant(expected) => {
            let expected = program.variant(expected).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "variant_guard",
                })
            })?;
            execute_variant_id_guard(
                value,
                expected.id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::Shape { ty, shape_id } => {
            let expected = program.ty(ty).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "shape_guard",
                })
            })?;
            execute_shape_id_guard(
                value,
                expected.id,
                shape_id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::HostType(_) => Ok(()),
    }
}

pub(crate) fn execute_linked_param_guards(
    code: &LinkedCodeObject,
    program: &LinkedProgram,
    frame: &CallFrame,
    context: &mut GuardExecutionContext<'_, '_>,
) -> VmResult<()> {
    let param_offset = usize::from(code.capture_count);
    for param_guard in &code.param_guards {
        let register = Register(
            code.capture_count
                .checked_add(param_guard.parameter)
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::RegisterOutOfBounds {
                        register: Register(u16::MAX),
                    })
                })?,
        );
        let value = frame.read(register)?;
        if matches!(value, Value::Missing) {
            continue;
        }
        let guard = code.type_guard(param_guard.guard).ok_or_else(|| {
            VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "param_guard",
            })
        })?;
        execute_linked_guard(
            &value,
            guard,
            program,
            context,
            program.debug_name(guard.context.debug_name),
        )?;
        debug_assert!(usize::from(param_guard.parameter) < code.params.len());
        debug_assert!(usize::from(register.0) >= param_offset);
    }
    Ok(())
}

pub(crate) fn execute_linked_register_guard(
    code: &LinkedCodeObject,
    program: &LinkedProgram,
    frame: &CallFrame,
    register: Register,
    guard_id: TypeGuardPlanId,
    context: &mut GuardExecutionContext<'_, '_>,
) -> VmResult<()> {
    let value = frame.read(register)?;
    let guard = code.type_guard(guard_id).ok_or_else(|| {
        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "GuardType",
        })
    })?;
    execute_linked_guard(
        &value,
        guard,
        program,
        context,
        program.debug_name(guard.context.debug_name),
    )
}

pub(crate) fn execute_linked_return_guard(
    code: &LinkedCodeObject,
    program: &LinkedProgram,
    value: Value,
    context: &mut GuardExecutionContext<'_, '_>,
) -> VmResult<Value> {
    let Some(guard_id) = code.return_guard else {
        return Ok(value);
    };
    let guard = code.type_guard(guard_id).ok_or_else(|| {
        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "return_guard",
        })
    })?;
    execute_linked_guard(
        &value,
        guard,
        program,
        context,
        program.debug_name(guard.context.debug_name),
    )?;
    Ok(value)
}

pub(crate) fn execute_iterator_item_guard(
    value: &Value,
    guard: &IteratorItemGuard,
    runtime: &mut MethodRuntime<'_, '_, '_>,
) -> VmResult<()> {
    let mut context =
        GuardExecutionContext::new(runtime.heap.as_deref_mut(), runtime.budget.as_deref_mut());
    match guard {
        IteratorItemGuard::Unlinked { plan, debug_name } => {
            execute_unlinked_guard_plan(value, plan, &mut context, debug_name)
        }
        IteratorItemGuard::Linked { plan, debug_name } => {
            let Some(program) = runtime.linked_program else {
                return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "iterator_item_guard",
                }));
            };
            execute_linked_guard_plan(value, plan, program, &mut context, debug_name)
        }
    }
}

fn execute_primitive_guard(
    value: &Value,
    expected: PrimitiveTag,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_primitive_tag(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::TypeContractViolation {
        expected: primitive_type_name(expected).to_owned(),
        actual: runtime_type_name(value, heap).to_owned(),
        debug_name: debug_name.to_owned(),
    }))
}

fn execute_standard_guard(
    value: &Value,
    expected: StandardTypeGuard,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_standard_type(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(type_contract_error(
        value,
        standard_type_name(expected),
        heap,
        debug_name,
    ))
}

fn execute_iterator_guard(
    value: &Value,
    item_guard: Option<IteratorItemGuard>,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    if runtime_standard_type(value, heap) != Some(StandardTypeGuard::Iterator) {
        return Err(type_contract_error(value, "Iterator", heap, debug_name));
    }
    if let Some(item_guard) = item_guard {
        let Value::HeapRef(reference) = value else {
            return Err(type_contract_error(value, "Iterator", heap, debug_name));
        };
        let Some(HeapValue::Iterator(iterator)) = context
            .heap_mut()
            .and_then(|heap| heap.heap.get_mut(*reference).ok())
        else {
            return Err(type_contract_error(
                value,
                "Iterator",
                context.heap(),
                debug_name,
            ));
        };
        iterator.add_item_guard(item_guard);
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ContainerGuardKind {
    Array,
    Set,
    Map,
}

fn container_guard_type_name(kind: ContainerGuardKind) -> &'static str {
    match kind {
        ContainerGuardKind::Array => "Array",
        ContainerGuardKind::Set => "Set",
        ContainerGuardKind::Map => "Map",
    }
}

fn container_reference(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
    kind: ContainerGuardKind,
) -> VmResult<crate::heap::GcRef> {
    let type_name = container_guard_type_name(kind);
    let Value::HeapRef(reference) = value else {
        return Err(type_contract_error(value, type_name, heap, debug_name));
    };
    let matches_kind = heap
        .and_then(|heap| heap.heap.get(*reference))
        .is_some_and(|value| {
            matches!(
                (kind, value),
                (ContainerGuardKind::Array, HeapValue::Array(_))
                    | (ContainerGuardKind::Set, HeapValue::Set(_))
                    | (ContainerGuardKind::Map, HeapValue::Map(_))
            )
        });
    if matches_kind {
        Ok(*reference)
    } else {
        Err(type_contract_error(value, type_name, heap, debug_name))
    }
}

fn copied_container_values(
    reference: crate::heap::GcRef,
    heap: Option<&HeapExecution<'_>>,
    _debug_name: &str,
    kind: ContainerGuardKind,
) -> VmResult<Vec<Value>> {
    let type_name = container_guard_type_name(kind);
    let values = heap
        .and_then(|heap| heap.heap.get(reference))
        .and_then(|value| match (kind, value) {
            (ContainerGuardKind::Array, HeapValue::Array(values)) => Some(values.to_vec()),
            (ContainerGuardKind::Set, HeapValue::Set(values)) => Some(values.values_vec()),
            (ContainerGuardKind::Map, HeapValue::Map(values)) => {
                Some(values.values().copied().collect())
            }
            _ => None,
        });
    values.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: type_name,
        })
    })
}

fn copied_map_entries(
    reference: crate::heap::GcRef,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Vec<(Value, Value)>> {
    heap.and_then(|heap| heap.heap.get(reference))
        .and_then(|value| match value {
            HeapValue::Map(values) => Some(values.entries_vec()),
            _ => None,
        })
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation: "Map" }))
}

fn try_unlinked_container_contract_fast_path(
    reference: crate::heap::GcRef,
    stamp: &ContainerContractStamp,
    element: &UnlinkedTypeGuardPlan,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<bool> {
    let Some(heap) = context.heap_mut() else {
        return Ok(false);
    };
    if heap.heap.has_container_contract_stamp(reference, stamp) {
        return Ok(true);
    }
    let summary = heap
        .heap
        .container_value_summary(reference)
        .unwrap_or(ContainerTypeSummary::Unknown);
    match summary.prove_unlinked_plan(element) {
        ContainerSummaryProof::Proven => {
            heap.heap
                .install_container_contract_stamp(reference, stamp.clone());
            Ok(true)
        }
        ContainerSummaryProof::Mismatch(actual) => {
            Err(VmError::new(VmErrorKind::TypeContractViolation {
                expected: unlinked_plan_type_name(element).to_owned(),
                actual: actual.type_name().to_owned(),
                debug_name: debug_name.to_owned(),
            }))
        }
        ContainerSummaryProof::Unknown => Ok(false),
    }
}

fn try_linked_container_contract_fast_path(
    reference: crate::heap::GcRef,
    stamp: &ContainerContractStamp,
    element: &TypeGuardPlan,
    program: &LinkedProgram,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<bool> {
    let Some(heap) = context.heap_mut() else {
        return Ok(false);
    };
    if heap.heap.has_container_contract_stamp(reference, stamp) {
        return Ok(true);
    }
    let summary = heap
        .heap
        .container_value_summary(reference)
        .unwrap_or(ContainerTypeSummary::Unknown);
    match linked_summary_proof(summary, element, program) {
        ContainerSummaryProof::Proven => {
            heap.heap
                .install_container_contract_stamp(reference, stamp.clone());
            Ok(true)
        }
        ContainerSummaryProof::Mismatch(actual) => {
            Err(VmError::new(VmErrorKind::TypeContractViolation {
                expected: linked_plan_expected_name(element, program),
                actual: actual.type_name().to_owned(),
                debug_name: debug_name.to_owned(),
            }))
        }
        ContainerSummaryProof::Unknown => Ok(false),
    }
}

fn try_unlinked_map_contract_fast_path(
    reference: crate::heap::GcRef,
    stamp: &ContainerContractStamp,
    key_plan: Option<&UnlinkedTypeGuardPlan>,
    value_plan: Option<&UnlinkedTypeGuardPlan>,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<bool> {
    let Some(heap) = context.heap_mut() else {
        return Ok(false);
    };
    if heap.heap.has_container_contract_stamp(reference, stamp) {
        return Ok(true);
    }
    let key_proof = key_plan.map_or(Ok(ContainerSummaryProof::Proven), |plan| {
        map_summary_proof_unlinked(
            heap.heap
                .container_key_summary(reference)
                .unwrap_or(ContainerTypeSummary::Unknown),
            plan,
            debug_name,
        )
    })?;
    let value_proof = value_plan.map_or(Ok(ContainerSummaryProof::Proven), |plan| {
        map_summary_proof_unlinked(
            heap.heap
                .container_value_summary(reference)
                .unwrap_or(ContainerTypeSummary::Unknown),
            plan,
            debug_name,
        )
    })?;
    if key_proof == ContainerSummaryProof::Proven && value_proof == ContainerSummaryProof::Proven {
        heap.heap
            .install_container_contract_stamp(reference, stamp.clone());
        return Ok(true);
    }
    Ok(false)
}

fn try_linked_map_contract_fast_path(
    reference: crate::heap::GcRef,
    stamp: &ContainerContractStamp,
    key_plan: Option<&TypeGuardPlan>,
    value_plan: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<bool> {
    let Some(heap) = context.heap_mut() else {
        return Ok(false);
    };
    if heap.heap.has_container_contract_stamp(reference, stamp) {
        return Ok(true);
    }
    let key_proof = key_plan.map_or(Ok(ContainerSummaryProof::Proven), |plan| {
        map_summary_proof_linked(
            heap.heap
                .container_key_summary(reference)
                .unwrap_or(ContainerTypeSummary::Unknown),
            plan,
            program,
            debug_name,
        )
    })?;
    let value_proof = value_plan.map_or(Ok(ContainerSummaryProof::Proven), |plan| {
        map_summary_proof_linked(
            heap.heap
                .container_value_summary(reference)
                .unwrap_or(ContainerTypeSummary::Unknown),
            plan,
            program,
            debug_name,
        )
    })?;
    if key_proof == ContainerSummaryProof::Proven && value_proof == ContainerSummaryProof::Proven {
        heap.heap
            .install_container_contract_stamp(reference, stamp.clone());
        return Ok(true);
    }
    Ok(false)
}

fn map_summary_proof_unlinked(
    summary: ContainerTypeSummary,
    plan: &UnlinkedTypeGuardPlan,
    debug_name: &str,
) -> VmResult<ContainerSummaryProof> {
    match summary.prove_unlinked_plan(plan) {
        ContainerSummaryProof::Mismatch(actual) => {
            Err(VmError::new(VmErrorKind::TypeContractViolation {
                expected: unlinked_plan_type_name(plan).to_owned(),
                actual: actual.type_name().to_owned(),
                debug_name: debug_name.to_owned(),
            }))
        }
        proof => Ok(proof),
    }
}

fn map_summary_proof_linked(
    summary: ContainerTypeSummary,
    plan: &TypeGuardPlan,
    program: &LinkedProgram,
    debug_name: &str,
) -> VmResult<ContainerSummaryProof> {
    match linked_summary_proof(summary, plan, program) {
        ContainerSummaryProof::Mismatch(actual) => {
            Err(VmError::new(VmErrorKind::TypeContractViolation {
                expected: linked_plan_expected_name(plan, program),
                actual: actual.type_name().to_owned(),
                debug_name: debug_name.to_owned(),
            }))
        }
        proof => Ok(proof),
    }
}

fn linked_summary_proof(
    summary: ContainerTypeSummary,
    plan: &TypeGuardPlan,
    program: &LinkedProgram,
) -> ContainerSummaryProof {
    if let Some(key) = linked_exact_shallow_key(plan, program) {
        return summary.prove_exact_key(key);
    }
    summary.prove_linked_plan(plan)
}

fn linked_exact_shallow_key(
    plan: &TypeGuardPlan,
    program: &LinkedProgram,
) -> Option<ShallowTypeKey> {
    match plan {
        TypeGuardPlan::Shape { ty, shape_id } => program
            .ty(*ty)
            .map(|ty| ShallowTypeKey::Shape(ty.id, *shape_id)),
        TypeGuardPlan::Variant(variant) => program
            .variant(*variant)
            .map(|variant| ShallowTypeKey::Variant(variant.id)),
        _ => None,
    }
}

fn linked_plan_expected_name(plan: &TypeGuardPlan, program: &LinkedProgram) -> String {
    match plan {
        TypeGuardPlan::Shape { ty, .. } | TypeGuardPlan::Type(ty) => program
            .ty(*ty)
            .map(|ty| program.debug_name(ty.debug_name).to_owned())
            .unwrap_or_else(|| linked_plan_type_name(plan).to_owned()),
        TypeGuardPlan::Variant(variant) => program
            .variant(*variant)
            .map(|variant| program.debug_name(variant.debug_name).to_owned())
            .unwrap_or_else(|| linked_plan_type_name(plan).to_owned()),
        _ => linked_plan_type_name(plan).to_owned(),
    }
}

fn install_container_contract_stamp(
    reference: crate::heap::GcRef,
    stamp: ContainerContractStamp,
    context: &mut GuardExecutionContext<'_, '_>,
) {
    if let Some(heap) = context.heap_mut() {
        heap.heap.refresh_container_contracts(reference);
        heap.heap.install_container_contract_stamp(reference, stamp);
    }
}

fn register_container_contract_dependencies(
    parent: crate::heap::GcRef,
    values: &[Value],
    context: &mut GuardExecutionContext<'_, '_>,
) {
    let Some(heap) = context.heap_mut() else {
        return;
    };
    for value in values {
        if let Value::HeapRef(child) = value {
            heap.heap.add_container_contract_dependency(*child, parent);
        }
    }
}

fn execute_option_guard(
    value: &Value,
    some: Option<&UnlinkedTypeGuardPlan>,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    match std_enum_value(value, heap) {
        Some((StdEnumKind::Option, StdEnumVariant::Some, fields)) => {
            if let Some(some) = some {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Option payload contract",
                        })
                    })?;
                execute_unlinked_guard_plan(&payload, some, context, debug_name)?;
            }
            Ok(())
        }
        Some((StdEnumKind::Option, StdEnumVariant::None, _)) => Ok(()),
        _ => Err(type_contract_error(value, "Option", heap, debug_name)),
    }
}

fn execute_array_guard(
    value: &Value,
    element: Option<&UnlinkedTypeGuardPlan>,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    let reference = container_reference(value, heap, debug_name, ContainerGuardKind::Array)?;
    if let Some(element) = element {
        let stamp = ContainerContractStamp::Unlinked(UnlinkedTypeGuardPlan::Array {
            element: Some(Box::new(element.clone())),
        });
        if try_unlinked_container_contract_fast_path(
            reference, &stamp, element, context, debug_name,
        )? {
            return Ok(());
        }
        let values = copied_container_values(
            reference,
            context.heap(),
            debug_name,
            ContainerGuardKind::Array,
        )?;
        for value in &values {
            context.charge_scan_item()?;
            execute_unlinked_guard_plan(value, element, context, debug_name)?;
        }
        register_container_contract_dependencies(reference, &values, context);
        install_container_contract_stamp(reference, stamp, context);
    }
    Ok(())
}

fn execute_set_guard(
    value: &Value,
    element: Option<&UnlinkedTypeGuardPlan>,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    let reference = container_reference(value, heap, debug_name, ContainerGuardKind::Set)?;
    if let Some(element) = element {
        let stamp = ContainerContractStamp::Unlinked(UnlinkedTypeGuardPlan::Set {
            element: Some(Box::new(element.clone())),
        });
        if try_unlinked_container_contract_fast_path(
            reference, &stamp, element, context, debug_name,
        )? {
            return Ok(());
        }
        let values = copied_container_values(
            reference,
            context.heap(),
            debug_name,
            ContainerGuardKind::Set,
        )?;
        for value in &values {
            context.charge_scan_item()?;
            execute_unlinked_guard_plan(value, element, context, debug_name)?;
        }
        register_container_contract_dependencies(reference, &values, context);
        install_container_contract_stamp(reference, stamp, context);
    }
    Ok(())
}

fn execute_map_guard(
    value: &Value,
    key: Option<&UnlinkedTypeGuardPlan>,
    value_plan: Option<&UnlinkedTypeGuardPlan>,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    let reference = container_reference(value, heap, debug_name, ContainerGuardKind::Map)?;
    if key.is_none() && value_plan.is_none() {
        return Ok(());
    }
    let stamp = ContainerContractStamp::Unlinked(UnlinkedTypeGuardPlan::Map {
        key: key.cloned().map(Box::new),
        value: value_plan.cloned().map(Box::new),
    });
    if try_unlinked_map_contract_fast_path(reference, &stamp, key, value_plan, context, debug_name)?
    {
        return Ok(());
    }
    let entries = copied_map_entries(reference, context.heap())?;
    for (entry_key, entry_value) in &entries {
        if let Some(key_plan) = key {
            context.charge_scan_item()?;
            execute_unlinked_guard_plan(entry_key, key_plan, context, debug_name)?;
        }
        if let Some(value_plan) = value_plan {
            context.charge_scan_item()?;
            execute_unlinked_guard_plan(entry_value, value_plan, context, debug_name)?;
        }
    }
    let dependencies = entries
        .iter()
        .flat_map(|(key, value)| [key, value])
        .copied()
        .collect::<Vec<_>>();
    register_container_contract_dependencies(reference, &dependencies, context);
    install_container_contract_stamp(reference, stamp, context);
    Ok(())
}

fn execute_result_guard(
    value: &Value,
    ok: Option<&UnlinkedTypeGuardPlan>,
    err: Option<&UnlinkedTypeGuardPlan>,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    match std_enum_value(value, heap) {
        Some((StdEnumKind::Result, StdEnumVariant::Ok, fields)) => {
            if let Some(ok) = ok {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Result Ok payload contract",
                        })
                    })?;
                execute_unlinked_guard_plan(&payload, ok, context, debug_name)?;
            }
            Ok(())
        }
        Some((StdEnumKind::Result, StdEnumVariant::Err, fields)) => {
            if let Some(err) = err {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Result Err payload contract",
                        })
                    })?;
                execute_unlinked_guard_plan(&payload, err, context, debug_name)?;
            }
            Ok(())
        }
        _ => Err(type_contract_error(value, "Result", heap, debug_name)),
    }
}

fn execute_unlinked_guard_plan(
    value: &Value,
    plan: &UnlinkedTypeGuardPlan,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    match plan {
        UnlinkedTypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, *expected, heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Standard(expected) => {
            execute_standard_guard(value, *expected, heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Array { element } => {
            execute_array_guard(value, element.as_deref(), context, debug_name)
        }
        UnlinkedTypeGuardPlan::Map { key, value: values } => execute_map_guard(
            value,
            key.as_deref(),
            values.as_deref(),
            context,
            debug_name,
        ),
        UnlinkedTypeGuardPlan::Set { element } => {
            execute_set_guard(value, element.as_deref(), context, debug_name)
        }
        UnlinkedTypeGuardPlan::Iterator { item } => execute_iterator_guard(
            value,
            item.as_deref()
                .cloned()
                .map(|plan| IteratorItemGuard::unlinked(plan, debug_name.to_owned())),
            context,
            debug_name,
        ),
        UnlinkedTypeGuardPlan::Option { some } => {
            execute_option_guard(value, some.as_deref(), context, debug_name)
        }
        UnlinkedTypeGuardPlan::Result { ok, err } => {
            execute_result_guard(value, ok.as_deref(), err.as_deref(), context, debug_name)
        }
        UnlinkedTypeGuardPlan::Type(expected) => {
            execute_unlinked_type_guard(value, expected, heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Variant { enum_name, variant } => {
            execute_unlinked_variant_guard(value, enum_name, variant, heap, debug_name)
        }
        UnlinkedTypeGuardPlan::Shape {
            type_name,
            shape_id,
        } => execute_unlinked_shape_guard(value, type_name, *shape_id, heap, debug_name),
        UnlinkedTypeGuardPlan::HostType(_) => Ok(()),
    }
}

fn execute_linked_option_guard(
    value: &Value,
    some: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    match std_enum_value(value, heap) {
        Some((StdEnumKind::Option, StdEnumVariant::Some, fields)) => {
            if let Some(some) = some {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Option payload contract",
                        })
                    })?;
                execute_linked_guard_plan(&payload, some, program, context, debug_name)?;
            }
            Ok(())
        }
        Some((StdEnumKind::Option, StdEnumVariant::None, _)) => Ok(()),
        _ => Err(type_contract_error(value, "Option", heap, debug_name)),
    }
}

fn execute_linked_array_guard(
    value: &Value,
    element: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    let reference = container_reference(value, heap, debug_name, ContainerGuardKind::Array)?;
    if let Some(element) = element {
        let stamp = ContainerContractStamp::Linked(TypeGuardPlan::Array {
            element: Some(Box::new(element.clone())),
        });
        if try_linked_container_contract_fast_path(
            reference, &stamp, element, program, context, debug_name,
        )? {
            return Ok(());
        }
        let values = copied_container_values(
            reference,
            context.heap(),
            debug_name,
            ContainerGuardKind::Array,
        )?;
        for value in &values {
            context.charge_scan_item()?;
            execute_linked_guard_plan(value, element, program, context, debug_name)?;
        }
        register_container_contract_dependencies(reference, &values, context);
        install_container_contract_stamp(reference, stamp, context);
    }
    Ok(())
}

fn execute_linked_set_guard(
    value: &Value,
    element: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    let reference = container_reference(value, heap, debug_name, ContainerGuardKind::Set)?;
    if let Some(element) = element {
        let stamp = ContainerContractStamp::Linked(TypeGuardPlan::Set {
            element: Some(Box::new(element.clone())),
        });
        if try_linked_container_contract_fast_path(
            reference, &stamp, element, program, context, debug_name,
        )? {
            return Ok(());
        }
        let values = copied_container_values(
            reference,
            context.heap(),
            debug_name,
            ContainerGuardKind::Set,
        )?;
        for value in &values {
            context.charge_scan_item()?;
            execute_linked_guard_plan(value, element, program, context, debug_name)?;
        }
        register_container_contract_dependencies(reference, &values, context);
        install_container_contract_stamp(reference, stamp, context);
    }
    Ok(())
}

fn execute_linked_map_guard(
    value: &Value,
    key: Option<&TypeGuardPlan>,
    value_plan: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    let reference = container_reference(value, heap, debug_name, ContainerGuardKind::Map)?;
    if key.is_none() && value_plan.is_none() {
        return Ok(());
    }
    let stamp = ContainerContractStamp::Linked(TypeGuardPlan::Map {
        key: key.cloned().map(Box::new),
        value: value_plan.cloned().map(Box::new),
    });
    if try_linked_map_contract_fast_path(
        reference, &stamp, key, value_plan, program, context, debug_name,
    )? {
        return Ok(());
    }
    let entries = copied_map_entries(reference, context.heap())?;
    for (entry_key, entry_value) in &entries {
        if let Some(key_plan) = key {
            context.charge_scan_item()?;
            execute_linked_guard_plan(entry_key, key_plan, program, context, debug_name)?;
        }
        if let Some(value_plan) = value_plan {
            context.charge_scan_item()?;
            execute_linked_guard_plan(entry_value, value_plan, program, context, debug_name)?;
        }
    }
    let dependencies = entries
        .iter()
        .flat_map(|(key, value)| [key, value])
        .copied()
        .collect::<Vec<_>>();
    register_container_contract_dependencies(reference, &dependencies, context);
    install_container_contract_stamp(reference, stamp, context);
    Ok(())
}

fn execute_linked_result_guard(
    value: &Value,
    ok: Option<&TypeGuardPlan>,
    err: Option<&TypeGuardPlan>,
    program: &LinkedProgram,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    match std_enum_value(value, heap) {
        Some((StdEnumKind::Result, StdEnumVariant::Ok, fields)) => {
            if let Some(ok) = ok {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Result Ok payload contract",
                        })
                    })?;
                execute_linked_guard_plan(&payload, ok, program, context, debug_name)?;
            }
            Ok(())
        }
        Some((StdEnumKind::Result, StdEnumVariant::Err, fields)) => {
            if let Some(err) = err {
                let payload = fields
                    .get_slot(0, "0")
                    .map(stored_runtime_value)
                    .ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "Result Err payload contract",
                        })
                    })?;
                execute_linked_guard_plan(&payload, err, program, context, debug_name)?;
            }
            Ok(())
        }
        _ => Err(type_contract_error(value, "Result", heap, debug_name)),
    }
}

fn execute_linked_guard_plan(
    value: &Value,
    plan: &TypeGuardPlan,
    program: &LinkedProgram,
    context: &mut GuardExecutionContext<'_, '_>,
    debug_name: &str,
) -> VmResult<()> {
    let heap = context.heap();
    match plan {
        TypeGuardPlan::Primitive(expected) => {
            execute_primitive_guard(value, *expected, heap, debug_name)
        }
        TypeGuardPlan::Standard(expected) => {
            execute_standard_guard(value, *expected, heap, debug_name)
        }
        TypeGuardPlan::Array { element } => {
            execute_linked_array_guard(value, element.as_deref(), program, context, debug_name)
        }
        TypeGuardPlan::Map { key, value: values } => execute_linked_map_guard(
            value,
            key.as_deref(),
            values.as_deref(),
            program,
            context,
            debug_name,
        ),
        TypeGuardPlan::Set { element } => {
            execute_linked_set_guard(value, element.as_deref(), program, context, debug_name)
        }
        TypeGuardPlan::Iterator { item } => execute_iterator_guard(
            value,
            item.as_deref()
                .cloned()
                .map(|plan| IteratorItemGuard::linked(plan, debug_name.to_owned())),
            context,
            debug_name,
        ),
        TypeGuardPlan::Option { some } => {
            execute_linked_option_guard(value, some.as_deref(), program, context, debug_name)
        }
        TypeGuardPlan::Result { ok, err } => execute_linked_result_guard(
            value,
            ok.as_deref(),
            err.as_deref(),
            program,
            context,
            debug_name,
        ),
        TypeGuardPlan::Type(expected) => {
            let expected = program.ty(*expected).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "type_guard",
                })
            })?;
            execute_type_id_guard(
                value,
                expected.id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::Variant(expected) => {
            let expected = program.variant(*expected).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "variant_guard",
                })
            })?;
            execute_variant_id_guard(
                value,
                expected.id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::Shape { ty, shape_id } => {
            let expected = program.ty(*ty).ok_or_else(|| {
                VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                    opcode: "shape_guard",
                })
            })?;
            execute_shape_id_guard(
                value,
                expected.id,
                *shape_id,
                program.debug_name(expected.debug_name),
                heap,
                debug_name,
            )
        }
        TypeGuardPlan::HostType(_) => Ok(()),
    }
}

fn execute_type_id_guard(
    value: &Value,
    expected: vela_def::TypeId,
    expected_name: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_type_id(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(type_contract_error(value, expected_name, heap, debug_name))
}

fn execute_variant_id_guard(
    value: &Value,
    expected: vela_def::VariantId,
    expected_name: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_variant_id(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(type_contract_error(value, expected_name, heap, debug_name))
}

fn execute_shape_id_guard(
    value: &Value,
    expected_type: vela_def::TypeId,
    expected_shape: vela_common::ShapeId,
    expected_name: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_record_shape(value, heap) == Some((expected_type, expected_shape)) {
        return Ok(());
    }
    if runtime_record_debug_shape(value, heap) == Some((expected_name, expected_shape)) {
        return Ok(());
    }
    Err(type_contract_error(value, expected_name, heap, debug_name))
}

fn execute_unlinked_type_guard(
    value: &Value,
    expected: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_type_debug_name(value, heap) == Some(expected) {
        return Ok(());
    }
    Err(type_contract_error(value, expected, heap, debug_name))
}

fn execute_unlinked_variant_guard(
    value: &Value,
    expected_enum: &str,
    expected_variant: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    let expected = format!("{expected_enum}::{expected_variant}");
    if runtime_variant_debug_name(value, heap) == Some((expected_enum, expected_variant)) {
        return Ok(());
    }
    Err(type_contract_error(value, &expected, heap, debug_name))
}

fn execute_unlinked_shape_guard(
    value: &Value,
    expected_type: &str,
    expected_shape: vela_common::ShapeId,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmResult<()> {
    if runtime_record_debug_shape(value, heap) == Some((expected_type, expected_shape)) {
        return Ok(());
    }
    Err(type_contract_error(value, expected_type, heap, debug_name))
}

fn std_enum_value<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<(
    StdEnumKind,
    StdEnumVariant,
    &'a crate::script_object::ScriptFields<Value>,
)> {
    let Value::HeapRef(reference) = value else {
        return None;
    };
    let HeapValue::Enum {
        identity: Some(identity),
        fields,
        ..
    } = heap?.heap.get(*reference)?
    else {
        return None;
    };
    let (kind, variant) = std_enum_tag(*identity)?;
    Some((kind, variant, fields))
}

fn runtime_standard_type(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<StandardTypeGuard> {
    match value {
        Value::Range(_) => Some(StandardTypeGuard::Range),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Array(_)) => Some(StandardTypeGuard::Array),
            Some(HeapValue::Map(_)) => Some(StandardTypeGuard::Map),
            Some(HeapValue::Set(_)) => Some(StandardTypeGuard::Set),
            Some(HeapValue::Closure(_)) => Some(StandardTypeGuard::Closure),
            Some(HeapValue::Iterator(_)) => Some(StandardTypeGuard::Iterator),
            Some(HeapValue::Enum {
                identity: Some(identity),
                ..
            }) => match std_enum_tag(*identity) {
                Some((StdEnumKind::Option, _)) => Some(StandardTypeGuard::Option),
                Some((StdEnumKind::Result, _)) => Some(StandardTypeGuard::Result),
                None => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn standard_type_name(guard: StandardTypeGuard) -> &'static str {
    match guard {
        StandardTypeGuard::Array => "Array",
        StandardTypeGuard::Map => "Map",
        StandardTypeGuard::Set => "Set",
        StandardTypeGuard::Range => "Range",
        StandardTypeGuard::Function => "Function",
        StandardTypeGuard::Closure => "Closure",
        StandardTypeGuard::Iterator => "Iterator",
        StandardTypeGuard::Option => "Option",
        StandardTypeGuard::Result => "Result",
    }
}

fn unlinked_plan_type_name(plan: &UnlinkedTypeGuardPlan) -> &'static str {
    match plan {
        UnlinkedTypeGuardPlan::Primitive(tag) => primitive_type_name(*tag),
        UnlinkedTypeGuardPlan::Standard(guard) => standard_type_name(*guard),
        UnlinkedTypeGuardPlan::Array { .. } => "Array",
        UnlinkedTypeGuardPlan::Map { .. } => "Map",
        UnlinkedTypeGuardPlan::Set { .. } => "Set",
        UnlinkedTypeGuardPlan::Iterator { .. } => "Iterator",
        UnlinkedTypeGuardPlan::Option { .. } => "Option",
        UnlinkedTypeGuardPlan::Result { .. } => "Result",
        UnlinkedTypeGuardPlan::Type(_) => "record",
        UnlinkedTypeGuardPlan::Variant { .. } => "enum",
        UnlinkedTypeGuardPlan::Shape { .. } => "record",
        UnlinkedTypeGuardPlan::HostType(_) => "host",
    }
}

fn linked_plan_type_name(plan: &TypeGuardPlan) -> &'static str {
    match plan {
        TypeGuardPlan::Primitive(tag) => primitive_type_name(*tag),
        TypeGuardPlan::Standard(guard) => standard_type_name(*guard),
        TypeGuardPlan::Array { .. } => "Array",
        TypeGuardPlan::Map { .. } => "Map",
        TypeGuardPlan::Set { .. } => "Set",
        TypeGuardPlan::Iterator { .. } => "Iterator",
        TypeGuardPlan::Option { .. } => "Option",
        TypeGuardPlan::Result { .. } => "Result",
        TypeGuardPlan::Type(_) | TypeGuardPlan::Shape { .. } => "record",
        TypeGuardPlan::Variant(_) => "enum",
        TypeGuardPlan::HostType(_) => "host",
    }
}

const fn primitive_type_name(tag: PrimitiveTag) -> &'static str {
    match tag {
        PrimitiveTag::String => "String",
        PrimitiveTag::Bytes => "Bytes",
        _ => tag.name(),
    }
}

fn type_contract_error(
    value: &Value,
    expected: &str,
    heap: Option<&HeapExecution<'_>>,
    debug_name: &str,
) -> VmError {
    VmError::new(VmErrorKind::TypeContractViolation {
        expected: expected.to_owned(),
        actual: runtime_type_name(value, heap).to_owned(),
        debug_name: debug_name.to_owned(),
    })
}

macro_rules! define_runtime_type_helpers {
    ($($value_variant:ident => $primitive_tag:ident),* $(,)?) => {
        fn runtime_primitive_tag(
            value: &Value,
            heap: Option<&HeapExecution<'_>>,
        ) -> Option<PrimitiveTag> {
            match value {
                Value::Null => Some(PrimitiveTag::Null),
                Value::Bool(_) => Some(PrimitiveTag::Bool),
                Value::Char(_) => Some(PrimitiveTag::Char),
                $(
                    Value::$value_variant(_) => Some(PrimitiveTag::$primitive_tag),
                )*
                Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                    Some(HeapValue::String(_)) => Some(PrimitiveTag::String),
                    Some(HeapValue::Bytes(_)) => Some(PrimitiveTag::Bytes),
                    _ => None,
                },
                Value::Missing | Value::Range(_) | Value::HostRef(_) => None,
            }
        }

        fn runtime_type_name<'a>(
            value: &Value,
            heap: Option<&'a HeapExecution<'_>>,
        ) -> &'a str {
            match value {
                Value::Missing => "missing",
                Value::Null => primitive_type_name(PrimitiveTag::Null),
                Value::Bool(_) => primitive_type_name(PrimitiveTag::Bool),
                Value::Char(_) => primitive_type_name(PrimitiveTag::Char),
                $(
                    Value::$value_variant(_) => primitive_type_name(PrimitiveTag::$primitive_tag),
                )*
                Value::Range(_) => "Range",
                Value::HostRef(_) => "host",
                Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                    Some(HeapValue::String(_)) => primitive_type_name(PrimitiveTag::String),
                    Some(HeapValue::Bytes(_)) => primitive_type_name(PrimitiveTag::Bytes),
                    Some(HeapValue::Array(_)) => "Array",
                    Some(HeapValue::Map(_)) => "Map",
                    Some(HeapValue::Set(_)) => "Set",
                    Some(HeapValue::Record { .. }) => "record",
                    Some(HeapValue::Enum { .. }) => "enum",
                    Some(HeapValue::Closure(_)) => "Closure",
                    Some(HeapValue::PathProxy(_)) => "host_path",
                    Some(HeapValue::Iterator(_)) => "Iterator",
                    None => "heap",
                },
            }
        }
    };
}

define_runtime_type_helpers!(
    I8 => I8,
    I16 => I16,
    I32 => I32,
    I64 => I64,
    U8 => U8,
    U16 => U16,
    U32 => U32,
    U64 => U64,
    F32 => F32,
    F64 => F64,
);

fn runtime_type_id(value: &Value, heap: Option<&HeapExecution<'_>>) -> Option<vela_def::TypeId> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Record {
                identity: Some(identity),
                ..
            }) => Some(identity.type_id),
            Some(HeapValue::Enum {
                identity: Some(identity),
                ..
            }) => Some(identity.type_id),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_variant_id(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<vela_def::VariantId> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Enum {
                identity: Some(identity),
                ..
            }) => Some(identity.variant_id),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_record_shape(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<(vela_def::TypeId, vela_common::ShapeId)> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Record {
                identity: Some(identity),
                ..
            }) => Some((identity.type_id, identity.shape_id)),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_type_debug_name<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<&'a str> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Record { type_name, .. }) => Some(type_name),
            Some(HeapValue::Enum { enum_name, .. }) => Some(enum_name),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_variant_debug_name<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<(&'a str, &'a str)> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Enum {
                enum_name, variant, ..
            }) => Some((enum_name, variant)),
            _ => None,
        },
        _ => None,
    }
}

fn runtime_record_debug_shape<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<(&'a str, vela_common::ShapeId)> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Record {
                type_name,
                identity: Some(identity),
                ..
            }) => Some((type_name, identity.shape_id)),
            Some(HeapValue::Record {
                type_name, fields, ..
            }) => Some((type_name, fields.shape_id())),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use vela_bytecode::{LinkedProgram, LinkedType, TypeGuardPlan};
    use vela_common::PrimitiveTag;

    use super::*;
    use crate::collection_mutation::{
        clear_map, clear_set, extend_map_slots, push_array_slot, remove_map_slot, remove_set_slot,
    };
    use crate::heap::{HeapValue, RecordIdentity, ScriptHeap};
    use crate::script_map::ScriptMap;
    use crate::script_object::ScriptFields;
    use crate::script_set::ScriptSet;
    use crate::value_key::ValueKey;

    #[test]
    fn exact_container_summary_proves_simple_array_contract_without_scan() {
        let mut heap = ScriptHeap::new();
        let array =
            Value::HeapRef(heap.allocate(HeapValue::Array(vec![Value::I64(1), Value::I64(2)])));
        let program = LinkedProgram::new();
        let plan = TypeGuardPlan::Array {
            element: Some(Box::new(TypeGuardPlan::Primitive(PrimitiveTag::I64))),
        };
        let mut budget = ExecutionBudget::new(0, usize::MAX, usize::MAX);
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut context = GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));

        execute_linked_guard_plan(&array, &plan, &program, &mut context, "values")
            .expect("summary should prove array element contract");

        assert_eq!(budget.instructions_executed(), 0);
    }

    #[test]
    fn exact_container_summary_proves_record_set_shape_contract_without_scan() {
        let mut program = LinkedProgram::new();
        let player_name = program.intern_debug_name("Player");
        let player_type_id = vela_def::TypeId::new(0x701);
        let player_type = program.push_type(LinkedType::new(player_type_id, player_name));

        let mut heap = ScriptHeap::new();
        let player = record_value(&mut heap, "Player", player_type_id, "level", Value::I64(10));
        let shape_id = record_shape(&heap, player);
        let mut set = ScriptSet::new();
        set.insert_keyed(ValueKey::HeapIdentity(player), Value::HeapRef(player));
        let value = Value::HeapRef(heap.allocate(HeapValue::Set(set)));
        let plan = TypeGuardPlan::Set {
            element: Some(Box::new(TypeGuardPlan::Shape {
                ty: player_type,
                shape_id,
            })),
        };
        let mut budget = ExecutionBudget::new(0, usize::MAX, usize::MAX);
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut context = GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));

        execute_linked_guard_plan(&value, &plan, &program, &mut context, "players")
            .expect("record shape summary should prove set element contract");

        assert_eq!(budget.instructions_executed(), 0);
    }

    #[test]
    fn exact_container_summary_proves_record_map_key_contract_without_scan() {
        let mut program = LinkedProgram::new();
        let player_name = program.intern_debug_name("Player");
        let player_type_id = vela_def::TypeId::new(0x702);
        let player_type = program.push_type(LinkedType::new(player_type_id, player_name));

        let mut heap = ScriptHeap::new();
        let player = record_value(&mut heap, "Player", player_type_id, "level", Value::I64(10));
        let shape_id = record_shape(&heap, player);
        let mut map = ScriptMap::new();
        map.insert_keyed(
            ValueKey::HeapIdentity(player),
            Value::HeapRef(player),
            Value::I64(42),
        );
        let value = Value::HeapRef(heap.allocate(HeapValue::Map(map)));
        let plan = TypeGuardPlan::Map {
            key: Some(Box::new(TypeGuardPlan::Shape {
                ty: player_type,
                shape_id,
            })),
            value: Some(Box::new(TypeGuardPlan::Primitive(PrimitiveTag::I64))),
        };
        let mut budget = ExecutionBudget::new(0, usize::MAX, usize::MAX);
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut context = GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));

        execute_linked_guard_plan(&value, &plan, &program, &mut context, "scores")
            .expect("record shape summary should prove map key contract");

        assert_eq!(budget.instructions_executed(), 0);
    }

    #[test]
    fn nested_container_stamp_skips_rescan_until_child_mutation() {
        let mut heap = ScriptHeap::new();
        let inner = heap.allocate(HeapValue::Array(vec![Value::I64(1), Value::I64(2)]));
        let outer = Value::HeapRef(heap.allocate(HeapValue::Array(vec![Value::HeapRef(inner)])));
        let program = LinkedProgram::new();
        let plan = TypeGuardPlan::Array {
            element: Some(Box::new(TypeGuardPlan::Array {
                element: Some(Box::new(TypeGuardPlan::Primitive(PrimitiveTag::I64))),
            })),
        };
        let mut budget = ExecutionBudget::new(1, usize::MAX, usize::MAX);

        {
            let mut heap_execution = HeapExecution::new(&mut heap);
            let mut context =
                GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));

            execute_linked_guard_plan(&outer, &plan, &program, &mut context, "values")
                .expect("first nested guard should scan");

            execute_linked_guard_plan(&outer, &plan, &program, &mut context, "values")
                .expect("matching stamp should skip second scan");
        }
        assert_eq!(budget.instructions_executed(), 1);

        let bad = heap.allocate(HeapValue::String("bad".to_owned()));
        {
            let mut heap_execution = HeapExecution::new(&mut heap);
            push_array_slot(
                &mut heap_execution,
                inner,
                Value::HeapRef(bad),
                None,
                "test inner mutation",
            )
            .expect("inner mutation should succeed");
        }

        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::unbounded();
        let mut context = GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));
        let error = execute_linked_guard_plan(&outer, &plan, &program, &mut context, "values")
            .expect_err("child mutation must invalidate parent stamp");

        assert_eq!(
            error.kind(),
            VmErrorKind::TypeContractViolation {
                expected: "i64".to_owned(),
                actual: "String".to_owned(),
                debug_name: "values".to_owned(),
            }
        );
    }

    #[test]
    fn mixed_map_extend_updates_key_summary_for_new_keys() {
        let mut map = ScriptMap::new();
        map.insert(Value::I64(1), Value::I64(10), None, "test map")
            .expect("initial map key should be keyable");

        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Map(map));
        let value = Value::HeapRef(reference);
        let program = LinkedProgram::new();
        let plan = TypeGuardPlan::Map {
            key: Some(Box::new(TypeGuardPlan::Primitive(PrimitiveTag::I64))),
            value: None,
        };

        {
            let mut heap_execution = HeapExecution::new(&mut heap);
            let mut budget = ExecutionBudget::new(0, usize::MAX, usize::MAX);
            let mut context =
                GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));
            execute_linked_guard_plan(&value, &plan, &program, &mut context, "scores")
                .expect("initial i64-key map should satisfy guard from summary");
        }

        {
            let mut heap_execution = HeapExecution::new(&mut heap);
            extend_map_slots(
                &mut heap_execution,
                reference,
                vec![
                    (Value::I64(1), Value::I64(11)),
                    (Value::Bool(true), Value::I64(20)),
                ],
                None,
                "test map extend",
            )
            .expect("mixed replacement and insertion should mutate map");
        }

        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::unbounded();
        let mut context = GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));
        let error = execute_linked_guard_plan(&value, &plan, &program, &mut context, "scores")
            .expect_err("new bool key must not be hidden by stale key summary");

        assert_eq!(
            error.kind(),
            VmErrorKind::TypeContractViolation {
                expected: "i64".to_owned(),
                actual: "bool".to_owned(),
                debug_name: "scores".to_owned(),
            }
        );
    }

    #[test]
    fn cleared_map_guard_does_not_use_stale_key_summary() {
        let mut map = ScriptMap::new();
        map.insert(Value::I64(1), Value::I64(10), None, "test map")
            .expect("initial map key should be keyable");

        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Map(map));
        let value = Value::HeapRef(reference);
        let program = LinkedProgram::new();
        let plan = TypeGuardPlan::Map {
            key: Some(Box::new(TypeGuardPlan::Primitive(PrimitiveTag::String))),
            value: None,
        };

        {
            let mut heap_execution = HeapExecution::new(&mut heap);
            clear_map(&mut heap_execution, reference, None, "test map clear")
                .expect("map clear should succeed");
        }

        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(0, usize::MAX, usize::MAX);
        let mut context = GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));
        execute_linked_guard_plan(&value, &plan, &program, &mut context, "scores")
            .expect("empty map should satisfy a different key contract after clear");

        assert_eq!(budget.instructions_executed(), 0);
    }

    #[test]
    fn removed_map_key_guard_does_not_use_stale_key_summary() {
        let mut map = ScriptMap::new();
        map.insert(Value::I64(1), Value::I64(10), None, "test map")
            .expect("initial map key should be keyable");

        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Map(map));
        let value = Value::HeapRef(reference);
        let program = LinkedProgram::new();
        let plan = TypeGuardPlan::Map {
            key: Some(Box::new(TypeGuardPlan::Primitive(PrimitiveTag::String))),
            value: None,
        };

        {
            let mut heap_execution = HeapExecution::new(&mut heap);
            remove_map_slot(
                &mut heap_execution,
                reference,
                &Value::I64(1),
                None,
                "test map remove",
            )
            .expect("map remove should succeed");
        }

        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::unbounded();
        let mut context = GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));
        execute_linked_guard_plan(&value, &plan, &program, &mut context, "scores")
            .expect("empty map should satisfy a different key contract after removal");
    }

    #[test]
    fn cleared_set_guard_does_not_use_stale_value_summary() {
        let mut set = ScriptSet::new();
        set.insert(Value::I64(1), None, "test set")
            .expect("initial set value should be keyable");

        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Set(set));
        let value = Value::HeapRef(reference);
        let program = LinkedProgram::new();
        let plan = TypeGuardPlan::Set {
            element: Some(Box::new(TypeGuardPlan::Primitive(PrimitiveTag::String))),
        };

        {
            let mut heap_execution = HeapExecution::new(&mut heap);
            clear_set(&mut heap_execution, reference, None, "test set clear")
                .expect("set clear should succeed");
        }

        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(0, usize::MAX, usize::MAX);
        let mut context = GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));
        execute_linked_guard_plan(&value, &plan, &program, &mut context, "values")
            .expect("empty set should satisfy a different element contract after clear");

        assert_eq!(budget.instructions_executed(), 0);
    }

    #[test]
    fn removed_set_value_guard_does_not_use_stale_value_summary() {
        let mut set = ScriptSet::new();
        set.insert(Value::I64(1), None, "test set")
            .expect("initial set value should be keyable");

        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Set(set));
        let value = Value::HeapRef(reference);
        let program = LinkedProgram::new();
        let plan = TypeGuardPlan::Set {
            element: Some(Box::new(TypeGuardPlan::Primitive(PrimitiveTag::String))),
        };

        {
            let mut heap_execution = HeapExecution::new(&mut heap);
            let key = ValueKey::from_value(&Value::I64(1), Some(&heap_execution), "test set")
                .expect("set value should be keyable");
            remove_set_slot(
                &mut heap_execution,
                reference,
                &key,
                None,
                "test set remove",
            )
            .expect("set remove should succeed");
        }

        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::unbounded();
        let mut context = GuardExecutionContext::new(Some(&mut heap_execution), Some(&mut budget));
        execute_linked_guard_plan(&value, &plan, &program, &mut context, "values")
            .expect("empty set should satisfy a different element contract after removal");
    }

    fn record_value(
        heap: &mut ScriptHeap,
        type_name: &str,
        type_id: vela_def::TypeId,
        field_name: &str,
        field_value: Value,
    ) -> crate::heap::GcRef {
        let fields = ScriptFields::single(type_name, field_name, field_value);
        let shape_id = fields.shape_id();
        heap.allocate(HeapValue::Record {
            type_name: type_name.to_owned(),
            identity: Some(RecordIdentity::new(type_id, shape_id)),
            fields,
        })
    }

    fn record_shape(heap: &ScriptHeap, reference: crate::heap::GcRef) -> vela_common::ShapeId {
        match heap.get(reference) {
            Some(HeapValue::Record {
                identity: Some(identity),
                ..
            }) => identity.shape_id,
            _ => panic!("test fixture should allocate a typed record"),
        }
    }
}
