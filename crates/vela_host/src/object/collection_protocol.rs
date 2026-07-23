use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use vela_common::ScalarValue;

use crate::error::{HostError, HostErrorKind, HostResult};
use crate::protocol::{HostCollectionKey, HostCollectionMutation, HostCollectionQuery};
use crate::value::HostValue;

use super::{ScriptHostFieldAccess, ScriptHostKey};

pub(super) fn collection_query_result(
    len: usize,
    query: HostCollectionQuery,
) -> HostResult<HostValue> {
    match query {
        HostCollectionQuery::Len => i64::try_from(len)
            .map(|len| HostValue::Scalar(ScalarValue::I64(len)))
            .map_err(|_| invalid_arg("collection length within i64 range")),
        HostCollectionQuery::IsEmpty => Ok(HostValue::Bool(len == 0)),
    }
}

pub(super) fn unsupported_collection_query(query: HostCollectionQuery) -> HostError {
    HostError {
        kind: HostErrorKind::UnsupportedCollectionQuery { query },
        source_span: None,
    }
}

pub(super) fn unsupported_collection_mutation(mutation: HostCollectionMutation<'_>) -> HostError {
    HostError {
        kind: HostErrorKind::UnsupportedCollectionMutation {
            mutation: mutation.kind(),
        },
        source_span: None,
    }
}

pub(super) fn mutate_btree_map<K, V>(
    values: &mut BTreeMap<K, V>,
    mutation: HostCollectionMutation<'_>,
) -> HostResult<()>
where
    K: ScriptHostKey,
    V: ScriptHostFieldAccess,
{
    match mutation {
        HostCollectionMutation::Clear => values.clear(),
        HostCollectionMutation::ExtendMap(entries) => {
            let prepared = entries
                .iter()
                .map(|(key, value)| {
                    Ok((
                        K::from_host_collection_key(key.as_ref())?,
                        V::from_host_collection_value(value.clone())?,
                    ))
                })
                .collect::<HostResult<Vec<_>>>()?;
            values.extend(prepared);
        }
        HostCollectionMutation::RetainKeys { expected, keep } => {
            let keep = validated_retain_keys(values.keys(), expected, keep)?;
            values.retain(|key, _| keep.contains(key));
        }
        mutation => return Err(unsupported_collection_mutation(mutation)),
    }
    Ok(())
}

pub(super) fn mutate_hash_map<K, V>(
    values: &mut HashMap<K, V>,
    mutation: HostCollectionMutation<'_>,
) -> HostResult<()>
where
    K: ScriptHostKey + Hash,
    V: ScriptHostFieldAccess,
{
    match mutation {
        HostCollectionMutation::Clear => values.clear(),
        HostCollectionMutation::ExtendMap(entries) => {
            let prepared = entries
                .iter()
                .map(|(key, value)| {
                    Ok((
                        K::from_host_collection_key(key.as_ref())?,
                        V::from_host_collection_value(value.clone())?,
                    ))
                })
                .collect::<HostResult<Vec<_>>>()?;
            values.extend(prepared);
        }
        HostCollectionMutation::RetainKeys { expected, keep } => {
            let keep = validated_retain_keys(values.keys(), expected, keep)?;
            values.retain(|key, _| keep.contains(key));
        }
        mutation => return Err(unsupported_collection_mutation(mutation)),
    }
    Ok(())
}

pub(super) fn mutate_vec<T>(
    values: &mut Vec<T>,
    mutation: HostCollectionMutation<'_>,
) -> HostResult<()>
where
    T: ScriptHostFieldAccess,
{
    match mutation {
        HostCollectionMutation::Clear => values.clear(),
        HostCollectionMutation::ExtendSequence(extension) => {
            let prepared = extension
                .iter()
                .cloned()
                .map(T::from_host_collection_value)
                .collect::<HostResult<Vec<_>>>()?;
            values.extend(prepared);
        }
        HostCollectionMutation::InsertSequence { index, value } => {
            if index > values.len() {
                return Err(invalid_arg("array insertion index"));
            }
            let value = T::from_host_collection_value(value.clone())?;
            values.insert(index, value);
        }
        HostCollectionMutation::RetainSequence { expected_len, keep } => {
            if values.len() != expected_len {
                return Err(invalid_arg("unchanged sequence snapshot"));
            }
            if keep.len() != expected_len {
                return Err(invalid_arg("retain decision for every sequence element"));
            }
            let mut index = 0;
            values.retain(|_| {
                let retain = keep[index];
                index += 1;
                retain
            });
        }
        mutation => return Err(unsupported_collection_mutation(mutation)),
    }
    Ok(())
}

pub(super) fn mutate_btree_set<K>(
    values: &mut BTreeSet<K>,
    mutation: HostCollectionMutation<'_>,
) -> HostResult<()>
where
    K: ScriptHostKey,
{
    match mutation {
        HostCollectionMutation::Clear => values.clear(),
        HostCollectionMutation::ExtendSet(extension) => {
            let prepared = extension
                .iter()
                .map(|key| K::from_host_collection_key(key.as_ref()))
                .collect::<HostResult<Vec<_>>>()?;
            values.extend(prepared);
        }
        HostCollectionMutation::RetainKeys { expected, keep } => {
            let keep = validated_retain_keys(values.iter(), expected, keep)?;
            values.retain(|key| keep.contains(key));
        }
        mutation => return Err(unsupported_collection_mutation(mutation)),
    }
    Ok(())
}

pub(super) fn mutate_hash_set<K>(
    values: &mut HashSet<K>,
    mutation: HostCollectionMutation<'_>,
) -> HostResult<()>
where
    K: ScriptHostKey + Hash,
{
    match mutation {
        HostCollectionMutation::Clear => values.clear(),
        HostCollectionMutation::ExtendSet(extension) => {
            let prepared = extension
                .iter()
                .map(|key| K::from_host_collection_key(key.as_ref()))
                .collect::<HostResult<Vec<_>>>()?;
            values.extend(prepared);
        }
        HostCollectionMutation::RetainKeys { expected, keep } => {
            let keep = validated_retain_keys(values.iter(), expected, keep)?;
            values.retain(|key| keep.contains(key));
        }
        mutation => return Err(unsupported_collection_mutation(mutation)),
    }
    Ok(())
}

fn validated_retain_keys<'a, K>(
    current: impl Iterator<Item = &'a K>,
    expected: &[HostCollectionKey],
    keep: &[HostCollectionKey],
) -> HostResult<BTreeSet<K>>
where
    K: ScriptHostKey + 'a,
{
    let expected_keys = convert_distinct_keys(expected)?;
    let keep_keys = convert_distinct_keys(keep)?;
    let current_keys = current.cloned().collect::<BTreeSet<_>>();
    if current_keys != expected_keys {
        return Err(invalid_arg("unchanged keyed collection snapshot"));
    }
    if !keep_keys.is_subset(&expected_keys) {
        return Err(invalid_arg("retained keyed collection subset"));
    }
    Ok(keep_keys)
}

fn convert_distinct_keys<K>(keys: &[HostCollectionKey]) -> HostResult<BTreeSet<K>>
where
    K: ScriptHostKey,
{
    let converted = keys
        .iter()
        .map(|key| K::from_host_collection_key(key.as_ref()))
        .collect::<HostResult<BTreeSet<_>>>()?;
    if converted.len() != keys.len() {
        return Err(invalid_arg("distinct keyed collection snapshot"));
    }
    Ok(converted)
}

fn invalid_arg(expected: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument { expected },
        source_span: None,
    }
}
