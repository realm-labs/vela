use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use vela_common::{HostMethodId, HostTypeId};

use crate::call_value::HostCallValue;
use crate::error::HostResult;
use crate::protocol::{
    HostCollectionMutation, HostCollectionProjection, HostCollectionQuery, HostCollectionSnapshot,
};
use crate::resolved::{
    HostAccessSpec, HostMutationOp, HostSchemaEpoch, PreparedHostStep, ResolvedHostAccess,
};
use crate::target::{HostPathPart, HostTargetInstance};
use crate::value::HostValue;

use super::{
    ScopedHostCollectionDependents, ScriptHostFieldAccess, ScriptHostKey, ScriptHostObject,
    collection_protocol::{collection_query_result, mutate_btree_map, mutate_hash_map},
    collection_snapshot::snapshot_map_entries,
    errors::missing_collection_entry,
    target::{target_is_leaf, target_key},
};

impl<K, V> ScriptHostFieldAccess for BTreeMap<K, V>
where
    K: ScriptHostKey + Send + Sync + 'static,
    V: ScriptHostFieldAccess + ScriptHostObject + Send + Sync + 'static,
{
    fn script_host_type_id(&self) -> HostTypeId {
        let (Some(key), Some(value)) = (K::script_host_key_shape(), V::script_host_type_shape())
        else {
            return HostTypeId::new(0);
        };
        HostTypeId::new(vela_common::rust_standard_type_id(
            "btree_map",
            &format!("{key}|{value}"),
        ))
    }

    fn script_host_type_shape() -> Option<String> {
        Some(format!(
            "Map<{}, {}>",
            K::script_host_key_shape()?,
            V::script_host_type_shape()?
        ))
    }

    fn resolve_host_type_target_from(
        spec: HostAccessSpec<'_>,
        offset: usize,
    ) -> HostResult<ResolvedHostAccess> {
        resolve_map_target::<V>(spec, offset)
    }

    fn resolve_host_target_from(
        &self,
        spec: HostAccessSpec<'_>,
        offset: usize,
    ) -> HostResult<ResolvedHostAccess> {
        Self::resolve_host_type_target_from(spec, offset)
    }

    fn read_resolved_host_target_from(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
            return self
                .get(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .read_resolved_host_target_from(child_access, target.at_offset(target.offset + 1));
        }
        self.read_host_target_from(target, target.offset)
    }

    fn borrow_resolved_host_shared(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        if target_is_leaf(target, target.offset) {
            let type_id = self.script_host_type_id();
            return Ok(Some(Box::new(
                crate::lease::SharedScopedHost::with_type_id(self, type_id),
            )));
        }
        borrow_map_value_shared(self, target)
    }

    fn borrow_resolved_host_exclusive(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        if target_is_leaf(target, target.offset) {
            let type_id = self.script_host_type_id();
            return Ok(Some(Box::new(
                crate::lease::ExclusiveScopedHost::with_type_id(self, type_id),
            )));
        }
        borrow_map_value_exclusive(self, target)
    }

    fn borrow_collection_resolved_host_shared(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        if !target_is_leaf(target, target.offset) {
            return Ok(None);
        }
        borrow_map_collection_shared(self.iter(), projection)
    }

    fn borrow_collection_resolved_host_exclusive(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        if !target_is_leaf(target, target.offset) {
            return Ok(None);
        }
        borrow_map_collection_exclusive(self.iter_mut(), projection)
    }

    fn write_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
            return self
                .get_mut(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .write_resolved_host_target_from(
                    child_access,
                    target.at_offset(target.offset + 1),
                    value,
                );
        }
        self.write_host_target_from(target, target.offset, value)
    }

    fn mutate_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
            return self
                .get_mut(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .mutate_resolved_host_target_from(
                    child_access,
                    target.at_offset(target.offset + 1),
                    op,
                    rhs,
                );
        }
        self.mutate_host_target_from(target, target.offset, op, rhs)
    }

    fn call_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostCallValue],
    ) -> HostResult<HostCallValue> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
            return self
                .get_mut(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .call_resolved_host(
                    child_access,
                    target.at_offset(target.offset + 1),
                    method,
                    args,
                );
        }
        self.call_host_target_from(target, target.offset, method, args)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .read_host_target_from(target, offset + 1)
    }

    fn query_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        if target_is_leaf(target, offset) {
            return collection_query_result(self.len(), query);
        }
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .query_collection_host_target_from(target, offset + 1, query)
    }

    fn snapshot_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        if !target_is_leaf(target, offset) {
            let key = K::from_host_collection_key(target_key(target, offset)?)?;
            return self
                .get(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .snapshot_collection_host_target_from(target, offset + 1, projection);
        }
        snapshot_map_entries(self.iter(), target, offset, projection)
    }

    fn mutate_collection_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        if target_is_leaf(target, offset) {
            return mutate_btree_map(self, mutation);
        }
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .mutate_collection_host_target_from(target, offset + 1, mutation)
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        if let Some(current) = self.get_mut(&key) {
            return current.write_host_target_from(target, offset + 1, value);
        }
        if offset + 1 != target.plan.parts.len() {
            return Err(missing_collection_entry(target));
        }
        self.insert(key, V::from_host_collection_value(value)?);
        Ok(())
    }

    fn remove_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<()> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        if offset + 1 == target.plan.parts.len() {
            self.remove(&key)
                .map(|_| ())
                .ok_or_else(|| missing_collection_entry(target))
        } else {
            self.get_mut(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .remove_host_target_from(target, offset + 1)
        }
    }

    fn call_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        method: HostMethodId,
        args: &[HostCallValue],
    ) -> HostResult<HostCallValue> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .call_host_target_from(target, offset + 1, method, args)
    }
}

