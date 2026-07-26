use std::collections::{BTreeMap, BTreeSet};

use crate::budget::ExecutionBudget;
use crate::error::{VmError, VmErrorKind, VmResult};
use crate::heap::{GcRef, HeapValue, ScriptHeap};
use crate::heap_execution::HeapExecution;
use crate::script_map::ScriptMap;
use crate::script_object::ScriptFields;
use crate::script_set::ScriptSet;
use crate::small_storage::SmallStorage;
use crate::value::{ClosureValue, Value};

impl ScriptHeap {
    /// Counts linked-artifact owners that are reachable only from `roots`.
    ///
    /// Owners also reachable from `external_roots` are deliberately excluded so
    /// runtime generation reclamation can distinguish self-rooting state values
    /// from closures retained by callers or by the active generation.
    #[must_use]
    pub fn linked_owner_counts_exclusive_to_roots(
        &self,
        roots: &[Value],
        external_roots: &[Value],
    ) -> BTreeMap<vela_bytecode::ExecutableGenerationId, usize> {
        let internal = reachable_from_values(self, roots);
        let external = reachable_from_values(self, external_roots);
        let mut counts = BTreeMap::new();

        for reference in internal.difference(&external) {
            let Some(HeapValue::Closure(closure)) = self.get(*reference) else {
                continue;
            };
            *counts.entry(closure.owner.generation()).or_insert(0) += 1;
        }

        counts
    }
}

fn reachable_from_values(heap: &ScriptHeap, roots: &[Value]) -> BTreeSet<GcRef> {
    let mut pending = Vec::new();
    roots
        .iter()
        .for_each(|value| value.trace_heap_refs(&mut pending));
    let mut reachable = BTreeSet::new();
    while let Some(reference) = pending.pop() {
        if !reachable.insert(reference) {
            continue;
        }
        if let Some(value) = heap.get(reference) {
            value.trace_refs(&mut pending);
        }
    }
    reachable
}

pub fn copy_persistent_value_graph(
    roots: &[Value],
    source: &ScriptHeap,
    target: &mut ScriptHeap,
    budget: &mut ExecutionBudget,
) -> VmResult<Vec<Value>> {
    let reachable = reachable_objects(roots, source)?;
    let mut references = BTreeMap::new();
    for source_ref in &reachable {
        let value = source
            .get(*source_ref)
            .ok_or_else(|| invalid_graph("missing source heap object"))?
            .clone();
        let target_ref = target.allocate_with_budget(value, budget)?;
        references.insert(*source_ref, target_ref);
    }

    for source_ref in &reachable {
        let target_ref = references[source_ref];
        let source_value = source
            .get(*source_ref)
            .ok_or_else(|| invalid_graph("missing source heap object"))?;
        let copied = remap_heap_value(source_value, &references, target)?;
        *target
            .get_mut(target_ref)
            .map_err(|_| invalid_graph("missing staged target heap object"))? = copied;
        target.refresh_container_contracts(target_ref);
    }

    roots
        .iter()
        .map(|value| remap_value(*value, &references))
        .collect()
}

fn reachable_objects(roots: &[Value], source: &ScriptHeap) -> VmResult<Vec<GcRef>> {
    let mut pending = Vec::new();
    for root in roots {
        root.trace_heap_refs(&mut pending);
    }
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    while let Some(reference) = pending.pop() {
        if !visited.insert(reference) {
            continue;
        }
        let value = source
            .get(reference)
            .ok_or_else(|| invalid_graph("invalid source heap reference"))?;
        value.trace_refs(&mut pending);
        ordered.push(reference);
    }
    Ok(ordered)
}

