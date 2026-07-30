//! Detached structural values used by schema-declared Host method calls.

use vela_common::ScalarValue;

use crate::path::HostRef;
use crate::value::HostValue;

/// A VM-independent, owned value crossing an erased Host method vtable.
///
/// Field and path access continue to use [`HostValue`]. This wider vocabulary
/// exists only for method arguments and results, where registered Rust methods
/// may accept ordinary records, enums, and collections.
#[derive(Clone, Debug, PartialEq)]
pub enum HostCallValue {
    Unit,
    Bool(bool),
    Char(char),
    Scalar(ScalarValue),
    String(String),
    Bytes(Vec<u8>),
    Tuple(Vec<Self>),
    Array(Vec<Self>),
    Map(Vec<HostCallMapEntry>),
    Set(Vec<Self>),
    Record {
        type_name: String,
        fields: Vec<HostCallField>,
    },
    Enum {
        enum_name: String,
        variant: String,
        fields: Vec<HostCallField>,
    },
    HostRef(HostRef),
}

impl HostCallValue {
    #[must_use]
    pub const fn i64(value: i64) -> Self {
        Self::Scalar(ScalarValue::I64(value))
    }

    #[must_use]
    pub fn record(
        type_name: impl Into<String>,
        fields: impl IntoIterator<Item = (impl Into<String>, Self)>,
    ) -> Self {
        Self::Record {
            type_name: type_name.into(),
            fields: fields
                .into_iter()
                .map(|(name, value)| HostCallField::new(name, value))
                .collect(),
        }
    }

    #[must_use]
    pub fn enum_variant(
        enum_name: impl Into<String>,
        variant: impl Into<String>,
        fields: impl IntoIterator<Item = (impl Into<String>, Self)>,
    ) -> Self {
        Self::Enum {
            enum_name: enum_name.into(),
            variant: variant.into(),
            fields: fields
                .into_iter()
                .map(|(name, value)| HostCallField::new(name, value))
                .collect(),
        }
    }

    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Self> {
        match self {
            Self::Record { fields, .. } | Self::Enum { fields, .. } => fields
                .iter()
                .find(|field| field.name == name)
                .map(|field| &field.value),
            _ => None,
        }
    }

    /// Converts the scalar Host field vocabulary into the method vocabulary.
    #[must_use]
    pub fn from_host_value(value: HostValue) -> Self {
        match value {
            HostValue::Unit => Self::Unit,
            HostValue::Bool(value) => Self::Bool(value),
            HostValue::Char(value) => Self::Char(value),
            HostValue::Scalar(value) => Self::Scalar(value),
            HostValue::String(value) => Self::String(value),
            HostValue::Bytes(value) => Self::Bytes(value),
            HostValue::HostRef(value) => Self::HostRef(value),
        }
    }

    /// Attempts to narrow a method value to the Host field vocabulary.
    #[must_use]
    pub fn to_host_value(&self) -> Option<HostValue> {
        match self {
            Self::Unit => Some(HostValue::Unit),
            Self::Bool(value) => Some(HostValue::Bool(*value)),
            Self::Char(value) => Some(HostValue::Char(*value)),
            Self::Scalar(value) => Some(HostValue::Scalar(*value)),
            Self::String(value) => Some(HostValue::String(value.clone())),
            Self::Bytes(value) => Some(HostValue::Bytes(value.clone())),
            Self::HostRef(value) => Some(HostValue::HostRef(*value)),
            Self::Tuple(_)
            | Self::Array(_)
            | Self::Map(_)
            | Self::Set(_)
            | Self::Record { .. }
            | Self::Enum { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostCallField {
    pub name: String,
    pub value: HostCallValue,
}

impl HostCallField {
    #[must_use]
    pub fn new(name: impl Into<String>, value: HostCallValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostCallMapEntry {
    pub key: HostCallValue,
    pub value: HostCallValue,
}

impl HostCallMapEntry {
    #[must_use]
    pub const fn new(key: HostCallValue, value: HostCallValue) -> Self {
        Self { key, value }
    }
}
