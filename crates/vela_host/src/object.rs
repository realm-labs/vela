use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;

use vela_common::{HostMethodId, HostTypeId, ScalarValue};

use crate::{
    error::HostResult,
    protocol::{
        HostCollectionKey, HostCollectionKeyRef, HostCollectionMutation, HostCollectionProjection,
        HostCollectionQuery, HostCollectionSnapshot,
    },
    resolved::{HostAccessOp, HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess},
    target::HostTargetInstance,
    value::HostValue,
};

mod collection_protocol;
mod collection_snapshot;
mod errors;
mod fixed_array;
mod keys;
mod maps;
mod mutation;
mod sequence;
mod sets;
mod slice;
mod target;

use collection_protocol::{
    collection_query_result, unsupported_collection_mutation, unsupported_collection_query,
};
use errors::{invalid_arg, missing_target, permission_denied, unsupported_method};
pub use mutation::mutate_host_value;
pub use slice::{lease_slice_mut, lease_slice_ref};
use target::{target_index, target_is_leaf};

pub type HostCallFuture<'call> =
    Pin<Box<dyn Future<Output = HostResult<HostValue>> + Send + 'call>>;

pub trait ScriptHostObject {
    fn host_type_id(&self) -> HostTypeId;

    /// Resolves a target from Rust type structure without requiring an object.
    #[doc(hidden)]
    fn resolve_host_type_target(_spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess>
    where
        Self: Sized,
    {
        Ok(ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)))
    }

    /// Exposes a concrete `'static` direct host object to Rust-only lease
    /// wrappers. Opaque or non-`'static` implementations remain ineligible.
    fn lease_any(&self) -> Option<&dyn Any> {
        None
    }

    /// Mutable counterpart to [`ScriptHostObject::lease_any`].
    fn lease_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }

    /// Produces a call-scoped erased shared slice borrow for generated Rust
    /// adapters. The erased representation is private to `vela_host` and is
    /// never stored in a Vela value or host-reference payload.
    #[doc(hidden)]
    fn erased_slice_ref(&self) -> Option<crate::erased_slice::ErasedSliceRef<'_>> {
        None
    }

    /// Exclusive counterpart to [`ScriptHostObject::erased_slice_ref`].
    #[doc(hidden)]
    fn erased_slice_mut(&mut self) -> Option<crate::erased_slice::ErasedSliceMut<'_>> {
        None
    }

    fn resolve_host_target(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        let _ = spec;
        Ok(ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)))
    }

    fn read_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue>;

    /// Creates one call-scoped shared child when a collection element is a
    /// host object rather than a copied boundary value.
    #[doc(hidden)]
    fn borrow_resolved_host_shared(
        &self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        Ok(None)
    }

    /// Exclusive counterpart to [`ScriptHostObject::borrow_resolved_host_shared`].
    #[doc(hidden)]
    fn borrow_resolved_host_exclusive(
        &mut self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        Ok(None)
    }

    /// Projects homogeneous host-object collection values as scoped children.
    #[doc(hidden)]
    fn borrow_collection_resolved_host_shared(
        &self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        _projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        Ok(None)
    }

    /// Exclusive counterpart to
    /// [`ScriptHostObject::borrow_collection_resolved_host_shared`].
    #[doc(hidden)]
    fn borrow_collection_resolved_host_exclusive(
        &mut self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        _projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        Ok(None)
    }

    fn query_collection_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        let _ = access;
        Err(if target.offset >= target.plan.parts.len() {
            unsupported_collection_query(query)
        } else {
            missing_target(target)
        })
    }

    fn snapshot_collection_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        let _ = access;
        Err(if target.offset >= target.plan.parts.len() {
            invalid_arg(projection.name())
        } else {
            missing_target(target)
        })
    }

    fn mutate_collection_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        let _ = access;
        Err(if target.offset >= target.plan.parts.len() {
            unsupported_collection_mutation(mutation)
        } else {
            missing_target(target)
        })
    }

    fn write_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        let _ = access;
        let _ = value;
        Err(permission_denied(target, "write"))
    }

    fn mutate_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        let current = self.read_resolved_host(access, target)?;
        let next = mutate_host_value(op, &current, &rhs, target)?;
        let write_access = self.resolve_host_target(
            HostAccessSpec::new(HostAccessOp::Write, target.plan).at_offset(target.offset),
        )?;
        self.write_resolved_host(write_access, target, next)
    }

    fn remove_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        let _ = access;
        Err(missing_target(target))
    }

    fn call_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let _ = access;
        let _ = args;
        Err(if target.offset >= target.plan.parts.len() {
            unsupported_method(method)
        } else {
            missing_target(target)
        })
    }

    /// Dispatches one schema-declared async shared receiver without requiring
    /// a concrete Rust downcast.
    fn call_async_host_shared<'call>(
        &'call self,
        method: HostMethodId,
        _args: Vec<HostValue>,
    ) -> HostCallFuture<'call> {
        let error = unsupported_method(method);
        Box::pin(async move { Err(error) })
    }

    /// Dispatches one schema-declared async exclusive receiver without
    /// requiring a concrete Rust downcast.
    fn call_async_host_exclusive<'call>(
        &'call mut self,
        method: HostMethodId,
        _args: Vec<HostValue>,
    ) -> HostCallFuture<'call> {
        let error = unsupported_method(method);
        Box::pin(async move { Err(error) })
    }
}

