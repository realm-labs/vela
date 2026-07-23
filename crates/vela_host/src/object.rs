use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

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
mod mutation;
mod sets;
mod slice;
mod target;

use collection_protocol::{
    collection_query_result, mutate_btree_map, mutate_hash_map, mutate_vec,
    unsupported_collection_mutation, unsupported_collection_query,
};
use collection_snapshot::snapshot_map_entries;
use errors::{
    invalid_arg, missing_collection_entry, missing_target, permission_denied, unsupported_method,
};
pub use mutation::mutate_host_value;
pub use slice::{lease_slice_mut, lease_slice_ref};
use target::{target_index, target_is_leaf, target_key};

pub trait ScriptHostObject {
    fn host_type_id(&self) -> HostTypeId;

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
}

pub trait ScriptHostFieldAccess {
    fn script_host_type_id(&self) -> HostTypeId;

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
}

pub trait HostValueInto {
    fn into_host_value(self) -> HostResult<HostValue>;
}

pub trait HostValueFrom: Sized {
    fn from_host_value(value: &HostValue) -> HostResult<Self>;
}

pub trait ScriptHostKey: Clone + Eq + Ord {
    fn from_host_collection_key(key: HostCollectionKeyRef<'_>) -> HostResult<Self>;

    fn to_host_collection_key(&self) -> HostCollectionKey;
}

macro_rules! impl_script_host_object_via_field {
    (@impl ($($generics:tt)*) $ty:ty ; $($where_clause:tt)*) => {
        impl $($generics)* ScriptHostObject for $ty $($where_clause)* {
            fn host_type_id(&self) -> HostTypeId {
                ScriptHostFieldAccess::script_host_type_id(self)
            }

            fn lease_any(&self) -> Option<&dyn Any> {
                Some(self)
            }

            fn lease_any_mut(&mut self) -> Option<&mut dyn Any> {
                Some(self)
            }

            fn read_resolved_host(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
            ) -> HostResult<HostValue> {
                let _ = access;
                ScriptHostFieldAccess::read_host_target_from(self, target, 0)
            }

            fn query_collection_resolved_host(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                query: HostCollectionQuery,
            ) -> HostResult<HostValue> {
                let _ = access;
                ScriptHostFieldAccess::query_collection_host_target_from(self, target, 0, query)
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
                    0,
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
                    0,
                    mutation,
                )
            }

            fn write_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                value: HostValue,
            ) -> HostResult<()> {
                let _ = access;
                ScriptHostFieldAccess::write_host_target_from(self, target, 0, value)
            }

            fn mutate_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                op: HostMutationOp,
                rhs: HostValue,
            ) -> HostResult<()> {
                let _ = access;
                ScriptHostFieldAccess::mutate_host_target_from(self, target, 0, op, rhs)
            }

            fn remove_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
            ) -> HostResult<()> {
                let _ = access;
                ScriptHostFieldAccess::remove_host_target_from(self, target, 0)
            }

            fn call_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                method: HostMethodId,
                args: &[HostValue],
            ) -> HostResult<HostValue> {
                let _ = access;
                ScriptHostFieldAccess::call_host_target_from(self, target, 0, method, args)
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

impl<K, V> ScriptHostFieldAccess for BTreeMap<K, V>
where
    K: ScriptHostKey,
    V: ScriptHostFieldAccess,
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
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .call_host_target_from(target, offset + 1, method, args)
    }
}

impl_script_host_object_via_field!(<K, V> BTreeMap<K, V> where K: ScriptHostKey + 'static, V: ScriptHostFieldAccess + 'static);

impl<K, V> ScriptHostFieldAccess for HashMap<K, V>
where
    K: ScriptHostKey + Hash,
    V: ScriptHostFieldAccess,
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
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let key = K::from_host_collection_key(target_key(target, offset)?)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_collection_entry(target))?
            .call_host_target_from(target, offset + 1, method, args)
    }
}

impl_script_host_object_via_field!(<K, V> HashMap<K, V> where K: ScriptHostKey + Hash + 'static, V: ScriptHostFieldAccess + 'static);

impl<T> ScriptHostFieldAccess for Vec<T>
where
    T: ScriptHostFieldAccess + 'static,
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

impl_script_host_object_via_field!(<T> Vec<T> where T: ScriptHostFieldAccess + 'static);
impl_script_host_object_via_field!(<T, const N: usize> [T; N] where T: ScriptHostFieldAccess + 'static);

impl_script_host_object_via_field!(<K> BTreeSet<K> where K: ScriptHostKey + 'static);
impl_script_host_object_via_field!(<K> HashSet<K> where K: ScriptHostKey + Hash + 'static);
