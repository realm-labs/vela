use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use vela_common::ScalarValue;

use crate::error::{HostError, HostErrorKind, HostResult};
use crate::protocol::{HostCollectionMutation, HostCollectionQuery};
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
        mutation => return Err(unsupported_collection_mutation(mutation)),
    }
    Ok(())
}

fn invalid_arg(expected: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument { expected },
        source_span: None,
    }
}
