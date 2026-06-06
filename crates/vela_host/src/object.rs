use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use vela_common::{HostMethodId, HostTypeId};

use crate::{
    error::{HostError, HostErrorKind, HostResult},
    path::HostPath,
    value::HostValue,
};

pub trait ScriptHostObject {
    fn host_type_id(&self) -> HostTypeId;

    fn read_host_path(&self, path: &HostPath) -> HostResult<HostValue>;

    fn write_host_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        let _ = value;
        Err(HostError {
            kind: HostErrorKind::PermissionDenied {
                path: path.clone(),
                action: "write",
            },
            source_span: None,
        })
    }

    fn remove_host_path(&mut self, path: &HostPath) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn call_host_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let _ = args;
        Err(HostError {
            kind: if path.segments.is_empty() {
                HostErrorKind::UnsupportedMethod { method }
            } else {
                HostErrorKind::MissingPath { path: path.clone() }
            },
            source_span: None,
        })
    }
}

pub trait ScriptHostFieldAccess {
    fn script_host_type_id(&self) -> HostTypeId;

    fn read_host_path_from(&self, path: &HostPath, offset: usize) -> HostResult<HostValue>;

    fn write_host_path_from(
        &mut self,
        path: &HostPath,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()>;

    fn call_host_method_from(
        &mut self,
        path: &HostPath,
        offset: usize,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let _ = args;
        Err(HostError {
            kind: if offset >= path.segments.len() {
                HostErrorKind::UnsupportedMethod { method }
            } else {
                HostErrorKind::MissingPath { path: path.clone() }
            },
            source_span: None,
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

macro_rules! impl_signed_int_host_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl HostValueInto for $ty {
                fn into_host_value(self) -> HostResult<HostValue> {
                    Ok(HostValue::Int(i64::from(self)))
                }
            }

            impl HostValueFrom for $ty {
                fn from_host_value(value: &HostValue) -> HostResult<Self> {
                    match value {
                        HostValue::Int(value) => <$ty>::try_from(*value).map_err(|_| invalid_arg("int value")),
                        _ => Err(invalid_arg("int value")),
                    }
                }
            }

            impl ScriptHostFieldAccess for $ty {
                fn script_host_type_id(&self) -> HostTypeId {
                    HostTypeId::new(0)
                }

                fn read_host_path_from(&self, path: &HostPath, offset: usize) -> HostResult<HostValue> {
                    if offset == path.segments.len() {
                        (*self).into_host_value()
                    } else {
                        Err(missing_path(path))
                    }
                }

                fn write_host_path_from(
                    &mut self,
                    path: &HostPath,
                    offset: usize,
                    value: HostValue,
                ) -> HostResult<()> {
                    if offset == path.segments.len() {
                        *self = <$ty as HostValueFrom>::from_host_value(&value)?;
                        Ok(())
                    } else {
                        Err(missing_path(path))
                    }
                }
            }
        )*
    };
}

macro_rules! impl_unsigned_int_host_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl HostValueInto for $ty {
                fn into_host_value(self) -> HostResult<HostValue> {
                    Ok(HostValue::Int(i64::from(self)))
                }
            }

            impl HostValueFrom for $ty {
                fn from_host_value(value: &HostValue) -> HostResult<Self> {
                    match value {
                        HostValue::Int(value) => <$ty>::try_from(*value).map_err(|_| invalid_arg("int value")),
                        _ => Err(invalid_arg("int value")),
                    }
                }
            }

            impl ScriptHostFieldAccess for $ty {
                fn script_host_type_id(&self) -> HostTypeId {
                    HostTypeId::new(0)
                }

                fn read_host_path_from(&self, path: &HostPath, offset: usize) -> HostResult<HostValue> {
                    if offset == path.segments.len() {
                        (*self).into_host_value()
                    } else {
                        Err(missing_path(path))
                    }
                }

                fn write_host_path_from(
                    &mut self,
                    path: &HostPath,
                    offset: usize,
                    value: HostValue,
                ) -> HostResult<()> {
                    if offset == path.segments.len() {
                        *self = <$ty as HostValueFrom>::from_host_value(&value)?;
                        Ok(())
                    } else {
                        Err(missing_path(path))
                    }
                }
            }
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

    fn read_host_path_from(&self, path: &HostPath, offset: usize) -> HostResult<HostValue> {
        if offset == path.segments.len() {
            (*self).into_host_value()
        } else {
            Err(missing_path(path))
        }
    }

    fn write_host_path_from(
        &mut self,
        path: &HostPath,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        if offset == path.segments.len() {
            *self = bool::from_host_value(&value)?;
            Ok(())
        } else {
            Err(missing_path(path))
        }
    }
}

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

