use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use vela_common::{HostObjectId, HostTypeId};
use vela_host::path::HostRef;
use vela_host::proxy::PathProxy;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::value::Value;

pub trait IntoScriptArg {
    fn into_script_arg(self) -> Value;
}

pub trait IntoHostArg {
    fn into_host_ref(self) -> HostRef;
}

pub trait FromScriptArg: Sized {
    const TYPE_NAME: &'static str;

    fn from_script_arg(value: &Value) -> VmResult<Self>;
}

pub trait ScriptArgsExt {
    fn required<T: FromScriptArg>(&self, index: usize) -> VmResult<T>;
}

impl ScriptArgsExt for [Value] {
    fn required<T: FromScriptArg>(&self, index: usize) -> VmResult<T> {
        let value = self.get(index).ok_or_else(|| VmError {
            kind: VmErrorKind::ArityMismatch {
                name: "native argument conversion".to_owned(),
                expected: index.saturating_add(1),
                actual: self.len(),
            },
            source_span: None,
            call_stack: Default::default(),
        })?;
        T::from_script_arg(value)
    }
}

impl IntoScriptArg for Value {
    fn into_script_arg(self) -> Value {
        self
    }
}

impl IntoScriptArg for &Value {
    fn into_script_arg(self) -> Value {
        self.clone()
    }
}

impl FromScriptArg for Value {
    const TYPE_NAME: &'static str = "value";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        Ok(value.clone())
    }
}

impl IntoScriptArg for HostRef {
    fn into_script_arg(self) -> Value {
        Value::HostRef(self)
    }
}

impl IntoHostArg for HostRef {
    fn into_host_ref(self) -> HostRef {
        self
    }
}

