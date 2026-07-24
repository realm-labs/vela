use crate::iteration;
use crate::method_runtime::{MethodRuntime, call_callback, callback_param_len};
use crate::option_result::option_value;
use crate::runtime_checks::is_truthy;
use crate::value_key::ValueKey;
use crate::{Value, VmError, VmErrorKind, VmResult};

use super::{expect_arity, make_map_from_entries, map_entries, map_entry};

pub(crate) fn map_values(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map_values", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method map_values")?;
    let param_len =
        callback_param_len_for_entries(&runtime, "method map_values", &args[0], &entries)?;
    let values = iteration::collect_values_over(
        entries.iter(),
        &mut runtime,
        "method map_values",
        |runtime, (key, value), protected_values| {
            call_map_callback(
                runtime,
                "method map_values",
                &args[0],
                param_len,
                key,
                *value,
                protected_values,
            )
        },
    )?;
    let mapped = entries
        .into_iter()
        .zip(values)
        .map(|((key, _), value)| (key, value))
        .collect();
    make_map_from_entries(
        mapped,
        &mut runtime.heap,
        &mut runtime.budget,
        "method map_values",
    )
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method filter")?;
    let param_len = callback_param_len_for_entries(&runtime, "method filter", &args[0], &entries)?;
    let filtered = iteration::filter_items_over(
        entries,
        &mut runtime,
        "method filter",
        |runtime, (key, value), protected_values| {
            call_map_callback(
                runtime,
                "method filter",
                &args[0],
                param_len,
                key,
                *value,
                protected_values,
            )
        },
        |(_, value)| *value,
    )?
    .into_iter()
    .collect();
    make_map_from_entries(
        filtered,
        &mut runtime.heap,
        &mut runtime.budget,
        "method filter",
    )
}

pub(crate) fn group_by(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("group_by", args, 1)?;
    let operation = "method group_by";
    let entries = map_entries(receiver, runtime.heap.as_deref(), operation)?;
    let param_len = callback_param_len_for_entries(&runtime, operation, &args[0], &entries)?;
    let mut groups = std::collections::BTreeMap::<ValueKey, GroupEntries>::new();
    for (key, value) in entries {
        if let Some(budget) = runtime.budget.as_deref_mut() {
            budget.charge_execution_units(1)?;
        }
        let grouped = call_map_callback(
            &mut runtime,
            operation,
            &args[0],
            param_len,
            &key,
            value,
            &protected_group_entries(&groups),
        )?;
        let identity = ValueKey::from_value(&grouped, runtime.heap.as_deref(), operation)?;
        match groups.entry(identity) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(GroupEntries {
                    key: grouped,
                    entries: vec![(key, value)],
                });
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                entry.get_mut().entries.push((key, value));
            }
        }
    }

    let mut grouped = Vec::with_capacity(groups.len());
    for group in groups.into_values() {
        let values = make_map_from_entries(
            group.entries,
            &mut runtime.heap,
            &mut runtime.budget,
            operation,
        )?;
        grouped.push((group.key, values));
    }
    make_map_from_entries(grouped, &mut runtime.heap, &mut runtime.budget, operation)
}