    fn read_host_path_from(&self, path: &HostPath, offset: usize) -> HostResult<HostValue> {
        if offset == path.segments.len() {
            self.as_str().into_host_value()
        } else {
            Err(missing_path(path))
        }
    }

    fn write_host_path_from(
        &mut self,
        path: &HostPath,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        if offset == path.segments.len() {
            *self = String::from_host_value(&value)?;
            Ok(())
        } else {
            Err(missing_path(path))
        }
    }
}

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
    V: Default + ScriptHostFieldAccess,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn read_host_path_from(&self, path: &HostPath, offset: usize) -> HostResult<HostValue> {
        let Some(crate::path::PathSegment::Key(key)) = path.segments.get(offset) else {
            return Err(missing_path(path));
        };
        let key = K::parse_host_key(key)?;
        self.get(&key)
            .ok_or_else(|| missing_path(path))?
            .read_host_path_from(path, offset + 1)
    }

    fn write_host_path_from(
        &mut self,
        path: &HostPath,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        let Some(crate::path::PathSegment::Key(key)) = path.segments.get(offset) else {
            return Err(missing_path(path));
        };
        let key = K::parse_host_key(key)?;
        self.entry(key)
            .or_default()
            .write_host_path_from(path, offset + 1, value)
    }

    fn call_host_method_from(
        &mut self,
        path: &HostPath,
        offset: usize,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let Some(crate::path::PathSegment::Key(key)) = path.segments.get(offset) else {
            return Err(missing_path(path));
        };
        let key = K::parse_host_key(key)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_path(path))?
            .call_host_method_from(path, offset + 1, method, args)
    }
}

impl<K, V> ScriptHostFieldAccess for HashMap<K, V>
where
    K: ScriptHostKey + Hash,
    V: Default + ScriptHostFieldAccess,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn read_host_path_from(&self, path: &HostPath, offset: usize) -> HostResult<HostValue> {
        let Some(crate::path::PathSegment::Key(key)) = path.segments.get(offset) else {
            return Err(missing_path(path));
        };
        let key = K::parse_host_key(key)?;
        self.get(&key)
            .ok_or_else(|| missing_path(path))?
            .read_host_path_from(path, offset + 1)
    }

    fn write_host_path_from(
        &mut self,
        path: &HostPath,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        let Some(crate::path::PathSegment::Key(key)) = path.segments.get(offset) else {
            return Err(missing_path(path));
        };
        let key = K::parse_host_key(key)?;
        self.entry(key)
            .or_default()
            .write_host_path_from(path, offset + 1, value)
    }

    fn call_host_method_from(
        &mut self,
        path: &HostPath,
        offset: usize,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let Some(crate::path::PathSegment::Key(key)) = path.segments.get(offset) else {
            return Err(missing_path(path));
        };
        let key = K::parse_host_key(key)?;
        self.get_mut(&key)
            .ok_or_else(|| missing_path(path))?
            .call_host_method_from(path, offset + 1, method, args)
    }
}

impl<K> ScriptHostFieldAccess for BTreeSet<K>
where
    K: ScriptHostKey,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn read_host_path_from(&self, path: &HostPath, offset: usize) -> HostResult<HostValue> {
        let Some(crate::path::PathSegment::Key(key)) = path.segments.get(offset) else {
            return Err(missing_path(path));
        };
        let key = K::parse_host_key(key)?;
        if offset + 1 == path.segments.len() {
            Ok(HostValue::Bool(self.contains(&key)))
        } else {
            Err(missing_path(path))
        }
    }

    fn write_host_path_from(
        &mut self,
        path: &HostPath,
        _offset: usize,
        _value: HostValue,
    ) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::PermissionDenied {
                path: path.clone(),
                action: "write",
            },
            source_span: None,
        })
    }
}

impl<K> ScriptHostFieldAccess for HashSet<K>
where
    K: ScriptHostKey + Hash,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn read_host_path_from(&self, path: &HostPath, offset: usize) -> HostResult<HostValue> {
        let Some(crate::path::PathSegment::Key(key)) = path.segments.get(offset) else {
            return Err(missing_path(path));
        };
        let key = K::parse_host_key(key)?;
        if offset + 1 == path.segments.len() {
            Ok(HostValue::Bool(self.contains(&key)))
        } else {
            Err(missing_path(path))
        }
    }

    fn write_host_path_from(
        &mut self,
        path: &HostPath,
        _offset: usize,
        _value: HostValue,
    ) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::PermissionDenied {
                path: path.clone(),
                action: "write",
            },
            source_span: None,
        })
    }
}

fn invalid_arg(expected: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument { expected },
        source_span: None,
    }
}

fn missing_path(path: &HostPath) -> HostError {
    HostError {
        kind: HostErrorKind::MissingPath { path: path.clone() },
        source_span: None,
    }
}
