use std::any::{Any, TypeId};

use vela_common::{HostMethodId, HostTypeId};

use crate::{
    error::HostResult,
    protocol::{
        HostCollectionMutation, HostCollectionProjection, HostCollectionQuery,
        HostCollectionSnapshot,
    },
    resolved::{HostAccessSpec, HostSchemaEpoch, PreparedHostStep, ResolvedHostAccess},
    target::{HostPathPart, HostTargetInstance},
    value::HostValue,
};

use super::{
    HostValueFrom, ScriptHostFieldAccess, ScriptHostObject,
    collection_protocol::{collection_query_result, mutate_vec},
    errors::{invalid_arg, missing_target},
    target::{target_index, target_is_leaf},
};

impl<T> ScriptHostFieldAccess for Vec<T>
where
    T: ScriptHostFieldAccess + ScriptHostObject + 'static,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn from_host_collection_value(value: HostValue) -> HostResult<Self> {
        if TypeId::of::<T>() != TypeId::of::<u8>() {
            return Err(invalid_arg("host collection value"));
        }
        let bytes = Vec::<u8>::from_host_value(&value)?;
        let value: Box<dyn Any> = Box::new(bytes);
        Ok(*value
            .downcast::<Self>()
            .expect("Vec<T> TypeId matched Vec<u8>"))
    }

    fn resolve_host_type_target_from(
        spec: HostAccessSpec<'_>,
        offset: usize,
    ) -> HostResult<ResolvedHostAccess> {
        Ok(match spec.plan.parts.as_slice().get(offset) {
            None => ResolvedHostAccess::adapter_local(0, HostSchemaEpoch::new(0)),
            Some(HostPathPart::ConstIndex(_) | HostPathPart::DynIndex { .. }) => {
                if matches!(spec.op, crate::resolved::HostAccessOp::Call(_)) {
                    T::resolve_host_type_target(spec.at_offset(offset + 1))?
                        .prepend_prepared_adapter(0)
                } else if offset + 1 == spec.plan.parts.len() {
                    ResolvedHostAccess::adapter_local(0, HostSchemaEpoch::new(0))
                } else {
                    T::resolve_host_type_target_from(spec, offset + 1)?.prepend_prepared_adapter(0)
                }
            }
            Some(_) => ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)),
        })
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
            let index = usize::try_from(target_index(target, target.offset)?)
                .map_err(|_| invalid_arg("array index"))?;
            return self
                .get(index)
                .ok_or_else(|| missing_target(target))?
                .read_resolved_host_target_from(child_access, target.at_offset(target.offset + 1));
        }
        self.read_host_target_from(target, target.offset)
    }

    fn write_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let index = usize::try_from(target_index(target, target.offset)?)
                .map_err(|_| invalid_arg("array index"))?;
            return self
                .get_mut(index)
                .ok_or_else(|| missing_target(target))?
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
        op: crate::resolved::HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let index = usize::try_from(target_index(target, target.offset)?)
                .map_err(|_| invalid_arg("array index"))?;
            return self
                .get_mut(index)
                .ok_or_else(|| missing_target(target))?
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
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let index = usize::try_from(target_index(target, target.offset)?)
                .map_err(|_| invalid_arg("array index"))?;
            return self
                .get_mut(index)
                .ok_or_else(|| missing_target(target))?
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
        if target_is_leaf(target, offset) && TypeId::of::<T>() == TypeId::of::<u8>() {
            let bytes = (self as &dyn Any)
                .downcast_ref::<Vec<u8>>()
                .expect("Vec<T> TypeId matched Vec<u8>");
            return Ok(HostValue::Bytes(bytes.clone()));
        }
        let index = usize::try_from(target_index(target, offset)?)
            .map_err(|_| invalid_arg("array index"))?;
        self.get(index)
            .ok_or_else(|| missing_target(target))?
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
        let index = usize::try_from(target_index(target, offset)?)
            .map_err(|_| invalid_arg("array index"))?;
        self.get(index)
            .ok_or_else(|| missing_target(target))?
            .query_collection_host_target_from(target, offset + 1, query)
    }

    fn snapshot_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        if !target_is_leaf(target, offset) {
            let index = usize::try_from(target_index(target, offset)?)
                .map_err(|_| invalid_arg("array index"))?;
            return self
                .get(index)
                .ok_or_else(|| missing_target(target))?
                .snapshot_collection_host_target_from(target, offset + 1, projection);
        }
        if projection != HostCollectionProjection::Values {
            return Err(invalid_arg(projection.name()));
        }
        self.iter()
            .map(|value| value.read_host_target_from(target, offset))
            .collect::<HostResult<Vec<_>>>()
            .map(HostCollectionSnapshot::Items)
    }

    fn mutate_collection_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        if target_is_leaf(target, offset) {
            return mutate_vec(self, mutation);
        }
        let index = usize::try_from(target_index(target, offset)?)
            .map_err(|_| invalid_arg("array index"))?;
        self.get_mut(index)
            .ok_or_else(|| missing_target(target))?
            .mutate_collection_host_target_from(target, offset + 1, mutation)
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        if target_is_leaf(target, offset) && TypeId::of::<T>() == TypeId::of::<u8>() {
            let bytes = Vec::<u8>::from_host_value(&value)?;
            let target = (self as &mut dyn Any)
                .downcast_mut::<Vec<u8>>()
                .expect("Vec<T> TypeId matched Vec<u8>");
            *target = bytes;
            return Ok(());
        }
        let index = usize::try_from(target_index(target, offset)?)
            .map_err(|_| invalid_arg("array index"))?;
        self.get_mut(index)
            .ok_or_else(|| missing_target(target))?
            .write_host_target_from(target, offset + 1, value)
    }

    fn remove_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<()> {
        let index = usize::try_from(target_index(target, offset)?)
            .map_err(|_| invalid_arg("array index"))?;
        if target_is_leaf(target, offset + 1) {
            if index >= self.len() {
                return Err(missing_target(target));
            }
            self.remove(index);
            return Ok(());
        }
        self.get_mut(index)
            .ok_or_else(|| missing_target(target))?
            .remove_host_target_from(target, offset + 1)
    }

    fn call_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let index = usize::try_from(target_index(target, offset)?)
            .map_err(|_| invalid_arg("array index"))?;
        self.get_mut(index)
            .ok_or_else(|| missing_target(target))?
            .call_host_target_from(target, offset + 1, method, args)
    }
}