/// Borrowed collection payload retained under one parent lease by the Runtime.
#[doc(hidden)]
pub enum ScopedHostCollectionDependents<'host> {
    Items(Vec<crate::lease::ScopedHostDependent<'host>>),
    Entries(Vec<(HostValue, crate::lease::ScopedHostDependent<'host>)>),
}

pub trait ScriptHostFieldAccess {
    fn script_host_type_id(&self) -> HostTypeId;

    /// Returns the canonical Vela-facing shape used when this Rust type is
    /// nested inside a concrete standard collection binding.
    ///
    /// Handwritten host adapters may omit the shape when they can never be a
    /// standard collection element. Generated and built-in adapters provide
    /// it so nested collection HostRefs retain the sealed binding identity.
    #[doc(hidden)]
    fn script_host_type_shape() -> Option<String>
    where
        Self: Sized,
    {
        None
    }

    /// Resolves a target from Rust type structure without requiring a live
    /// collection element. Generated adapters override this for field paths.
    #[doc(hidden)]
    fn resolve_host_type_target_from(
        _spec: HostAccessSpec<'_>,
        _offset: usize,
    ) -> HostResult<ResolvedHostAccess> {
        Ok(ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)))
    }

    /// Reads a field selected by the adapter's dense, schema-local slot.
    ///
    /// Generated host adapters override this entry point. The default keeps
    /// handwritten adapters compatible with ordinary target traversal.
    #[doc(hidden)]
    fn read_direct_field(
        &self,
        slot: u32,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        let _ = slot;
        self.read_host_target_from(target, 0)
    }

    /// Writes a field selected by the adapter's dense, schema-local slot.
    #[doc(hidden)]
    fn write_direct_field(
        &mut self,
        slot: u32,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        let _ = slot;
        self.write_host_target_from(target, 0, value)
    }

    /// Mutates a field selected by the adapter's dense, schema-local slot.
    #[doc(hidden)]
    fn mutate_direct_field(
        &mut self,
        slot: u32,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        let current = self.read_direct_field(slot, target)?;
        let next = mutate_host_value(op, &current, &rhs, target)?;
        self.write_direct_field(slot, target, next)
    }

    fn from_host_collection_value(_value: HostValue) -> HostResult<Self>
    where
        Self: Sized,
    {
        Err(invalid_arg("host collection value"))
    }

    fn resolve_host_target_from(
        &self,
        spec: HostAccessSpec<'_>,
        offset: usize,
    ) -> HostResult<ResolvedHostAccess> {
        let _ = spec;
        let _ = offset;
        Ok(ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)))
    }

    /// Executes a resolved target from the cursor carried by `target`.
    #[doc(hidden)]
    fn read_resolved_host_target_from(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        let _ = access;
        self.read_host_target_from(target, target.offset)
    }

    #[doc(hidden)]
    fn borrow_resolved_host_shared(
        &self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        Ok(None)
    }

    #[doc(hidden)]
    fn borrow_resolved_host_exclusive(
        &mut self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        Ok(None)
    }

    #[doc(hidden)]
    fn borrow_collection_resolved_host_shared(
        &self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        _projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        Ok(None)
    }

    #[doc(hidden)]
    fn borrow_collection_resolved_host_exclusive(
        &mut self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        _projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        Ok(None)
    }

    /// Executes a resolved write from the cursor carried by `target`.
    #[doc(hidden)]
    fn write_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        let _ = access;
        self.write_host_target_from(target, target.offset, value)
    }

    /// Executes a resolved mutation from the cursor carried by `target`.
    #[doc(hidden)]
    fn mutate_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        let _ = access;
        self.mutate_host_target_from(target, target.offset, op, rhs)
    }

    /// Executes a resolved method call from the cursor carried by `target`.
    #[doc(hidden)]
    fn call_resolved_host_target_from(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let _ = access;
        self.call_host_target_from(target, target.offset, method, args)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue>;

    fn query_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        Err(if target_is_leaf(target, offset) {
            unsupported_collection_query(query)
        } else {
            missing_target(target)
        })
    }

    fn snapshot_collection_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        Err(if target_is_leaf(target, offset) {
            invalid_arg(projection.name())
        } else {
            missing_target(target)
        })
    }

    fn mutate_collection_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        Err(if target_is_leaf(target, offset) {
            unsupported_collection_mutation(mutation)
        } else {
            missing_target(target)
        })
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()>;

    fn mutate_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        let current = self.read_host_target_from(target, offset)?;
        let next = mutate_host_value(op, &current, &rhs, target)?;
        self.write_host_target_from(target, offset, next)
    }

    fn remove_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        _offset: usize,
    ) -> HostResult<()> {
        Err(missing_target(target))
    }

    fn call_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let _ = args;
        Err(if offset >= target.plan.parts.len() {
            unsupported_method(method)
        } else {
            missing_target(target)
        })
    }

    /// Calls through a generated schema-local field slot in a prepared nested
    /// adapter chain. Handwritten adapters retain validated target traversal.
    #[doc(hidden)]
    fn call_prepared_field_target(
        &mut self,
        slot: u32,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let _ = slot;
        let _ = access;
        self.call_host_target_from(target, target.offset, method, args)
    }

    #[doc(hidden)]
    fn read_prepared_field_target(
        &self,
        slot: u32,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        let _ = slot;
        let _ = access;
        self.read_host_target_from(target, target.offset)
    }

    #[doc(hidden)]
    fn borrow_prepared_field_shared(
        &self,
        _slot: u32,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        Ok(None)
    }

    #[doc(hidden)]
    fn borrow_prepared_field_exclusive(
        &mut self,
        _slot: u32,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
        Ok(None)
    }

    #[doc(hidden)]
    fn borrow_collection_prepared_field_shared(
        &self,
        _slot: u32,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        _projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        Ok(None)
    }

    #[doc(hidden)]
    fn borrow_collection_prepared_field_exclusive(
        &mut self,
        _slot: u32,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        _projection: HostCollectionProjection,
    ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
        Ok(None)
    }

    #[doc(hidden)]
    fn write_prepared_field_target(
        &mut self,
        slot: u32,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        let _ = slot;
        let _ = access;
        self.write_host_target_from(target, target.offset, value)
    }

    #[doc(hidden)]
    fn mutate_prepared_field_target(
        &mut self,
        slot: u32,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        let _ = slot;
        let _ = access;
        self.mutate_host_target_from(target, target.offset, op, rhs)
    }

    #[doc(hidden)]
    fn query_prepared_field_target(
        &self,
        slot: u32,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        let _ = slot;
        let _ = access;
        self.query_collection_host_target_from(target, target.offset, query)
    }

    #[doc(hidden)]
    fn snapshot_prepared_field_target(
        &self,
        slot: u32,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        let _ = slot;
        let _ = access;
        self.snapshot_collection_host_target_from(target, target.offset, projection)
    }

    #[doc(hidden)]
    fn mutate_collection_prepared_field_target(
        &mut self,
        slot: u32,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        let _ = slot;
        let _ = access;
        self.mutate_collection_host_target_from(target, target.offset, mutation)
    }
}