impl<K, V> ScriptHostFieldAccess for HashMap<K, V>
where
    K: ScriptHostKey + Hash + Send + Sync + 'static,
    V: ScriptHostFieldAccess + ScriptHostObject + Send + Sync + 'static,
{
    fn script_host_type_id(&self) -> HostTypeId {
        let (Some(key), Some(value)) = (K::script_host_key_shape(), V::script_host_type_shape())
        else {
            return HostTypeId::new(0);
        };
        HostTypeId::new(vela_common::rust_standard_type_id(
            "hash_map",
            &format!("{key}|{value}"),
        ))
    }

    fn script_host_type_shape() -> Option<String> {
        Some(format!(
            "Map<{}, {}>",
            K::script_host_key_shape()?,
            V::script_host_type_shape()?
        ))
    }

    fn resolve_host_type_target_from(
        spec: HostAccessSpec<'_>,
        offset: usize,
    ) -> HostResult<ResolvedHostAccess> {
        resolve_map_target::<V>(spec, offset)
    }

    fn resolve_host_target_from(
        &self,
        spec: HostAccessSpec<'_>,
        offset: usize,
    ) -> HostResult<ResolvedHostAccess> {
        Self::resolve_host_type_target_from(spec, offset)
    }

    fn read_resolved_host_target_from(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
            return self
                .get(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .read_resolved_host_target_from(child_access, target.at_offset(target.offset + 1));
        }
        self.read_host_target_from(target, target.offset)
    }

    fn borrow_resolved_host_shared(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        if target_is_leaf(target, target.offset) {
            let type_id = self.script_host_type_id();
            return Ok(Some(Box::new(
                crate::lease::SharedScopedHost::with_type_id(self, type_id),
            )));
        }
        borrow_map_value_shared(self, target)
    }

    fn borrow_resolved_host_exclusive(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        if target_is_leaf(target, target.offset) {
            let type_id = self.script_host_type_id();
            return Ok(Some(Box::new(
                crate::lease::ExclusiveScopedHost::with_type_id(self, type_id),
            )));
        }
        borrow_map_value_exclusive(self, target)
    }

    fn borrow_collection_resolved_host_shared(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        if !target_is_leaf(target, target.offset) {
            return Ok(None);
        }
        let mut entries = self.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(key, _)| (*key).clone());
        borrow_map_collection_shared(entries, projection)
    }

    fn borrow_collection_resolved_host_exclusive(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        if !target_is_leaf(target, target.offset) {
            return Ok(None);
        }
        let mut entries = self.iter_mut().collect::<Vec<_>>();
        entries.sort_by_key(|(key, _)| (*key).clone());
        borrow_map_collection_exclusive(entries, projection)
    }

    fn write_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
            return self
                .get_mut(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .write_resolved_host_target_from(
                    child_access,
                    target.at_offset(target.offset + 1),
                    value,
                );
        }
        self.write_host_target_from(target, target.offset, value)
    }

    fn mutate_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
            return self
                .get_mut(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .mutate_resolved_host_target_from(
                    child_access,
                    target.at_offset(target.offset + 1),
                    op,
                    rhs,
                );
        }
        self.mutate_host_target_from(target, target.offset, op, rhs)
    }

    fn call_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostCallValue],
    ) -> HostResult<HostCallValue> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
            return self
                .get_mut(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .call_resolved_host(
                    child_access,
                    target.at_offset(target.offset + 1),
                    method,
                    args,
                );
        }
        self.call_host_target_from(target, target.offset, method, args)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .read_host_target_from(target, offset + 1)
    }

    fn query_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        if target_is_leaf(target, offset) {
            return collection_query_result(self.len(), query);
        }
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .query_collection_host_target_from(target, offset + 1, query)
    }

    fn snapshot_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        if !target_is_leaf(target, offset) {
            let key = K::from_host_collection_key(target_key(target, offset)?)?;
            return self
                .get(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .snapshot_collection_host_target_from(target, offset + 1, projection);
        }
        let mut entries = self.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(key, _)| (*key).clone());
        snapshot_map_entries(entries, target, offset, projection)
    }

    fn mutate_collection_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        if target_is_leaf(target, offset) {
            return mutate_hash_map(self, mutation);
        }
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .mutate_collection_host_target_from(target, offset + 1, mutation)
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        if let Some(current) = self.get_mut(&key) {
            return current.write_host_target_from(target, offset + 1, value);
        }
        if offset + 1 != target.plan.parts.len() {
            return Err(missing_collection_entry(target));
        }
        self.insert(key, V::from_host_collection_value(value)?);
        Ok(())
    }

    fn remove_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<()> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        if offset + 1 == target.plan.parts.len() {
            self.remove(&key)
                .map(|_| ())
                .ok_or_else(|| missing_collection_entry(target))
        } else {
            self.get_mut(&key)
                .ok_or_else(|| missing_collection_entry(target))?
                .remove_host_target_from(target, offset + 1)
        }
    }

    fn call_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        method: HostMethodId,
        args: &[HostCallValue],
    ) -> HostResult<HostCallValue> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .call_host_target_from(target, offset + 1, method, args)
    }
}

