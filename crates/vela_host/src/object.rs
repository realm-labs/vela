use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use vela_common::{HostMethodId, HostTypeId, ScalarValue};

use crate::{
    error::{HostError, HostErrorKind, HostResult},
    resolved::{HostAccessOp, HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess},
    target::{HostPathArg, HostPathPart, HostTargetInstance},
    value::{HostValue, add_values, div_values, mul_values, rem_values, sub_values},
};

pub trait ScriptHostObject {
    fn host_type_id(&self) -> HostTypeId;

    fn resolve_host_target(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        let _ = spec;
        Ok(ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)))
    }

    fn read_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue>;

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
    fn parse_host_key(key: &str) -> HostResult<Self>;
}

macro_rules! impl_script_host_object_via_field {
    (@impl ($($generics:tt)*) $ty:ty ; $($where_clause:tt)*) => {
        impl $($generics)* ScriptHostObject for $ty $($where_clause)* {
            fn host_type_id(&self) -> HostTypeId {
                ScriptHostFieldAccess::script_host_type_id(self)
            }

            fn read_resolved_host(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
            ) -> HostResult<HostValue> {
                let _ = access;
                ScriptHostFieldAccess::read_host_target_from(self, target, 0)
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
        Ok(HostValue::Null)
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
    fn parse_host_key(key: &str) -> HostResult<Self> {
        Ok(key.to_owned())
    }
}

impl ScriptHostKey for i64 {
    fn parse_host_key(key: &str) -> HostResult<Self> {
        key.parse().map_err(|_| invalid_arg("integer host key"))
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
        let key = target_key(target, offset)?;
        let key = K::parse_host_key(key)?;
        self.get(&key)
            .ok_or_else(|| missing_target(target))?
            .read_host_target_from(target, offset + 1)
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        let key = target_key(target, offset)?;
        let key = K::parse_host_key(key)?;
        self.get_mut(&key)
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
        let key = target_key(target, offset)?;
        let key = K::parse_host_key(key)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_target(target))?
            .call_host_target_from(target, offset + 1, method, args)
    }
}

impl_script_host_object_via_field!(<K, V> BTreeMap<K, V> where K: ScriptHostKey, V: ScriptHostFieldAccess);

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
        let key = target_key(target, offset)?;
        let key = K::parse_host_key(key)?;
        self.get(&key)
            .ok_or_else(|| missing_target(target))?
            .read_host_target_from(target, offset + 1)
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        let key = target_key(target, offset)?;
        let key = K::parse_host_key(key)?;
        self.get_mut(&key)
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
        let key = target_key(target, offset)?;
        let key = K::parse_host_key(key)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_target(target))?
            .call_host_target_from(target, offset + 1, method, args)
    }
}

impl_script_host_object_via_field!(<K, V> HashMap<K, V> where K: ScriptHostKey + Hash, V: ScriptHostFieldAccess);

impl<T> ScriptHostFieldAccess for Vec<T>
where
    T: ScriptHostFieldAccess + 'static,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
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
        let key = target_key(target, offset)?;
        let key = K::parse_host_key(key)?;
        if offset + 1 == target.plan.parts.len() {
            Ok(HostValue::Bool(self.contains(&key)))
        } else {
            Err(missing_target(target))
        }
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        _offset: usize,
        _value: HostValue,
    ) -> HostResult<()> {
        Err(permission_denied(target, "write"))
    }
}

impl_script_host_object_via_field!(<K> BTreeSet<K> where K: ScriptHostKey);

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
        let key = target_key(target, offset)?;
        let key = K::parse_host_key(key)?;
        if offset + 1 == target.plan.parts.len() {
            Ok(HostValue::Bool(self.contains(&key)))
        } else {
            Err(missing_target(target))
        }
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        _offset: usize,
        _value: HostValue,
    ) -> HostResult<()> {
        Err(permission_denied(target, "write"))
    }
}

impl_script_host_object_via_field!(<K> HashSet<K> where K: ScriptHostKey + Hash);

fn target_is_leaf(target: HostTargetInstance<'_>, offset: usize) -> bool {
    offset == target.plan.parts.len()
}

fn target_part(target: HostTargetInstance<'_>, offset: usize) -> HostResult<&HostPathPart> {
    target
        .plan
        .parts
        .as_slice()
        .get(offset)
        .ok_or_else(|| missing_target(target))
}

fn target_key(target: HostTargetInstance<'_>, offset: usize) -> HostResult<&str> {
    match target_part(target, offset)? {
        HostPathPart::ConstKey(key) => Ok(key),
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

fn missing_target(target: HostTargetInstance<'_>) -> HostError {
    HostError {
        kind: HostErrorKind::MissingPath {
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
