use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use vela_common::{HostMethodId, HostTypeId, ScalarValue};

use crate::{
    error::{HostError, HostErrorKind, HostResult},
    path::HostRef,
    protocol::{
        HostCollectionKey, HostCollectionKeyRef, HostCollectionProjection, HostCollectionQuery,
        HostCollectionSnapshot,
    },
    resolved::{HostAccessOp, HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess},
    target::{HostPathArg, HostPathPart, HostTargetInstance},
    value::{HostValue, add_values, div_values, mul_values, rem_values, sub_values},
};

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
        Err(if target.plan.parts.is_empty() {
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
        Err(if target.plan.parts.is_empty() {
            invalid_arg(projection.name())
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
        let write_access =
            self.resolve_host_target(HostAccessSpec::new(HostAccessOp::Write, target.plan))?;
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
        Err(if target.plan.parts.is_empty() {
            unsupported_method(method)
        } else {
            missing_target(target)
        })
    }
}

pub trait ScriptHostFieldAccess {
    fn script_host_type_id(&self) -> HostTypeId;

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

impl ScriptHostKey for String {
    fn from_host_collection_key(key: HostCollectionKeyRef<'_>) -> HostResult<Self> {
        match key {
            HostCollectionKeyRef::String(key) => Ok(key.to_owned()),
            _ => Err(invalid_arg("String collection key")),
        }
    }

    fn to_host_collection_key(&self) -> HostCollectionKey {
        HostCollectionKey::String(self.clone())
    }
}

macro_rules! impl_script_host_key {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl ScriptHostKey for $ty {
                fn from_host_collection_key(
                    key: HostCollectionKeyRef<'_>,
                ) -> HostResult<Self> {
                    match key {
                        HostCollectionKeyRef::$variant(key) => Ok(key),
                        _ => Err(invalid_arg(concat!(stringify!($ty), " collection key"))),
                    }
                }

                fn to_host_collection_key(&self) -> HostCollectionKey {
                    HostCollectionKey::$variant(*self)
                }
            }
        )*
    };
}

impl_script_host_key!(
    bool => Bool,
    char => Char,
    i8 => I8,
    i16 => I16,
    i32 => I32,
    i64 => I64,
    u8 => U8,
    u16 => U16,
    u32 => U32,
    u64 => U64,
);

impl ScriptHostKey for Vec<u8> {
    fn from_host_collection_key(key: HostCollectionKeyRef<'_>) -> HostResult<Self> {
        match key {
            HostCollectionKeyRef::Bytes(key) => Ok(key.to_owned()),
            _ => Err(invalid_arg("Bytes collection key")),
        }
    }

    fn to_host_collection_key(&self) -> HostCollectionKey {
        HostCollectionKey::Bytes(self.clone())
    }
}

impl ScriptHostKey for HostRef {
    fn from_host_collection_key(key: HostCollectionKeyRef<'_>) -> HostResult<Self> {
        match key {
            HostCollectionKeyRef::HostRef(key) => Ok(key),
            _ => Err(invalid_arg("HostRef collection key")),
        }
    }

    fn to_host_collection_key(&self) -> HostCollectionKey {
        HostCollectionKey::HostRef(*self)
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

impl_script_host_object_via_field!(<K> BTreeSet<K> where K: ScriptHostKey + 'static);

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

impl_script_host_object_via_field!(<K> HashSet<K> where K: ScriptHostKey + Hash + 'static);

fn target_is_leaf(target: HostTargetInstance<'_>, offset: usize) -> bool {
    offset == target.plan.parts.len()
}

fn snapshot_map_entries<'a, K, V>(
    entries: impl IntoIterator<Item = (&'a K, &'a V)>,
    target: HostTargetInstance<'_>,
    offset: usize,
    projection: HostCollectionProjection,
) -> HostResult<HostCollectionSnapshot>
where
    K: ScriptHostKey + 'a,
    V: ScriptHostFieldAccess + 'a,
{
    match projection {
        HostCollectionProjection::Keys => Ok(HostCollectionSnapshot::Items(
            entries
                .into_iter()
                .map(|(key, _)| key.to_host_collection_key().into_host_value())
                .collect(),
        )),
        HostCollectionProjection::Values => entries
            .into_iter()
            .map(|(_, value)| value.read_host_target_from(target, offset))
            .collect::<HostResult<Vec<_>>>()
            .map(HostCollectionSnapshot::Items),
        HostCollectionProjection::Entries => entries
            .into_iter()
            .map(|(key, value)| {
                Ok((
                    key.to_host_collection_key().into_host_value(),
                    value.read_host_target_from(target, offset)?,
                ))
            })
            .collect::<HostResult<Vec<_>>>()
            .map(HostCollectionSnapshot::Entries),
    }
}

fn snapshot_set_values<'a, K>(
    values: impl IntoIterator<Item = &'a K>,
    projection: HostCollectionProjection,
) -> HostResult<HostCollectionSnapshot>
where
    K: ScriptHostKey + 'a,
{
    if projection == HostCollectionProjection::Entries {
        return Err(invalid_arg(projection.name()));
    }
    Ok(HostCollectionSnapshot::Items(
        values
            .into_iter()
            .map(|value| value.to_host_collection_key().into_host_value())
            .collect(),
    ))
}

fn target_part(target: HostTargetInstance<'_>, offset: usize) -> HostResult<&HostPathPart> {
    target
        .plan
        .parts
        .as_slice()
        .get(offset)
        .ok_or_else(|| missing_target(target))
}

fn target_key(
    target: HostTargetInstance<'_>,
    offset: usize,
) -> HostResult<HostCollectionKeyRef<'_>> {
    match target_part(target, offset)? {
        HostPathPart::ConstKey(key) => Ok(HostCollectionKeyRef::String(key)),
        HostPathPart::DynKey { arg } | HostPathPart::DynIndex { arg } => match target.arg(*arg) {
            Some(HostPathArg::Key(key)) => Ok(key),
            Some(HostPathArg::Index(_)) | None => Err(missing_target(target)),
        },
        HostPathPart::Field(_) | HostPathPart::VariantField(_) | HostPathPart::ConstIndex(_) => {
            Err(missing_target(target))
        }
    }
}

fn target_index(target: HostTargetInstance<'_>, offset: usize) -> HostResult<u32> {
    match target_part(target, offset)? {
        HostPathPart::ConstIndex(index) => Ok(*index),
        HostPathPart::DynIndex { arg } | HostPathPart::DynKey { arg } => match target.arg(*arg) {
            Some(HostPathArg::Index(index)) => Ok(index),
            Some(HostPathArg::Key(HostCollectionKeyRef::I64(index))) if index >= 0 => {
                u32::try_from(index).map_err(|_| missing_target(target))
            }
            Some(HostPathArg::Key(_)) | None => Err(missing_target(target)),
        },
        HostPathPart::Field(_) | HostPathPart::VariantField(_) | HostPathPart::ConstKey(_) => {
            Err(missing_target(target))
        }
    }
}

fn invalid_arg(expected: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument { expected },
        source_span: None,
    }
}

fn collection_query_result(len: usize, query: HostCollectionQuery) -> HostResult<HostValue> {
    match query {
        HostCollectionQuery::Len => i64::try_from(len)
            .map(|len| HostValue::Scalar(ScalarValue::I64(len)))
            .map_err(|_| invalid_arg("collection length within i64 range")),
        HostCollectionQuery::IsEmpty => Ok(HostValue::Bool(len == 0)),
    }
}

fn missing_target(target: HostTargetInstance<'_>) -> HostError {
    HostError {
        kind: HostErrorKind::MissingPath {
            path: target.to_diagnostic_path().to_host_path(),
        },
        source_span: None,
    }
}

fn missing_collection_entry(target: HostTargetInstance<'_>) -> HostError {
    HostError {
        kind: HostErrorKind::MissingCollectionEntry {
            path: target.to_diagnostic_path().to_host_path(),
        },
        source_span: None,
    }
}

fn permission_denied(target: HostTargetInstance<'_>, action: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::PermissionDenied {
            path: target.to_diagnostic_path().to_host_path(),
            action,
        },
        source_span: None,
    }
}

