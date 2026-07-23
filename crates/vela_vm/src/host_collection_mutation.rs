use vela_bytecode::{CacheSiteId, Register};
use vela_common::ScalarValue;
use vela_host::protocol::{
    HostCollectionKey, HostCollectionMutation, HostCollectionProjection, HostCollectionQuery,
    HostCollectionSnapshot,
};
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::target::{HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;

use crate::host_access::{
    HostAccessRuntime, missing_host_context, resolve_cached_access, runtime_collection_key,
};
use crate::host_values::value_to_host;
use crate::resumable_callbacks::HostCollectionRetain;
use crate::{HostInlineCacheTarget, Value, VmError, VmErrorKind, VmResult, expect_host_ref};

pub(crate) fn execute_host_root_collection_clear(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        runtime.host.as_deref(),
        "host collection clear",
    )?;
    let target = HostTargetPlan::new(root.type_id);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;
    let read = resolve_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        instance,
        HostAccessOp::Read,
        runtime.source_span,
    )?;
    let len = host.access.query_collection_resolved(
        host.adapter,
        read,
        instance,
        HostCollectionQuery::Len,
        runtime.source_span,
    )?;
    let HostValue::Scalar(ScalarValue::I64(len)) = len else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host collection clear length",
        }));
    };
    let units = u64::try_from(len).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host collection clear length",
        })
    })?;
    if let Some(budget) = runtime.budget.as_deref_mut() {
        budget.charge_execution_units(units)?;
    }
    let write = resolve_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        instance,
        HostAccessOp::Write,
        runtime.source_span,
    )?;
    host.access.mutate_collection_resolved(
        host.adapter,
        write,
        instance,
        HostCollectionMutation::Clear,
        runtime.source_span,
    )?;
    Ok(Value::Unit)
}

enum PreparedCollectionExtension {
    SequenceItem(HostValue),
    Sequence(Vec<HostValue>),
    Map(Vec<(HostCollectionKey, HostValue)>),
    Set(Vec<HostCollectionKey>),
}

impl PreparedCollectionExtension {
    fn len(&self) -> usize {
        match self {
            Self::SequenceItem(_) => 1,
            Self::Sequence(values) => values.len(),
            Self::Map(entries) => entries.len(),
            Self::Set(values) => values.len(),
        }
    }

    fn mutation(&self) -> HostCollectionMutation<'_> {
        match self {
            Self::SequenceItem(value) => {
                HostCollectionMutation::ExtendSequence(std::slice::from_ref(value))
            }
            Self::Sequence(values) => HostCollectionMutation::ExtendSequence(values),
            Self::Map(entries) => HostCollectionMutation::ExtendMap(entries),
            Self::Set(values) => HostCollectionMutation::ExtendSet(values),
        }
    }
}

fn prepare_sequence_extension(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    extension: &Value,
    operation: &'static str,
) -> VmResult<PreparedCollectionExtension> {
    if matches!(extension, Value::HostRef(_)) {
        let snapshot = crate::host_collection_projection::snapshot_host_collection_value(
            runtime,
            extension,
            HostCollectionProjection::Values,
        )?;
        let HostCollectionSnapshot::Items(values) = snapshot else {
            return extension_type_error(operation);
        };
        return Ok(PreparedCollectionExtension::Sequence(values));
    }
    Ok(PreparedCollectionExtension::Sequence(
        crate::array_methods::array_values(extension, runtime.heap.as_deref(), operation)?
            .iter()
            .map(|value| {
                value_to_host(
                    value,
                    operation,
                    runtime.heap.as_deref(),
                    runtime.host.as_deref(),
                )
            })
            .collect::<VmResult<Vec<_>>>()?,
    ))
}

fn prepare_map_extension(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    extension: &Value,
    operation: &'static str,
) -> VmResult<PreparedCollectionExtension> {
    if matches!(extension, Value::HostRef(_)) {
        let snapshot = crate::host_collection_projection::snapshot_host_collection_value(
            runtime,
            extension,
            HostCollectionProjection::Entries,
        )?;
        let HostCollectionSnapshot::Entries(entries) = snapshot else {
            return extension_type_error(operation);
        };
        return entries
            .into_iter()
            .map(|(key, value)| Ok((host_value_to_collection_key(key, operation)?, value)))
            .collect::<VmResult<Vec<_>>>()
            .map(PreparedCollectionExtension::Map);
    }
    crate::map_methods::map_entries(extension, runtime.heap.as_deref(), operation)?
        .iter()
        .map(|(key, value)| {
            Ok((
                runtime_collection_key(
                    key,
                    runtime.heap.as_deref(),
                    runtime.host.as_deref(),
                    operation,
                )?,
                value_to_host(
                    value,
                    operation,
                    runtime.heap.as_deref(),
                    runtime.host.as_deref(),
                )?,
            ))
        })
        .collect::<VmResult<Vec<_>>>()
        .map(PreparedCollectionExtension::Map)
}