fn remap_heap_value(
    value: &HeapValue,
    references: &BTreeMap<GcRef, GcRef>,
    target: &mut ScriptHeap,
) -> VmResult<HeapValue> {
    Ok(match value {
        HeapValue::String(value) => HeapValue::String(value.clone()),
        HeapValue::Bytes(value) => HeapValue::Bytes(value.clone()),
        HeapValue::Range(value) => HeapValue::Range(*value),
        HeapValue::Tuple(values) => HeapValue::Tuple(remap_values(values, references)?),
        HeapValue::Array(values) => HeapValue::Array(remap_values(values, references)?),
        HeapValue::Map(values) => {
            let entries = values
                .entries()
                .map(|entry| {
                    Ok((
                        remap_value(entry.key, references)?,
                        remap_value(entry.value, references)?,
                    ))
                })
                .collect::<VmResult<Vec<_>>>()?;
            let execution = HeapExecution::new(target);
            HeapValue::Map(ScriptMap::from_entries(
                entries,
                Some(&execution),
                "persistent heap graph copy",
            )?)
        }
        HeapValue::Set(values) => {
            let values = values
                .values()
                .map(|value| remap_value(*value, references))
                .collect::<VmResult<Vec<_>>>()?;
            let execution = HeapExecution::new(target);
            HeapValue::Set(ScriptSet::from_values(
                values,
                Some(&execution),
                "persistent heap graph copy",
            )?)
        }
        HeapValue::Record { identity, fields } => HeapValue::Record {
            identity: *identity,
            fields: remap_fields(fields.owner_name(), fields, references)?,
        },
        HeapValue::Enum {
            enum_name,
            variant,
            identity,
            fields,
        } => HeapValue::Enum {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            identity: *identity,
            fields: remap_fields(
                &crate::heap_values::enum_variant_owner(enum_name, variant),
                fields,
                references,
            )?,
        },
        HeapValue::Closure(closure) => HeapValue::Closure(ClosureValue {
            owner: std::sync::Arc::clone(&closure.owner),
            function: closure.function,
            captures: SmallStorage::try_from_slice_map(
                closure.captures.as_slice(),
                4,
                |capture| remap_value(*capture, references),
            )?,
        }),
        HeapValue::Iterator(iterator) => {
            let mut iterator = iterator.clone();
            iterator.remap_heap_refs(references)?;
            HeapValue::Iterator(iterator)
        }
        HeapValue::PathProxy(proxy) => HeapValue::PathProxy(proxy.clone()),
    })
}

fn remap_fields(
    owner: &str,
    fields: &ScriptFields<Value>,
    references: &BTreeMap<GcRef, GcRef>,
) -> VmResult<ScriptFields<Value>> {
    fields
        .iter()
        .map(|(name, value)| Ok((name.to_owned(), remap_value(*value, references)?)))
        .collect::<VmResult<Vec<_>>>()
        .map(|fields| ScriptFields::from_pairs(owner, fields))
}

fn remap_values(values: &[Value], references: &BTreeMap<GcRef, GcRef>) -> VmResult<Vec<Value>> {
    values
        .iter()
        .map(|value| remap_value(*value, references))
        .collect()
}

fn remap_value(value: Value, references: &BTreeMap<GcRef, GcRef>) -> VmResult<Value> {
    match value {
        Value::HeapRef(reference) => references
            .get(&reference)
            .copied()
            .map(Value::HeapRef)
            .ok_or_else(|| invalid_graph("unmapped source heap reference")),
        value => Ok(value),
    }
}

fn invalid_graph(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}

#[cfg(test)]
mod tests {
    use super::copy_persistent_value_graph;
    use crate::budget::ExecutionBudget;
    use crate::heap::{HeapValue, ScriptHeap};
    use crate::value::Value;

    #[test]
    fn graph_copy_preserves_aliases_and_cycles() {
        let mut source = ScriptHeap::new();
        let shared = source.allocate(HeapValue::Array(vec![Value::i64(7)]));
        let root = source.allocate(HeapValue::Array(Vec::new()));
        *source.get_mut(root).expect("root exists") = HeapValue::Array(vec![
            Value::HeapRef(shared),
            Value::HeapRef(shared),
            Value::HeapRef(root),
        ]);
        let mut target = ScriptHeap::new();
        let mut budget = ExecutionBudget::new(100, 4096, 16);

        let copied =
            copy_persistent_value_graph(&[Value::HeapRef(root)], &source, &mut target, &mut budget)
                .expect("cyclic graph copies");
        let Value::HeapRef(copied_root) = copied[0] else {
            panic!("copied root should remain managed");
        };
        let Some(HeapValue::Array(values)) = target.get(copied_root) else {
            panic!("copied root should remain an array");
        };

        assert_eq!(values[0], values[1], "shared child alias must be retained");
        assert_eq!(values[2], Value::HeapRef(copied_root));
    }

    #[test]
    fn graph_copy_charges_before_allocation_and_terminates_on_cycles() {
        let mut source = ScriptHeap::new();
        let root = source.allocate(HeapValue::Array(Vec::new()));
        *source.get_mut(root).expect("root exists") = HeapValue::Array(vec![Value::HeapRef(root)]);
        let mut target = ScriptHeap::new();
        let mut budget = ExecutionBudget::new(100, 1, 16);

        let error =
            copy_persistent_value_graph(&[Value::HeapRef(root)], &source, &mut target, &mut budget)
                .expect_err("insufficient graph-copy budget should fail");

        assert!(matches!(
            error.kind(),
            crate::error::VmErrorKind::BudgetExceeded { .. }
        ));
        assert_eq!(target.live_object_count(), 0);
    }
}