pub trait HostValueInto {
    fn into_host_value(self) -> HostResult<HostValue>;
}

pub trait HostValueFrom: Sized {
    fn from_host_value(value: &HostValue) -> HostResult<Self>;
}

pub trait ScriptHostKey: Clone + Eq + Ord {
    #[doc(hidden)]
    fn script_host_key_shape() -> Option<&'static str> {
        None
    }

    fn from_host_collection_key(key: HostCollectionKeyRef<'_>) -> HostResult<Self>;

    fn to_host_collection_key(&self) -> HostCollectionKey;
}

macro_rules! impl_script_host_object_via_field {
    (@impl ($($generics:tt)*) $ty:ty ; $($where_clause:tt)*) => {
        impl $($generics)* ScriptHostObject for $ty $($where_clause)* {
            fn host_type_id(&self) -> HostTypeId {
                ScriptHostFieldAccess::script_host_type_id(self)
            }

            fn resolve_host_type_target(
                spec: HostAccessSpec<'_>,
            ) -> HostResult<ResolvedHostAccess> {
                <Self as ScriptHostFieldAccess>::resolve_host_type_target_from(spec, spec.offset)
            }

            fn lease_any(&self) -> Option<&dyn Any> {
                Some(self)
            }

            fn lease_any_mut(&mut self) -> Option<&mut dyn Any> {
                Some(self)
            }

            fn resolve_host_target(
                &self,
                spec: HostAccessSpec<'_>,
            ) -> HostResult<ResolvedHostAccess> {
                ScriptHostFieldAccess::resolve_host_target_from(self, spec, spec.offset)
            }

            fn read_resolved_host(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
            ) -> HostResult<HostValue> {
                ScriptHostFieldAccess::read_resolved_host_target_from(self, access, target)
            }

            fn borrow_resolved_host_shared(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
            ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
                ScriptHostFieldAccess::borrow_resolved_host_shared(self, access, target)
            }

            fn borrow_resolved_host_exclusive(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
            ) -> HostResult<Option<crate::lease::ScopedHostDependent<'_>>> {
                ScriptHostFieldAccess::borrow_resolved_host_exclusive(self, access, target)
            }

            fn borrow_collection_resolved_host_shared(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                projection: HostCollectionProjection,
            ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
                ScriptHostFieldAccess::borrow_collection_resolved_host_shared(
                    self, access, target, projection,
                )
            }

            fn borrow_collection_resolved_host_exclusive(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                projection: HostCollectionProjection,
            ) -> HostResult<Option<ScopedHostCollectionDependents<'_>>> {
                ScriptHostFieldAccess::borrow_collection_resolved_host_exclusive(
                    self, access, target, projection,
                )
            }

            fn query_collection_resolved_host(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                query: HostCollectionQuery,
            ) -> HostResult<HostValue> {
                let _ = access;
                ScriptHostFieldAccess::query_collection_host_target_from(
                    self,
                    target,
                    target.offset,
                    query,
                )
            }

            fn snapshot_collection_resolved_host(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                projection: HostCollectionProjection,
            ) -> HostResult<HostCollectionSnapshot> {
                let _ = access;
                ScriptHostFieldAccess::snapshot_collection_host_target_from(
                    self,
                    target,
                    target.offset,
                    projection,
                )
            }

            fn mutate_collection_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                mutation: HostCollectionMutation<'_>,
            ) -> HostResult<()> {
                let _ = access;
                ScriptHostFieldAccess::mutate_collection_host_target_from(
                    self,
                    target,
                    target.offset,
                    mutation,
                )
            }

            fn write_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                value: HostValue,
            ) -> HostResult<()> {
                ScriptHostFieldAccess::write_resolved_host_target_from(
                    self, access, target, value,
                )
            }

            fn mutate_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                op: HostMutationOp,
                rhs: HostValue,
            ) -> HostResult<()> {
                ScriptHostFieldAccess::mutate_resolved_host_target_from(
                    self, access, target, op, rhs,
                )
            }

            fn remove_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
            ) -> HostResult<()> {
                let _ = access;
                ScriptHostFieldAccess::remove_host_target_from(self, target, target.offset)
            }

            fn call_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                method: HostMethodId,
                args: &[HostValue],
            ) -> HostResult<HostValue> {
                ScriptHostFieldAccess::call_resolved_host_target_from(
                    self, access, target, method, args,
                )
            }
        }
    };
    (<$($generics:ident),+> $ty:ty where $($bounds:tt)+) => {
        impl_script_host_object_via_field!(@impl (<$($generics),+>) $ty ; where $($bounds)+);
    };
    (<$generic:ident, const $constant:ident: $constant_ty:ty> $ty:ty where $($bounds:tt)+) => {
        impl_script_host_object_via_field!(
            @impl (<$generic, const $constant: $constant_ty>) $ty ; where $($bounds)+
        );
    };
    ($ty:ty) => {
        impl_script_host_object_via_field!(@impl () $ty ;);
    };
}