fn prepare_set_extension(
    runtime: &mut HostAccessRuntime<'_, '_, '_>,
    extension: &Value,
    operation: &'static str,
) -> VmResult<PreparedCollectionExtension> {
    if matches!(extension, Value::HostRef(_)) {
        let snapshot = crate::host_collection_projection::snapshot_host_collection_value(
            runtime,
            extension,
            HostCollectionProjection::Values,
        )?;
        let HostCollectionSnapshot::Items(values) = snapshot else {
            return extension_type_error(operation);
        };
        return values
            .into_iter()
            .map(|value| host_value_to_collection_key(value, operation))
            .collect::<VmResult<Vec<_>>>()
            .map(PreparedCollectionExtension::Set);
    }
    crate::set_methods::set_values(extension, runtime.heap.as_deref(), operation)?
        .iter()
        .map(|value| {
            runtime_collection_key(
                value,
                runtime.heap.as_deref(),
                runtime.host.as_deref(),
                operation,
            )
        })
        .collect::<VmResult<Vec<_>>>()
        .map(PreparedCollectionExtension::Set)
}

fn host_value_to_collection_key(
    value: HostValue,
    operation: &'static str,
) -> VmResult<HostCollectionKey> {
    Ok(match value {
        HostValue::Bool(value) => HostCollectionKey::Bool(value),
        HostValue::Char(value) => HostCollectionKey::Char(value),
        HostValue::Scalar(ScalarValue::I8(value)) => HostCollectionKey::I8(value),
        HostValue::Scalar(ScalarValue::I16(value)) => HostCollectionKey::I16(value),
        HostValue::Scalar(ScalarValue::I32(value)) => HostCollectionKey::I32(value),
        HostValue::Scalar(ScalarValue::I64(value)) => HostCollectionKey::I64(value),
        HostValue::Scalar(ScalarValue::U8(value)) => HostCollectionKey::U8(value),
        HostValue::Scalar(ScalarValue::U16(value)) => HostCollectionKey::U16(value),
        HostValue::Scalar(ScalarValue::U32(value)) => HostCollectionKey::U32(value),
        HostValue::Scalar(ScalarValue::U64(value)) => HostCollectionKey::U64(value),
        HostValue::String(value) => HostCollectionKey::String(value),
        HostValue::Bytes(value) => HostCollectionKey::Bytes(value),
        HostValue::HostRef(value) => HostCollectionKey::HostRef(value),
        HostValue::Unit | HostValue::Scalar(ScalarValue::F32(_) | ScalarValue::F64(_)) => {
            return extension_type_error(operation);
        }
    })
}

fn extension_type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

pub(crate) fn execute_host_root_collection_batch(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    mutation: crate::std_method_ids::HostCollectionMutation,
    extension: &Value,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    use crate::std_method_ids::HostCollectionMutation as VmMutation;

    let operation = match mutation {
        VmMutation::ArrayPush => "host array push",
        _ => "host collection extend",
    };
    let extension = match mutation {
        VmMutation::ArrayExtend => prepare_sequence_extension(&mut runtime, extension, operation)?,
        VmMutation::ArrayPush => PreparedCollectionExtension::SequenceItem(value_to_host(
            extension,
            operation,
            runtime.heap.as_deref(),
            runtime.host.as_deref(),
        )?),
        VmMutation::MapExtend => prepare_map_extension(&mut runtime, extension, operation)?,
        VmMutation::SetExtend => prepare_set_extension(&mut runtime, extension, operation)?,
        _ => unreachable!("only prepared batch mutations reach collection extension"),
    };
    if let Some(budget) = runtime.budget.as_deref_mut() {
        budget.charge_execution_units(u64::try_from(extension.len()).unwrap_or(u64::MAX))?;
    }
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        runtime.host.as_deref(),
        operation,
    )?;
    let target = HostTargetPlan::new(root.type_id);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;
    let write = resolve_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        instance,
        HostAccessOp::Write,
        runtime.source_span,
    )?;
    host.access.mutate_collection_resolved(
        host.adapter,
        write,
        instance,
        extension.mutation(),
        runtime.source_span,
    )?;
    Ok(Value::Unit)
}