pub(crate) fn retain(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("retain", args, 1)?;
    let operation = "method retain";
    let entries = map_entries(receiver, runtime.heap.as_deref(), operation)?;
    let param_len = callback_param_len_for_entries(&runtime, operation, &args[0], &entries)?;
    let protected = entries
        .iter()
        .flat_map(|(key, value)| [*key, *value])
        .collect::<Vec<_>>();
    let decisions = entries
        .iter()
        .map(|(key, value)| {
            call_map_callback(
                &mut runtime,
                operation,
                &args[0],
                param_len,
                key,
                *value,
                &protected,
            )
            .map(|returned| is_truthy(&returned))
        })
        .collect::<VmResult<Vec<_>>>()?;
    let expected = entries
        .iter()
        .map(|(key, _)| ValueKey::from_value(key, runtime.heap.as_deref(), operation))
        .collect::<VmResult<std::collections::BTreeSet<_>>>()?;
    let keep = entries
        .iter()
        .zip(decisions)
        .filter(|(_, keep)| *keep)
        .map(|((key, _), _)| ValueKey::from_value(key, runtime.heap.as_deref(), operation))
        .collect::<VmResult<std::collections::BTreeSet<_>>>()?;
    let Value::HeapRef(reference) = receiver else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let Some(heap) = runtime.heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    crate::collection_mutation::retain_map_slots(
        heap,
        *reference,
        &expected,
        &keep,
        runtime.budget.as_deref_mut(),
        operation,
    )?;
    Ok(Value::Unit)
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method find")?;
    let param_len = callback_param_len_for_entries(&runtime, "method find", &args[0], &entries)?;
    let found = iteration::callback_find_over(
        entries,
        &mut runtime,
        "method find",
        |runtime, (key, value)| {
            call_map_callback(
                runtime,
                "method find",
                &args[0],
                param_len,
                key,
                *value,
                &[],
            )
        },
    )?;
    if let Some((key, value)) = found {
        let entry = map_entry(key, value, &mut runtime.heap, &mut runtime.budget)?;
        let Some(heap) = runtime.heap.as_deref_mut() else {
            return super::type_error("method find");
        };
        return option_value(Some(entry), heap, runtime.budget.as_deref_mut());
    }
    let Some(heap) = runtime.heap.as_deref_mut() else {
        return super::type_error("method find");
    };
    option_value(None, heap, runtime.budget.as_deref_mut())
}

pub(crate) fn any(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("any", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method any")?;
    let param_len = callback_param_len_for_entries(&runtime, "method any", &args[0], &entries)?;
    iteration::callback_any_over(
        entries,
        &mut runtime,
        "method any",
        |runtime, (key, value)| {
            call_map_callback(runtime, "method any", &args[0], param_len, key, *value, &[])
        },
    )
}

pub(crate) fn all(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("all", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method all")?;
    let param_len = callback_param_len_for_entries(&runtime, "method all", &args[0], &entries)?;
    iteration::callback_all_over(
        entries,
        &mut runtime,
        "method all",
        |runtime, (key, value)| {
            call_map_callback(runtime, "method all", &args[0], param_len, key, *value, &[])
        },
    )
}

pub(crate) fn count(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<i64> {
    expect_arity("count", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method count")?;
    let param_len = callback_param_len_for_entries(&runtime, "method count", &args[0], &entries)?;
    iteration::callback_count_over(
        entries,
        &mut runtime,
        "method count",
        |runtime, (key, value)| {
            call_map_callback(
                runtime,
                "method count",
                &args[0],
                param_len,
                key,
                *value,
                &[],
            )
        },
    )
}

fn callback_param_len_for_entries(
    runtime: &MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    entries: &[(Value, Value)],
) -> VmResult<usize> {
    if entries.is_empty() {
        Ok(0)
    } else {
        callback_param_len(runtime, operation, callback)
    }
}

fn call_map_callback(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    param_len: usize,
    key: &Value,
    value: Value,
    protected_values: &[Value],
) -> VmResult<Value> {
    match param_len {
        0 => call_callback(runtime, operation, callback, &[], protected_values),
        1 => call_callback(
            runtime,
            operation,
            callback,
            std::slice::from_ref(&value),
            protected_values,
        ),
        _ => {
            let callback_args = [*key, value];
            call_callback(
                runtime,
                operation,
                callback,
                &callback_args,
                protected_values,
            )
        }
    }
}

struct GroupEntries {
    key: Value,
    entries: Vec<(Value, Value)>,
}

fn protected_group_entries(
    groups: &std::collections::BTreeMap<ValueKey, GroupEntries>,
) -> Vec<Value> {
    groups
        .values()
        .flat_map(|group| {
            std::iter::once(group.key)
                .chain(group.entries.iter().flat_map(|(key, value)| [*key, *value]))
        })
        .collect()
}
