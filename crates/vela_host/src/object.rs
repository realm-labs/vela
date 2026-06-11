use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use vela_common::{HostMethodId, HostTypeId};

use crate::{
    error::{HostError, HostErrorKind, HostResult},
    resolved::{HostAccessOp, HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess},
    target::{HostPathArg, HostPathPart, HostTargetInstance},
    value::HostValue,
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

macro_rules! impl_signed_int_host_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl HostValueInto for $ty {
                fn into_host_value(self) -> HostResult<HostValue> {
                    Ok(HostValue::i64(i64::from(self)))
                }
            }

            impl HostValueFrom for $ty {
                fn from_host_value(value: &HostValue) -> HostResult<Self> {
                    match value {
                        HostValue::Scalar(vela_common::ScalarValue::I64(value)) => <$ty>::try_from(*value).map_err(|_| invalid_arg("int value")),
                        _ => Err(invalid_arg("int value")),
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

macro_rules! impl_unsigned_int_host_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl HostValueInto for $ty {
                fn into_host_value(self) -> HostResult<HostValue> {
                    Ok(HostValue::i64(i64::from(self)))
                }
            }

            impl HostValueFrom for $ty {
                fn from_host_value(value: &HostValue) -> HostResult<Self> {
                    match value {
                        HostValue::Scalar(vela_common::ScalarValue::I64(value)) => <$ty>::try_from(*value).map_err(|_| invalid_arg("int value")),
                        _ => Err(invalid_arg("int value")),
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

impl_signed_int_host_value!(i8, i16, i32, i64);
impl_unsigned_int_host_value!(u8, u16, u32);

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

impl_script_host_object_via_field!(<T> Vec<T> where T: ScriptHostFieldAccess);

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
        HostMutationOp::Add => host_add_values(current, rhs),
        HostMutationOp::Sub => host_sub_values(current, rhs),
        HostMutationOp::Mul => host_mul_values(current, rhs),
        HostMutationOp::Div => host_div_values(current, rhs),
        HostMutationOp::Rem => host_rem_values(current, rhs),
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

fn host_add_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (
            HostValue::Scalar(vela_common::ScalarValue::I64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => lhs.checked_add(*rhs).map(HostValue::i64),
        (
            HostValue::Scalar(vela_common::ScalarValue::F64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Some(HostValue::Scalar(vela_common::ScalarValue::F64(lhs + rhs))),
        (HostValue::String(lhs), HostValue::String(rhs)) => {
            Some(HostValue::String(format!("{lhs}{rhs}")))
        }
        _ => None,
    }
}

fn host_sub_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (
            HostValue::Scalar(vela_common::ScalarValue::I64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => lhs.checked_sub(*rhs).map(HostValue::i64),
        (
            HostValue::Scalar(vela_common::ScalarValue::F64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Some(HostValue::Scalar(vela_common::ScalarValue::F64(lhs - rhs))),
        _ => None,
    }
}

fn host_mul_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (
            HostValue::Scalar(vela_common::ScalarValue::I64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => lhs.checked_mul(*rhs).map(HostValue::i64),
        (
            HostValue::Scalar(vela_common::ScalarValue::F64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Some(HostValue::Scalar(vela_common::ScalarValue::F64(lhs * rhs))),
        _ => None,
    }
}

fn host_div_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (
            HostValue::Scalar(vela_common::ScalarValue::I64(_)),
            HostValue::Scalar(vela_common::ScalarValue::I64(0)),
        ) => None,
        (
            HostValue::Scalar(vela_common::ScalarValue::I64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => lhs.checked_div(*rhs).map(HostValue::i64),
        (
            HostValue::Scalar(vela_common::ScalarValue::F64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) if *rhs != 0.0 => Some(HostValue::Scalar(vela_common::ScalarValue::F64(lhs / rhs))),
        _ => None,
    }
}

fn host_rem_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (
            HostValue::Scalar(vela_common::ScalarValue::I64(_)),
            HostValue::Scalar(vela_common::ScalarValue::I64(0)),
        ) => None,
        (
            HostValue::Scalar(vela_common::ScalarValue::I64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => lhs.checked_rem(*rhs).map(HostValue::i64),
        (
            HostValue::Scalar(vela_common::ScalarValue::F64(lhs)),
            HostValue::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) if *rhs != 0.0 => Some(HostValue::Scalar(vela_common::ScalarValue::F64(lhs % rhs))),
        _ => None,
    }
}

fn unsupported_method(method: HostMethodId) -> HostError {
    HostError {
        kind: HostErrorKind::UnsupportedMethod { method },
        source_span: None,
    }
}
