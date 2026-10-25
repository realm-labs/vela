use vela_common::ScalarValue;

use crate::call_value::HostCallValue;

#[derive(Clone, Debug, PartialEq)]
pub enum HostValue {
    Unit,
    Bool(bool),
    Char(char),
    Scalar(ScalarValue),
    String(String),
    Bytes(Vec<u8>),
    /// An owned structural value with no Rust identity or implicit write-back.
    Detached(Box<HostCallValue>),
}

impl HostValue {
    #[must_use]
    pub const fn i64(value: i64) -> Self {
        Self::Scalar(ScalarValue::I64(value))
    }

    #[must_use]
    pub const fn f64(value: f64) -> Self {
        Self::Scalar(ScalarValue::F64(value))
    }
}
