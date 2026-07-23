use std::{
    collections::{BTreeSet, HashSet},
    hash::Hash,
};

use vela_common::HostTypeId;

use crate::{
    error::HostResult,
    protocol::{
        HostCollectionMutation, HostCollectionProjection, HostCollectionQuery,
        HostCollectionSnapshot,
    },
    target::HostTargetInstance,
    value::HostValue,
};

use super::{
    HostValueFrom, ScriptHostFieldAccess, ScriptHostKey,
    collection_protocol::{collection_query_result, mutate_btree_set, mutate_hash_set},
    collection_snapshot::snapshot_set_values,
    errors::missing_target,
    target::{target_is_leaf, target_key},
};

impl<K> ScriptHostFieldAccess for BTreeSet<K>
where
    K: ScriptHostKey,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        if offset + 1 == target.plan.parts.len() {
            Ok(HostValue::Bool(self.contains(&key)))
        } else {
            Err(missing_target(target))
        }
    }

    fn query_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        if target_is_leaf(target, offset) {
            collection_query_result(self.len(), query)
        } else {
            Err(missing_target(target))
        }
    }

    fn snapshot_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        if !target_is_leaf(target, offset) {
            return Err(missing_target(target));
        }
        snapshot_set_values(self.iter(), projection)
    }

    fn mutate_collection_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        if !target_is_leaf(target, offset) {
            return Err(missing_target(target));
        }
        mutate_btree_set(self, mutation)
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        if offset + 1 != target.plan.parts.len() {
            return Err(missing_target(target));
        }
        if bool::from_host_value(&value)? {
            self.insert(key);
        } else {
            self.remove(&key);
        }
        Ok(())
    }
}

impl<K> ScriptHostFieldAccess for HashSet<K>
where
    K: ScriptHostKey + Hash,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        if offset + 1 == target.plan.parts.len() {
            Ok(HostValue::Bool(self.contains(&key)))
        } else {
            Err(missing_target(target))
        }
    }

    fn query_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        if target_is_leaf(target, offset) {
            collection_query_result(self.len(), query)
        } else {
            Err(missing_target(target))
        }
    }

    fn snapshot_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        if !target_is_leaf(target, offset) {
            return Err(missing_target(target));
        }
        let mut values = self.iter().collect::<Vec<_>>();
        values.sort();
        snapshot_set_values(values, projection)
    }

    fn mutate_collection_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        if !target_is_leaf(target, offset) {
            return Err(missing_target(target));
        }
        mutate_hash_set(self, mutation)
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        if offset + 1 != target.plan.parts.len() {
            return Err(missing_target(target));
        }
        if bool::from_host_value(&value)? {
            self.insert(key);
        } else {
            self.remove(&key);
        }
        Ok(())
    }
}