impl FromScriptArg for HostRef {
    const TYPE_NAME: &'static str = "host ref";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::HostRef(host_ref) => Ok(*host_ref),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

impl IntoScriptArg for &HostRef {
    fn into_script_arg(self) -> Value {
        Value::HostRef(*self)
    }
}

impl IntoHostArg for &HostRef {
    fn into_host_ref(self) -> HostRef {
        *self
    }
}

impl IntoHostArg for (u32, u64, u32) {
    fn into_host_ref(self) -> HostRef {
        HostRef::new(HostTypeId::new(self.0), HostObjectId::new(self.1), self.2)
    }
}

impl IntoHostArg for (HostTypeId, HostObjectId, u32) {
    fn into_host_ref(self) -> HostRef {
        HostRef::new(self.0, self.1, self.2)
    }
}

impl IntoScriptArg for PathProxy {
    fn into_script_arg(self) -> Value {
        Value::PathProxy(self)
    }
}

impl FromScriptArg for PathProxy {
    const TYPE_NAME: &'static str = "path proxy";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::PathProxy(proxy) => Ok(proxy.clone()),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

impl IntoScriptArg for &PathProxy {
    fn into_script_arg(self) -> Value {
        Value::PathProxy(self.clone())
    }
}

impl IntoScriptArg for () {
    fn into_script_arg(self) -> Value {
        Value::Null
    }
}

impl IntoScriptArg for bool {
    fn into_script_arg(self) -> Value {
        Value::Bool(self)
    }
}

impl FromScriptArg for bool {
    const TYPE_NAME: &'static str = "bool";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Bool(value) => Ok(*value),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

macro_rules! int_arg {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoScriptArg for $ty {
                fn into_script_arg(self) -> Value {
                    Value::Int(i64::from(self))
                }
            }
        )*
    };
}

int_arg!(i8, i16, i32, i64, u8, u16, u32);

macro_rules! signed_from_arg {
    ($($ty:ty),* $(,)?) => {
        $(
            impl FromScriptArg for $ty {
                const TYPE_NAME: &'static str = "int";

                fn from_script_arg(value: &Value) -> VmResult<Self> {
                    match value {
                        Value::Int(value) => (*value)
                            .try_into()
                            .map_err(|_| type_mismatch(Self::TYPE_NAME)),
                        _ => Err(type_mismatch(Self::TYPE_NAME)),
                    }
                }
            }
        )*
    };
}

signed_from_arg!(i8, i16, i32, i64, u8, u16, u32);

impl IntoScriptArg for f32 {
    fn into_script_arg(self) -> Value {
        Value::Float(f64::from(self))
    }
}

impl FromScriptArg for f32 {
    const TYPE_NAME: &'static str = "float";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Float(value) => f32_from_f64(*value),
            Value::Int(value) => f32_from_f64(*value as f64),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

impl IntoScriptArg for f64 {
    fn into_script_arg(self) -> Value {
        Value::Float(self)
    }
}

impl FromScriptArg for f64 {
    const TYPE_NAME: &'static str = "float";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Float(value) => Ok(*value),
            Value::Int(value) => Ok(*value as f64),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

fn f32_from_f64(value: f64) -> VmResult<f32> {
    let converted = value as f32;
    if value.is_finite() && !converted.is_finite() {
        return Err(type_mismatch(<f32 as FromScriptArg>::TYPE_NAME));
    }
    Ok(converted)
}

impl IntoScriptArg for String {
    fn into_script_arg(self) -> Value {
        Value::String(self)
    }
}

impl IntoScriptArg for &str {
    fn into_script_arg(self) -> Value {
        Value::String(self.to_owned())
    }
}

impl FromScriptArg for String {
    const TYPE_NAME: &'static str = "string";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::String(value) => Ok(value.clone()),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

impl<T> IntoScriptArg for Option<T>
where
    T: IntoScriptArg,
{
    fn into_script_arg(self) -> Value {
        match self {
            Some(value) => value.into_script_arg(),
            None => Value::Null,
        }
    }
}

impl<T> FromScriptArg for Option<T>
where
    T: FromScriptArg,
{
    const TYPE_NAME: &'static str = "option";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Null => Ok(None),
            Value::Enum {
                enum_name,
                variant,
                fields,
            } if enum_name == "Option" || enum_name.rsplit('.').next() == Some("Option") => {
                match variant.as_str() {
                    "Some" => fields
                        .get("0")
                        .ok_or_else(|| type_mismatch(Self::TYPE_NAME))
                        .and_then(T::from_script_arg)
                        .map(Some),
                    "None" => Ok(None),
                    _ => Err(type_mismatch(Self::TYPE_NAME)),
                }
            }
            value => T::from_script_arg(value).map(Some),
        }
    }
}

impl<T, E> IntoScriptArg for std::result::Result<T, E>
where
    T: IntoScriptArg,
    E: IntoScriptArg,
{
    fn into_script_arg(self) -> Value {
        match self {
            Ok(value) => enum_payload("Result", "Ok", value.into_script_arg()),
            Err(error) => enum_payload("Result", "Err", error.into_script_arg()),
        }
    }
}

impl<T, E> FromScriptArg for std::result::Result<T, E>
where
    T: FromScriptArg,
    E: FromScriptArg,
{
    const TYPE_NAME: &'static str = "result";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Enum {
                enum_name,
                variant,
                fields,
            } if enum_name == "Result" || enum_name.rsplit('.').next() == Some("Result") => {
                let payload = fields
                    .get("0")
                    .ok_or_else(|| type_mismatch(Self::TYPE_NAME))?;
                match variant.as_str() {
                    "Ok" => T::from_script_arg(payload).map(Ok),
                    "Err" => E::from_script_arg(payload).map(Err),
                    _ => Err(type_mismatch(Self::TYPE_NAME)),
                }
            }
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

impl<T> IntoScriptArg for Vec<T>
where
    T: IntoScriptArg,
{
    fn into_script_arg(self) -> Value {
        Value::Array(
            self.into_iter()
                .map(IntoScriptArg::into_script_arg)
                .collect(),
        )
    }
}

impl<T> FromScriptArg for Vec<T>
where
    T: FromScriptArg,
{
    const TYPE_NAME: &'static str = "array";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Array(values) => values.iter().map(T::from_script_arg).collect(),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

impl<T, const N: usize> IntoScriptArg for [T; N]
where
    T: IntoScriptArg,
{
    fn into_script_arg(self) -> Value {
        Value::Array(
            self.into_iter()
                .map(IntoScriptArg::into_script_arg)
                .collect(),
        )
    }
}

impl<T, const N: usize> FromScriptArg for [T; N]
where
    T: FromScriptArg,
{
    const TYPE_NAME: &'static str = "array";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        let Value::Array(values) = value else {
            return Err(type_mismatch(Self::TYPE_NAME));
        };
        if values.len() != N {
            return Err(type_mismatch(Self::TYPE_NAME));
        }
        let converted = values
            .iter()
            .map(T::from_script_arg)
            .collect::<VmResult<Vec<_>>>()?;
        converted
            .try_into()
            .map_err(|_| type_mismatch(Self::TYPE_NAME))
    }
}

impl<K, T> IntoScriptArg for BTreeMap<K, T>
where
    K: Into<String> + Ord,
    T: IntoScriptArg,
{
    fn into_script_arg(self) -> Value {
        Value::Map(
            self.into_iter()
                .map(|(key, value)| (key.into(), value.into_script_arg()))
                .collect(),
        )
    }
}

impl<T> FromScriptArg for BTreeMap<String, T>
where
    T: FromScriptArg,
{
    const TYPE_NAME: &'static str = "map";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Map(values) => values
                .iter()
                .map(|(key, value)| Ok((key.clone(), T::from_script_arg(value)?)))
                .collect(),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

impl<K, T> IntoScriptArg for HashMap<K, T>
where
    K: Into<String> + Eq + Hash,
    T: IntoScriptArg,
{
    fn into_script_arg(self) -> Value {
        Value::Map(
            self.into_iter()
                .map(|(key, value)| (key.into(), value.into_script_arg()))
                .collect(),
        )
    }
}

impl<T> FromScriptArg for HashMap<String, T>
where
    T: FromScriptArg,
{
    const TYPE_NAME: &'static str = "map";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Map(values) => values
                .iter()
                .map(|(key, value)| Ok((key.clone(), T::from_script_arg(value)?)))
                .collect(),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

impl<T> IntoScriptArg for BTreeSet<T>
where
    T: IntoScriptArg,
{
    fn into_script_arg(self) -> Value {
        Value::Set(
            self.into_iter()
                .map(IntoScriptArg::into_script_arg)
                .collect(),
        )
    }
}

impl<T> FromScriptArg for BTreeSet<T>
where
    T: FromScriptArg + Ord,
{
    const TYPE_NAME: &'static str = "set";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Set(values) => values.iter().map(T::from_script_arg).collect(),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

impl<T> IntoScriptArg for HashSet<T>
where
    T: IntoScriptArg + Eq + Hash + Ord,
{
    fn into_script_arg(self) -> Value {
        let mut values = self.into_iter().collect::<Vec<_>>();
        values.sort();
        Value::Set(
            values
                .into_iter()
                .map(IntoScriptArg::into_script_arg)
                .collect(),
        )
    }
}

impl<T> FromScriptArg for HashSet<T>
where
    T: FromScriptArg + Eq + Hash,
{
    const TYPE_NAME: &'static str = "set";

    fn from_script_arg(value: &Value) -> VmResult<Self> {
        match value {
            Value::Set(values) => values.iter().map(T::from_script_arg).collect(),
            _ => Err(type_mismatch(Self::TYPE_NAME)),
        }
    }
}

fn type_mismatch(operation: &'static str) -> VmError {
    VmError {
        kind: VmErrorKind::TypeMismatch { operation },
        source_span: None,
        call_stack: Default::default(),
    }
}

fn enum_payload(enum_name: &str, variant: &str, payload: Value) -> Value {
    Value::Enum {
        enum_name: enum_name.to_owned(),
        variant: variant.to_owned(),
        fields: [("0".to_owned(), payload)].into(),
    }
}

#[doc(hidden)]
#[must_use]
pub fn empty_args() -> Vec<Value> {
    Vec::new()
}

#[doc(hidden)]
#[must_use]
pub fn host_ref_value(type_id: u32, object_id: u64, generation: u32) -> Value {
    host((type_id, object_id, generation))
}

#[must_use]
pub fn host(host: impl IntoHostArg) -> Value {
    Value::HostRef(host.into_host_ref())
}

#[macro_export]
macro_rules! args {
    () => {
        $crate::args::empty_args()
    };
    ($($arg:expr),+ $(,)?) => {
        ::std::vec![$($crate::args::IntoScriptArg::into_script_arg($arg)),+]
    };
}

#[macro_export]
macro_rules! host {
    ($type_id:expr, $object_id:expr, $generation:expr $(,)?) => {
        $crate::args::host_ref_value($type_id, $object_id, $generation)
    };
    ($host:expr $(,)?) => {
        $crate::args::host($host)
    };
}