macro_rules! impl_scalar_host_value {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl HostValueInto for $ty {
                fn into_host_value(self) -> HostResult<HostValue> {
                    Ok(HostValue::Scalar(ScalarValue::$variant(self)))
                }
            }

            impl HostValueFrom for $ty {
                fn from_host_value(value: &HostValue) -> HostResult<Self> {
                    match value {
                        HostValue::Scalar(ScalarValue::$variant(value)) => Ok(*value),
                        _ => Err(invalid_arg(stringify!($ty))),
                    }
                }
            }

            impl ScriptHostFieldAccess for $ty {
                fn script_host_type_id(&self) -> HostTypeId {
                    HostTypeId::new(0)
                }

                fn script_host_type_shape() -> Option<String> {
                    Some(stringify!($ty).to_owned())
                }

                fn from_host_collection_value(value: HostValue) -> HostResult<Self> {
                    <$ty as HostValueFrom>::from_host_value(&value)
                }

                fn read_host_target_from(
                    &self,
                    target: HostTargetInstance<'_>,
                    offset: usize,
                ) -> HostResult<HostValue> {
                    if target_is_leaf(target, offset) {
                        (*self).into_host_value()
                    } else {
                        Err(missing_target(target))
                    }
                }

                fn write_host_target_from(
                    &mut self,
                    target: HostTargetInstance<'_>,
                    offset: usize,
                    value: HostValue,
                ) -> HostResult<()> {
                    if target_is_leaf(target, offset) {
                        *self = <$ty as HostValueFrom>::from_host_value(&value)?;
                        Ok(())
                    } else {
                        Err(missing_target(target))
                    }
                }
            }

            impl_script_host_object_via_field!($ty);
        )*
    };
}

