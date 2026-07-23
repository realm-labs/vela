use better_any::{Tid, TidExt};
use vela_common::{HostMethodId, HostTypeId};

use crate::error::HostResult;
use crate::protocol::{
    HostCollectionMutation, HostCollectionProjection, HostCollectionQuery, HostCollectionSnapshot,
};
use crate::resolved::{HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use crate::target::HostTargetInstance;
use crate::value::HostValue;

use super::{
    ScriptHostFieldAccess, ScriptHostObject, collection_query_result, invalid_arg, missing_target,
    target_index, target_is_leaf, unsupported_collection_mutation,
};

struct SharedSliceReborrow<'a, T: 'static>(&'a [T]);
better_any::tid! { impl<'a, T:'static> TidAble<'a> for SharedSliceReborrow<'a, T> }

struct ExclusiveSliceReborrow<'a, T: 'static>(Option<&'a mut [T]>);
better_any::tid! { impl<'a, T:'static> TidAble<'a> for ExclusiveSliceReborrow<'a, T> }

#[doc(hidden)]
pub trait HostSliceRefVisitor<'a> {
    fn visit(&mut self, value: &(dyn Tid<'a> + 'a));
}

#[doc(hidden)]
pub trait HostSliceMutVisitor<'a> {
    fn visit(&mut self, value: &mut (dyn Tid<'a> + 'a));
}

/// Safely reborrows a dynamically-sized slice from a host object without
/// copying or exposing a Rust reference to script code.
#[must_use]
pub fn lease_slice_ref<T: 'static>(object: &dyn ScriptHostObject) -> Option<&[T]> {
    struct Visitor<'a, T: 'static> {
        value: Option<&'a [T]>,
    }

    impl<'a, T: 'static> HostSliceRefVisitor<'a> for Visitor<'a, T> {
        fn visit(&mut self, value: &(dyn Tid<'a> + 'a)) {
            self.value = value
                .downcast_ref::<SharedSliceReborrow<'a, T>>()
                .map(|slice| slice.0);
        }
    }

    let mut visitor = Visitor { value: None };
    object.visit_slice_ref(&mut visitor).then_some(())?;
    visitor.value
}

/// Mutable counterpart to [`lease_slice_ref`].
#[must_use]
pub fn lease_slice_mut<T: 'static>(object: &mut dyn ScriptHostObject) -> Option<&mut [T]> {
    struct Visitor<'a, T: 'static> {
        value: Option<&'a mut [T]>,
    }

    impl<'a, T: 'static> HostSliceMutVisitor<'a> for Visitor<'a, T> {
        fn visit(&mut self, value: &mut (dyn Tid<'a> + 'a)) {
            self.value = value
                .downcast_mut::<ExclusiveSliceReborrow<'a, T>>()
                .and_then(|slice| slice.0.take());
        }
    }

    let mut visitor = Visitor { value: None };
    object.visit_slice_mut(&mut visitor).then_some(())?;
    visitor.value
}

impl<T> ScriptHostFieldAccess for [T]
where
    T: ScriptHostFieldAccess,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
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
            return collection_query_result(self.len(), query);
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

impl<T: 'static> ScriptHostObject for [T]
where
    T: ScriptHostFieldAccess,
{
    fn host_type_id(&self) -> HostTypeId {
        ScriptHostFieldAccess::script_host_type_id(self)
    }

    fn visit_slice_ref<'a>(&'a self, visitor: &mut dyn HostSliceRefVisitor<'a>) -> bool {
        let value = SharedSliceReborrow(self);
        visitor.visit(&value);
        true
    }

    fn visit_slice_mut<'a>(&'a mut self, visitor: &mut dyn HostSliceMutVisitor<'a>) -> bool {
        let mut value = ExclusiveSliceReborrow(Some(self));
        visitor.visit(&mut value);
        true
    }

    fn supports_slice_ref(&self) -> bool {
        true
    }

    fn supports_slice_mut(&self) -> bool {
        true
    }

    fn resolve_host_target(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        ScriptHostFieldAccess::resolve_host_target_from(self, spec, 0)
    }

    fn read_resolved_host(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        ScriptHostFieldAccess::read_host_target_from(self, target, 0)
    }

    fn query_collection_resolved_host(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        ScriptHostFieldAccess::query_collection_host_target_from(self, target, 0, query)
    }

    fn snapshot_collection_resolved_host(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        ScriptHostFieldAccess::snapshot_collection_host_target_from(self, target, 0, projection)
    }

    fn mutate_collection_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        ScriptHostFieldAccess::mutate_collection_host_target_from(self, target, 0, mutation)
    }

    fn write_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        ScriptHostFieldAccess::write_host_target_from(self, target, 0, value)
    }

    fn mutate_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        ScriptHostFieldAccess::mutate_host_target_from(self, target, 0, op, rhs)
    }

    fn call_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        ScriptHostFieldAccess::call_host_target_from(self, target, 0, method, args)
    }
}

fn checked_index(target: HostTargetInstance<'_>, offset: usize) -> HostResult<usize> {
    usize::try_from(target_index(target, offset)?).map_err(|_| invalid_arg("slice index"))
}