pub fn mutate_host_value(
    op: HostMutationOp,
    current: &HostValue,
    rhs: &HostValue,
    target: HostTargetInstance<'_>,
) -> HostResult<HostValue> {
    let next = match op {
        HostMutationOp::Add => add_values(current, rhs),
        HostMutationOp::Sub => sub_values(current, rhs),
        HostMutationOp::Mul => mul_values(current, rhs),
        HostMutationOp::Div => div_values(current, rhs),
        HostMutationOp::Rem => rem_values(current, rhs),
        HostMutationOp::Push => None,
    };
    next.ok_or_else(|| invalid_mutation_error(op, target))
}

fn invalid_mutation_error(op: HostMutationOp, target: HostTargetInstance<'_>) -> HostError {
    let path = target.to_diagnostic_path().to_host_path();
    HostError {
        kind: match op {
            HostMutationOp::Add => HostErrorKind::InvalidAdd { path },
            HostMutationOp::Sub => HostErrorKind::InvalidSub { path },
            HostMutationOp::Mul => HostErrorKind::InvalidMul { path },
            HostMutationOp::Div => HostErrorKind::InvalidDiv { path },
            HostMutationOp::Rem => HostErrorKind::InvalidRem { path },
            HostMutationOp::Push => HostErrorKind::InvalidPush { path },
        },
        source_span: None,
    }
}

fn unsupported_method(method: HostMethodId) -> HostError {
    HostError {
        kind: HostErrorKind::UnsupportedMethod { method },
        source_span: None,
    }
}

fn unsupported_collection_query(query: HostCollectionQuery) -> HostError {
    HostError {
        kind: HostErrorKind::UnsupportedCollectionQuery { query },
        source_span: None,
    }
}