fn resolve_map_target<V: ScriptHostFieldAccess + ScriptHostObject>(
    spec: HostAccessSpec<'_>,
    offset: usize,
) -> HostResult<ResolvedHostAccess> {
    Ok(match spec.plan.parts.as_slice().get(offset) {
        None => ResolvedHostAccess::adapter_local(0, HostSchemaEpoch::new(0)),
        Some(HostPathPart::ConstKey(_) | HostPathPart::DynKey { .. }) => {
            if matches!(spec.op, crate::resolved::HostAccessOp::Call(_)) {
                V::resolve_host_type_target(spec.at_offset(offset + 1))?.prepend_prepared_adapter(0)
            } else if offset + 1 == spec.plan.parts.len() {
                ResolvedHostAccess::adapter_local(0, HostSchemaEpoch::new(0))
            } else {
                V::resolve_host_type_target_from(spec, offset + 1)?.prepend_prepared_adapter(0)
            }
        }
        Some(_) => ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)),
    })
}

fn borrow_map_value_shared<'a, K, V>(
    map: &'a impl MapValueRef<K, V>,
    target: HostTargetInstance<'_>,
) -> HostResult<Option<crate::lease::ScopedHostDependent<'a>>>
where
    K: ScriptHostKey,
    V: ScriptHostObject + Send + Sync + 'static,
{
    if target.offset + 1 != target.plan.parts.len() {
        return Ok(None);
    }
    let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
    let value = map
        .map_value_ref(&key)
        .ok_or_else(|| missing_collection_entry(target))?;
    Ok(Some(Box::new(
        crate::lease::SharedScopedHost::with_type_id(value, value.host_type_id()),
    )))
}

