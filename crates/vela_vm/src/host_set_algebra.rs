use vela_bytecode::{CacheSiteId, Register};

use crate::host_access::HostAccessRuntime;
use crate::script_set::ScriptSet;
use crate::set_methods::{SetCombination, SetRelation};
use crate::std_method_ids::HostSetAlgebra;
use crate::{Value, VmError, VmErrorKind, VmResult};

pub(crate) fn execute_host_root_set_algebra(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    algebra: HostSetAlgebra,
    args: &[Value],
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: algebra.name().to_owned(),
            expected: 1,
            actual: args.len(),
        }));
    }
    let operation = algebra.operation();
    if !crate::set_methods::is_set(&args[0], runtime.heap.as_deref()) {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    }

    let values = crate::host_collection_projection::project_host_root_collection_items(
        &mut runtime,
        receiver,
        cache_site,
    )?;
    let result = {
        let heap = runtime
            .heap
            .as_deref()
            .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
        let left = ScriptSet::from_values(values, Some(heap), operation)?;
        let right = crate::set_methods::set_slots(&args[0], Some(heap), operation)?;
        match algebra {
            HostSetAlgebra::Union => {
                ProjectedSetAlgebra::Values(crate::set_methods::combination_payload(
                    &left,
                    right,
                    Some(heap),
                    SetCombination::Union,
                    operation,
                )?)
            }
            HostSetAlgebra::Intersection => {
                ProjectedSetAlgebra::Values(crate::set_methods::combination_payload(
                    &left,
                    right,
                    Some(heap),
                    SetCombination::Intersection,
                    operation,
                )?)
            }
            HostSetAlgebra::Difference => {
                ProjectedSetAlgebra::Values(crate::set_methods::combination_payload(
                    &left,
                    right,
                    Some(heap),
                    SetCombination::Difference,
                    operation,
                )?)
            }
            HostSetAlgebra::SymmetricDifference => {
                ProjectedSetAlgebra::Values(crate::set_methods::combination_payload(
                    &left,
                    right,
                    Some(heap),
                    SetCombination::SymmetricDifference,
                    operation,
                )?)
            }
            HostSetAlgebra::IsSubset => {
                ProjectedSetAlgebra::Relation(crate::set_methods::relation_between(
                    &left,
                    right,
                    heap,
                    SetRelation::Subset,
                    operation,
                )?)
            }
            HostSetAlgebra::IsSuperset => {
                ProjectedSetAlgebra::Relation(crate::set_methods::relation_between(
                    &left,
                    right,
                    heap,
                    SetRelation::Superset,
                    operation,
                )?)
            }
            HostSetAlgebra::IsDisjoint => {
                ProjectedSetAlgebra::Relation(crate::set_methods::relation_between(
                    &left,
                    right,
                    heap,
                    SetRelation::Disjoint,
                    operation,
                )?)
            }
        }
    };

    match result {
        ProjectedSetAlgebra::Values(values) => {
            let heap = runtime
                .heap
                .as_deref_mut()
                .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            crate::heap_values::make_set_value(values, heap, runtime.budget.as_deref_mut())
        }
        ProjectedSetAlgebra::Relation(value) => Ok(Value::Bool(value)),
    }
}

enum ProjectedSetAlgebra {
    Values(Vec<Value>),
    Relation(bool),
}
