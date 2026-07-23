use vela_common::{HostMethodId, HostTypeId};

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
    ScriptHostFieldAccess, collection_query_result, invalid_arg, missing_target, target_index,
    target_is_leaf, unsupported_collection_mutation,
};

impl<T, const N: usize> ScriptHostFieldAccess for [T; N]
where
    T: ScriptHostFieldAccess,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn resolve_host_type_target_from(
        spec: HostAccessSpec<'_>,
        offset: usize,
    ) -> HostResult<ResolvedHostAccess> {
        Ok(match spec.plan.parts.as_slice().get(offset) {
            None => ResolvedHostAccess::adapter_local(0, HostSchemaEpoch::new(0)),
            Some(HostPathPart::ConstIndex(_) | HostPathPart::DynIndex { .. })
                if offset + 1 == spec.plan.parts.len() =>
            {
                ResolvedHostAccess::adapter_local(0, HostSchemaEpoch::new(0))
            }
            Some(HostPathPart::ConstIndex(_) | HostPathPart::DynIndex { .. }) => {
                T::resolve_host_type_target_from(spec, offset + 1)?.prepend_prepared_adapter(0)
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
            let index = checked_index(target, target.offset)?;
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
            let index = checked_index(target, target.offset)?;
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
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        if let Some((PreparedHostStep::AdapterLocal(0), child_access)) = access.next_prepared_step()
        {
            let index = checked_index(target, target.offset)?;
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

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        let index = checked_index(target, offset)?;
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
            return collection_query_result(N, query);
        }
        let index = checked_index(target, offset)?;
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
            let index = checked_index(target, offset)?;
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
            return Err(unsupported_collection_mutation(mutation));
        }
        let index = checked_index(target, offset)?;
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
        let index = checked_index(target, offset)?;
        self.get_mut(index)
            .ok_or_else(|| missing_target(target))?
            .write_host_target_from(target, offset + 1, value)
    }

    fn call_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let index = checked_index(target, offset)?;
        self.get_mut(index)
            .ok_or_else(|| missing_target(target))?
            .call_host_target_from(target, offset + 1, method, args)
    }
}

fn checked_index(target: HostTargetInstance<'_>, offset: usize) -> HostResult<usize> {
    usize::try_from(target_index(target, offset)?).map_err(|_| invalid_arg("array index"))
}