fn borrow_map_value_exclusive<'a, K, V, M>(
    map: &'a mut M,
    target: HostTargetInstance<'_>,
) -> HostResult<Option<crate::lease::ScopedHostDependent<'a>>>
where
    K: ScriptHostKey,
    V: ScriptHostObject + Send + Sync + 'static,
    M: MapValueMut<K, V>,
{
    if target.offset + 1 != target.plan.parts.len() {
        return Ok(None);
    }
    let key = K::from_host_collection_key(target_key(target, target.offset)?)?;
    let value = map
        .map_value_mut(&key)
        .ok_or_else(|| missing_collection_entry(target))?;
    let type_id = value.host_type_id();
    Ok(Some(Box::new(
        crate::lease::ExclusiveScopedHost::with_type_id(value, type_id),
    )))
}

trait MapValueMut<K, V> {
    fn map_value_mut(&mut self, key: &K) -> Option<&mut V>;
}

trait MapValueRef<K, V> {
    fn map_value_ref(&self, key: &K) -> Option<&V>;
}

impl<K: Ord, V> MapValueRef<K, V> for BTreeMap<K, V> {
    fn map_value_ref(&self, key: &K) -> Option<&V> {
        self.get(key)
    }
}

impl<K: Eq + Hash, V> MapValueRef<K, V> for HashMap<K, V> {
    fn map_value_ref(&self, key: &K) -> Option<&V> {
        self.get(key)
    }
}

impl<K: Ord, V> MapValueMut<K, V> for BTreeMap<K, V> {
    fn map_value_mut(&mut self, key: &K) -> Option<&mut V> {
        self.get_mut(key)
    }
}

impl<K: Eq + Hash, V> MapValueMut<K, V> for HashMap<K, V> {
    fn map_value_mut(&mut self, key: &K) -> Option<&mut V> {
        self.get_mut(key)
    }
}

fn borrow_map_collection_shared<'a, K, V>(
    entries: impl IntoIterator<Item = (&'a K, &'a V)>,
    projection: HostCollectionProjection,
) -> HostResult<Option<ScopedHostCollectionDependents<'a>>>
where
    K: ScriptHostKey + 'a,
    V: ScriptHostObject + Send + Sync + 'static,
{
    Ok(match projection {
        HostCollectionProjection::Keys => None,
        HostCollectionProjection::Values => Some(ScopedHostCollectionDependents::Items(
            entries
                .into_iter()
                .map(|(_, value)| {
                    Box::new(crate::lease::SharedScopedHost::with_type_id(
                        value,
                        value.host_type_id(),
                    )) as crate::lease::ScopedHostDependent<'a>
                })
                .collect(),
        )),
        HostCollectionProjection::Entries => Some(ScopedHostCollectionDependents::Entries(
            entries
                .into_iter()
                .map(|(key, value)| {
                    (
                        key.to_host_collection_key().into_host_value(),
                        Box::new(crate::lease::SharedScopedHost::with_type_id(
                            value,
                            value.host_type_id(),
                        )) as crate::lease::ScopedHostDependent<'a>,
                    )
                })
                .collect(),
        )),
    })
}

fn borrow_map_collection_exclusive<'a, K, V>(
    entries: impl IntoIterator<Item = (&'a K, &'a mut V)>,
    projection: HostCollectionProjection,
) -> HostResult<Option<ScopedHostCollectionDependents<'a>>>
where
    K: ScriptHostKey + 'a,
    V: ScriptHostObject + Send + Sync + 'static,
{
    Ok(match projection {
        HostCollectionProjection::Keys => None,
        HostCollectionProjection::Values => Some(ScopedHostCollectionDependents::Items(
            entries
                .into_iter()
                .map(|(_, value)| {
                    let type_id = value.host_type_id();
                    Box::new(crate::lease::ExclusiveScopedHost::with_type_id(
                        value, type_id,
                    )) as crate::lease::ScopedHostDependent<'a>
                })
                .collect(),
        )),
        HostCollectionProjection::Entries => Some(ScopedHostCollectionDependents::Entries(
            entries
                .into_iter()
                .map(|(key, value)| {
                    let type_id = value.host_type_id();
                    (
                        key.to_host_collection_key().into_host_value(),
                        Box::new(crate::lease::ExclusiveScopedHost::with_type_id(
                            value, type_id,
                        )) as crate::lease::ScopedHostDependent<'a>,
                    )
                })
                .collect(),
        )),
    })
}