pub(crate) fn execute_host_root_array_insert(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    receiver: Register,
    index: &Value,
    value: &Value,
    cache_site: Option<CacheSiteId>,
) -> VmResult<Value> {
    let operation = "host array insert";
    let index = crate::array_methods::index_value(index, operation)?;
    let root = expect_host_ref(
        &runtime.frame.read(receiver)?,
        runtime.host.as_deref(),
        operation,
    )?;
    let len = crate::host_collection_edges::host_array_len(&mut runtime, root, operation)?;
    if index > len {
        return Err(crate::array_methods::index_out_of_bounds(index, len));
    }
    let value = value_to_host(
        value,
        operation,
        runtime.heap.as_deref(),
        runtime.host.as_deref(),
    )?;
    if let Some(budget) = runtime.budget.as_deref_mut() {
        budget.charge_execution_units(1)?;
    }
    let target = HostTargetPlan::new(root.type_id);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;
    let write = resolve_access(
        host.adapter,
        runtime.inline_caches,
        cache_site,
        instance,
        HostAccessOp::Write,
        runtime.source_span,
    )?;
    host.access.mutate_collection_resolved(
        host.adapter,
        write,
        instance,
        HostCollectionMutation::InsertSequence {
            index,
            value: &value,
        },
        runtime.source_span,
    )?;
    Ok(Value::Unit)
}

pub(crate) fn execute_host_root_collection_retain(
    mut runtime: HostAccessRuntime<'_, '_, '_>,
    retain: HostCollectionRetain,
) -> VmResult<Value> {
    let (writeback, prepared, len) = match retain {
        HostCollectionRetain::Sequence {
            writeback,
            expected_len,
            keep,
        } => (
            writeback,
            PreparedCollectionRetain::Sequence { expected_len, keep },
            expected_len,
        ),
        HostCollectionRetain::Keys {
            writeback,
            expected,
            keep,
        } => {
            let expected = expected
                .iter()
                .map(|value| {
                    runtime_collection_key(
                        value,
                        runtime.heap.as_deref(),
                        runtime.host.as_deref(),
                        "host collection retain",
                    )
                })
                .collect::<VmResult<Vec<_>>>()?;
            let keep = keep
                .iter()
                .map(|value| {
                    runtime_collection_key(
                        value,
                        runtime.heap.as_deref(),
                        runtime.host.as_deref(),
                        "host collection retain",
                    )
                })
                .collect::<VmResult<Vec<_>>>()?;
            let len = expected.len();
            (
                writeback,
                PreparedCollectionRetain::Keys { expected, keep },
                len,
            )
        }
    };
    if let Some(budget) = runtime.budget.as_deref_mut() {
        budget.charge_execution_units(u64::try_from(len).unwrap_or(u64::MAX))?;
    }
    let root = expect_host_ref(
        &runtime.frame.read(writeback.receiver)?,
        runtime.host.as_deref(),
        "host collection retain",
    )?;
    let target = HostTargetPlan::new(root.type_id);
    let instance = HostTargetInstance::new(root, &target, &[]);
    let host = runtime
        .host
        .as_deref_mut()
        .ok_or_else(missing_host_context)?;
    let write = resolve_access(
        host.adapter,
        runtime.inline_caches,
        writeback.cache_site,
        instance,
        HostAccessOp::Write,
        runtime.source_span,
    )?;
    host.access.mutate_collection_resolved(
        host.adapter,
        write,
        instance,
        prepared.mutation(),
        runtime.source_span,
    )?;
    Ok(Value::Unit)
}

enum PreparedCollectionRetain {
    Sequence {
        expected_len: usize,
        keep: Vec<bool>,
    },
    Keys {
        expected: Vec<HostCollectionKey>,
        keep: Vec<HostCollectionKey>,
    },
}

impl PreparedCollectionRetain {
    fn mutation(&self) -> HostCollectionMutation<'_> {
        match self {
            Self::Sequence { expected_len, keep } => HostCollectionMutation::RetainSequence {
                expected_len: *expected_len,
                keep,
            },
            Self::Keys { expected, keep } => HostCollectionMutation::RetainKeys { expected, keep },
        }
    }
}

fn resolve_access(
    adapter: &dyn vela_host::adapter::ScriptStateAdapter,
    inline_caches: Option<&dyn crate::VmInlineCaches>,
    cache_site: Option<CacheSiteId>,
    target: HostTargetInstance<'_>,
    op: HostAccessOp,
    source_span: Option<vela_common::Span>,
) -> VmResult<vela_host::resolved::ResolvedHostAccess> {
    if let Some(cache_site) = cache_site {
        resolve_cached_access(
            adapter,
            inline_caches,
            cache_site,
            HostInlineCacheTarget::RootObject,
            target,
            op,
            source_span,
        )
    } else {
        adapter
            .resolve_host_access(HostAccessSpec::new(op, target.plan))
            .map_err(|error| error.with_source_span_if_absent(source_span).into())
    }
}
