use std::collections::BTreeMap;

use vela_host::HostRef;
use vela_vm::Value;

pub trait IntoScriptArg {
    fn into_script_arg(self) -> Value;
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

impl IntoScriptArg for HostRef {
    fn into_script_arg(self) -> Value {
        Value::HostRef(self)
    }
}

impl IntoScriptArg for &HostRef {
    fn into_script_arg(self) -> Value {
        Value::HostRef(*self)
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

impl IntoScriptArg for f32 {
    fn into_script_arg(self) -> Value {
        Value::Float(f64::from(self))
    }
}

impl IntoScriptArg for f64 {
    fn into_script_arg(self) -> Value {
        Value::Float(self)
    }
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

#[macro_export]
macro_rules! args {
    () => {
        ::std::vec::Vec::<$crate::Value>::new()
    };
    ($($arg:expr),+ $(,)?) => {
        ::std::vec![$($crate::IntoScriptArg::into_script_arg($arg)),+]
    };
}

#[macro_export]
macro_rules! host {
    ($type_id:expr, $object_id:expr, $generation:expr $(,)?) => {
        $crate::Value::HostRef($crate::HostRef::new(
            $crate::HostTypeId::new($type_id),
            $crate::HostObjectId::new($object_id),
            $generation,
        ))
    };
}