impl_scalar_host_value!(
    i8 => I8,
    i16 => I16,
    i32 => I32,
    i64 => I64,
    u8 => U8,
    u16 => U16,
    u32 => U32,
    u64 => U64,
    f32 => F32,
    f64 => F64,
);

impl HostValueInto for bool {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::Bool(self))
    }
}

impl HostValueFrom for bool {
    fn from_host_value(value: &HostValue) -> HostResult<Self> {
        match value {
            HostValue::Bool(value) => Ok(*value),
            _ => Err(invalid_arg("bool value")),
        }
    }
}

impl ScriptHostFieldAccess for bool {
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn script_host_type_shape() -> Option<String> {
        Some("bool".to_owned())
    }

    fn from_host_collection_value(value: HostValue) -> HostResult<Self> {
        bool::from_host_value(&value)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        if target_is_leaf(target, offset) {
            (*self).into_host_value()
        } else {
            Err(missing_target(target))
        }
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        if target_is_leaf(target, offset) {
            *self = bool::from_host_value(&value)?;
            Ok(())
        } else {
            Err(missing_target(target))
        }
    }
}

impl_script_host_object_via_field!(bool);

impl HostValueInto for String {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::String(self))
    }
}

impl HostValueInto for &str {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::String(self.to_owned()))
    }
}

impl HostValueFrom for String {
    fn from_host_value(value: &HostValue) -> HostResult<Self> {
        match value {
            HostValue::String(value) => Ok(value.clone()),
            _ => Err(invalid_arg("string value")),
        }
    }
}

impl HostValueInto for Vec<u8> {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::Bytes(self))
    }
}

impl HostValueInto for &[u8] {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::Bytes(self.to_vec()))
    }
}

impl HostValueFrom for Vec<u8> {
    fn from_host_value(value: &HostValue) -> HostResult<Self> {
        match value {
            HostValue::Bytes(value) => Ok(value.clone()),
            _ => Err(invalid_arg("bytes")),
        }
    }
}

impl ScriptHostFieldAccess for String {
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn script_host_type_shape() -> Option<String> {
        Some("String".to_owned())
    }

    fn from_host_collection_value(value: HostValue) -> HostResult<Self> {
        String::from_host_value(&value)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        if target_is_leaf(target, offset) {
            self.as_str().into_host_value()
        } else {
            Err(missing_target(target))
        }
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        if target_is_leaf(target, offset) {
            *self = String::from_host_value(&value)?;
            Ok(())
        } else {
            Err(missing_target(target))
        }
    }
}

impl_script_host_object_via_field!(String);

impl HostValueInto for () {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::Unit)
    }
}

impl HostValueInto for HostValue {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(self)
    }
}

impl<T: HostValueInto> HostValueInto for HostResult<T> {
    fn into_host_value(self) -> HostResult<HostValue> {
        self.and_then(HostValueInto::into_host_value)
    }
}

impl_script_host_object_via_field!(<K, V> BTreeMap<K, V> where K: ScriptHostKey + 'static, V: ScriptHostFieldAccess + ScriptHostObject + Send + Sync + 'static);

impl_script_host_object_via_field!(<K, V> HashMap<K, V> where K: ScriptHostKey + Hash + 'static, V: ScriptHostFieldAccess + ScriptHostObject + Send + Sync + 'static);

impl_script_host_object_via_field!(<T> Vec<T> where T: ScriptHostFieldAccess + ScriptHostObject + Send + Sync + 'static);
impl_script_host_object_via_field!(<T, const N: usize> [T; N] where T: ScriptHostFieldAccess + ScriptHostObject + Send + Sync + 'static);

impl_script_host_object_via_field!(<K> BTreeSet<K> where K: ScriptHostKey + 'static);
impl_script_host_object_via_field!(<K> HashSet<K> where K: ScriptHostKey + Hash + 'static);
