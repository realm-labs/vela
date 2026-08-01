use std::collections::BTreeSet;

use vela_common::NonDetachableValueKind;

use crate::budget::{CollectionLimits, ExecutionBudget};
use crate::error::{VmError, VmErrorKind, VmResult};
use crate::heap::{HeapValue, ScriptHeap};
use crate::value::Value;

/// Owned, Runtime-independent image of values crossing a detached-task boundary.
///
/// The private heap preserves aliases and cycles without retaining the source
/// Runtime. It is an execution-boundary image, not a persistence or artifact
/// serialization format.
#[derive(Clone, Debug)]
pub struct DetachedValueImage {
    heap: ScriptHeap,
    roots: Vec<Value>,
    transfer_units: u64,
}

impl DetachedValueImage {
    /// Exports all task arguments as one graph so aliases between arguments
    /// remain observable in the child Runtime.
    pub fn export_arguments(
        roots: &[Value],
        source: Option<&ScriptHeap>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Self> {
        Self::export(roots, source, budget, "argument")
    }

    /// Exports a successful task result before its child Runtime is torn down.
    pub fn export_result(
        root: Value,
        source: &ScriptHeap,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Self> {
        Self::export(&[root], Some(source), budget, "result")
    }

    fn export(
        roots: &[Value],
        source: Option<&ScriptHeap>,
        budget: &mut ExecutionBudget,
        root_name: &'static str,
    ) -> VmResult<Self> {
        let metrics = validate_graph(roots, source, budget.collection_limits(), root_name)?;
        let empty = ScriptHeap::new();
        let source = source.unwrap_or(&empty);
        let mut staged_budget = budget.clone();
        staged_budget.charge_execution_units(metrics.transfer_units)?;
        let mut heap = ScriptHeap::new();
        let roots =
            crate::copy_persistent_value_graph(roots, source, &mut heap, &mut staged_budget)?;
        *budget = staged_budget;
        Ok(Self {
            heap,
            roots,
            transfer_units: metrics.transfer_units,
        })
    }

    /// Imports every root into one target heap, preserving graph identity both
    /// within and between roots.
    pub fn import_into(
        &self,
        target: &mut ScriptHeap,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Vec<Value>> {
        let _ = validate_graph(
            &self.roots,
            Some(&self.heap),
            budget.collection_limits(),
            "image",
        )?;

        // Preflight all charges against a clone. The target mutation below is
        // infallible for a validated image unless an internal graph invariant
        // is broken, while budget failure leaves both counters and heap alone.
        let mut staged_budget = budget.clone();
        staged_budget.charge_execution_units(self.transfer_units)?;
        staged_budget.charge_memory_bytes(self.heap.allocated_bytes())?;
        let mut unbounded = ExecutionBudget::unbounded();
        let roots =
            crate::copy_persistent_value_graph(&self.roots, &self.heap, target, &mut unbounded)?;
        *budget = staged_budget;
        Ok(roots)
    }

    #[must_use]
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        self.heap.allocated_bytes()
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct GraphMetrics {
    transfer_units: u64,
}

fn validate_graph(
    roots: &[Value],
    source: Option<&ScriptHeap>,
    limits: CollectionLimits,
    root_name: &'static str,
) -> VmResult<GraphMetrics> {
    let mut pending = roots
        .iter()
        .enumerate()
        .map(|(index, value)| (*value, format!("{root_name}[{index}]")))
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut units = u64::try_from(roots.len()).unwrap_or(u64::MAX);

    while let Some((value, path)) = pending.pop() {
        match value {
            Value::HostRef(_) => {
                return Err(not_detachable(path, NonDetachableValueKind::HostReference));
            }
            Value::HeapRef(reference) => {
                if !visited.insert(reference) {
                    continue;
                }
                let heap_value = source
                    .and_then(|heap| heap.get(reference))
                    .ok_or_else(|| invalid_graph("detached value export"))?;
                units = units.saturating_add(1);
                validate_heap_value(heap_value, &path, limits, &mut pending, &mut units)?;
            }
            Value::Missing
            | Value::Unit
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
            | Value::F64(_) => {}
        }
    }
    Ok(GraphMetrics {
        transfer_units: units,
    })
}

fn validate_heap_value(
    value: &HeapValue,
    path: &str,
    limits: CollectionLimits,
    pending: &mut Vec<(Value, String)>,
    units: &mut u64,
) -> VmResult<()> {
    match value {
        HeapValue::String(_) | HeapValue::Bytes(_) | HeapValue::Range(_) => {}
        HeapValue::Tuple(values) => push_indexed(values, path, pending, units),
        HeapValue::Array(values) => {
            check_collection_limit("array", values.len(), limits.max_array_len)?;
            push_indexed(values, path, pending, units);
        }
        HeapValue::Map(values) => {
            check_collection_limit("map", values.len(), limits.max_map_entries)?;
            for (index, entry) in values.entries().enumerate() {
                pending.push((entry.key, format!("{path}.key[{index}]")));
                pending.push((entry.value, format!("{path}.value[{index}]")));
                *units = units.saturating_add(2);
            }
        }
        HeapValue::Set(values) => {
            check_collection_limit("set", values.len(), limits.max_set_len)?;
            let members = values.values().copied().collect::<Vec<_>>();
            push_indexed(&members, path, pending, units);
        }
        HeapValue::Record { fields, .. } | HeapValue::Enum { fields, .. } => {
            for (name, value) in fields.iter() {
                pending.push((*value, format!("{path}.{name}")));
                *units = units.saturating_add(1);
            }
        }
        HeapValue::Closure(_) => {
            return Err(not_detachable(
                path.to_owned(),
                NonDetachableValueKind::Callable,
            ));
        }
        HeapValue::Iterator(_) => {
            return Err(not_detachable(
                path.to_owned(),
                NonDetachableValueKind::Iterator,
            ));
        }
        HeapValue::PathProxy(_) => {
            return Err(not_detachable(
                path.to_owned(),
                NonDetachableValueKind::HostReference,
            ));
        }
    }
    Ok(())
}

fn push_indexed(values: &[Value], path: &str, pending: &mut Vec<(Value, String)>, units: &mut u64) {
    for (index, value) in values.iter().enumerate() {
        pending.push((*value, format!("{path}[{index}]")));
        *units = units.saturating_add(1);
    }
}

fn check_collection_limit(collection: &'static str, len: usize, limit: usize) -> VmResult<()> {
    if len > limit {
        return Err(VmError::new(VmErrorKind::CollectionLimitExceeded {
            collection,
            limit,
        }));
    }
    Ok(())
}

fn not_detachable(path: String, kind: NonDetachableValueKind) -> VmError {
    VmError::new(VmErrorKind::TaskValueNotDetachable { path, kind })
}

fn invalid_graph(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}

#[cfg(test)]
mod tests {
    use super::DetachedValueImage;
    use crate::budget::{CollectionLimits, ExecutionBudget, ExecutionLimits};
    use crate::heap::{HeapValue, ScriptHeap};
    use crate::value::Value;

    #[test]
    fn detached_image_preserves_aliases_and_cycles_across_roots() {
        let mut source = ScriptHeap::new();
        let shared = source.allocate(HeapValue::Array(vec![Value::i64(7)]));
        let cyclic = source.allocate(HeapValue::Array(Vec::new()));
        *source.get_mut(cyclic).expect("cyclic value exists") = HeapValue::Array(vec![
            Value::HeapRef(shared),
            Value::HeapRef(shared),
            Value::HeapRef(cyclic),
        ]);
        let mut export_budget = ExecutionBudget::new(100, 4096, 16);

        let image = DetachedValueImage::export_arguments(
            &[Value::HeapRef(cyclic), Value::HeapRef(shared)],
            Some(&source),
            &mut export_budget,
        )
        .expect("detachable graph exports");
        assert!(export_budget.execution_units_consumed() > 0);
        assert_eq!(
            export_budget.memory_bytes_allocated(),
            image.allocated_bytes()
        );

        let mut target = ScriptHeap::new();
        let mut import_budget = ExecutionBudget::new(100, 4096, 16);
        let roots = image
            .import_into(&mut target, &mut import_budget)
            .expect("detached graph imports");
        let (Value::HeapRef(copied_root), Value::HeapRef(copied_shared)) = (roots[0], roots[1])
        else {
            panic!("managed roots remain managed");
        };
        let Some(HeapValue::Array(values)) = target.get(copied_root) else {
            panic!("copied root remains an array");
        };
        assert_eq!(values[0], Value::HeapRef(copied_shared));
        assert_eq!(values[0], values[1]);
        assert_eq!(values[2], Value::HeapRef(copied_root));
        assert!(import_budget.execution_units_consumed() > 0);
        assert_eq!(
            import_budget.memory_bytes_allocated(),
            image.allocated_bytes()
        );
    }

    #[test]
    fn detached_image_rejects_nested_runtime_capability_with_path() {
        let mut source = ScriptHeap::new();
        let nested = source.allocate(HeapValue::Array(vec![Value::HostRef(
            vela_host::path::HostSlotRef::new(3, 1),
        )]));
        let mut budget = ExecutionBudget::new(100, 4096, 16);

        let error = DetachedValueImage::export_arguments(
            &[Value::HeapRef(nested)],
            Some(&source),
            &mut budget,
        )
        .expect_err("host reference must not detach");

        assert!(matches!(
            error.kind(),
            crate::error::VmErrorKind::TaskValueNotDetachable { path, kind }
                if path == "argument[0][0]"
                    && kind == vela_common::NonDetachableValueKind::HostReference
        ));
        assert_eq!(budget.execution_units_consumed(), 0);
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn detached_image_budget_failure_is_transactional() {
        let mut source = ScriptHeap::new();
        let root = source.allocate(HeapValue::Array(vec![Value::i64(1)]));
        let limits = ExecutionLimits::new(100, 1, 16).with_collection_limits(CollectionLimits {
            max_array_len: 8,
            max_map_entries: 8,
            max_set_len: 8,
        });
        let mut budget = ExecutionBudget::with_limits(limits);

        let error = DetachedValueImage::export_arguments(
            &[Value::HeapRef(root)],
            Some(&source),
            &mut budget,
        )
        .expect_err("transfer exceeds memory budget");

        assert!(matches!(
            error.kind(),
            crate::error::VmErrorKind::BudgetExceeded { .. }
        ));
        assert_eq!(budget.execution_units_consumed(), 0);
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }
}
