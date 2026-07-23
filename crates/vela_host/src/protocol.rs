use vela_common::ScalarValue;

use crate::path::HostRef;
use crate::value::HostValue;

/// Read-only collection operations understood by the host boundary.
///
/// These operations are semantic protocol identities rather than Vela
/// standard-library method IDs. Host adapters therefore stay independent of
/// the language spelling used to invoke the protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostCollectionQuery {
    Len,
    IsEmpty,
}

/// One bounded projection of a host-backed collection.
///
/// The projection is captured while the host lease is active, then consumed by
/// the VM as a script iterator. It deliberately carries only boundary values;
/// Rust container storage never moves under the script heap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostCollectionProjection {
    Keys,
    Values,
    Entries,
}

/// Write-through collection operations understood by the host boundary.
///
/// These operations are semantic protocol identities rather than Vela
/// standard-library method IDs. One operation may therefore be shared by
/// standard and user-defined Rust collection bindings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HostCollectionMutation<'a> {
    Clear,
    ExtendSequence(&'a [HostValue]),
    InsertSequence {
        index: usize,
        value: &'a HostValue,
    },
    /// Retains the elements selected by one complete decision mask.
    ///
    /// Adapters reject the request if the live length differs from the
    /// projected length so callback decisions cannot resize stale storage.
    RetainSequence {
        expected_len: usize,
        keep: &'a [bool],
    },
    ExtendMap(&'a [(HostCollectionKey, HostValue)]),
    ExtendSet(&'a [HostCollectionKey]),
    /// Retains selected Map entries or Set values after validating the key set.
    ///
    /// Both slices contain exact boundary keys. Adapters reject duplicate
    /// keys, retained keys outside `expected`, and any live key-set change.
    RetainKeys {
        expected: &'a [HostCollectionKey],
        keep: &'a [HostCollectionKey],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostCollectionMutationKind {
    Clear,
    ExtendSequence,
    InsertSequence,
    RetainSequence,
    ExtendMap,
    ExtendSet,
    RetainKeys,
}

impl HostCollectionMutation<'_> {
    #[must_use]
    pub const fn kind(self) -> HostCollectionMutationKind {
        match self {
            Self::Clear => HostCollectionMutationKind::Clear,
            Self::ExtendSequence(_) => HostCollectionMutationKind::ExtendSequence,
            Self::InsertSequence { .. } => HostCollectionMutationKind::InsertSequence,
            Self::RetainSequence { .. } => HostCollectionMutationKind::RetainSequence,
            Self::ExtendMap(_) => HostCollectionMutationKind::ExtendMap,
            Self::ExtendSet(_) => HostCollectionMutationKind::ExtendSet,
            Self::RetainKeys { .. } => HostCollectionMutationKind::RetainKeys,
        }
    }
}

impl HostCollectionMutationKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::ExtendSequence => "extend sequence",
            Self::InsertSequence => "insert sequence",
            Self::RetainSequence => "retain sequence",
            Self::ExtendMap => "extend map",
            Self::ExtendSet => "extend set",
            Self::RetainKeys => "retain keyed collection",
        }
    }
}

impl HostCollectionProjection {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Keys => "host collection keys projection",
            Self::Values => "host collection values projection",
            Self::Entries => "host collection entries projection",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HostCollectionSnapshot {
    Items(Vec<HostValue>),
    Entries(Vec<(HostValue, HostValue)>),
}

/// An owned, exact key crossing the Vela/Rust collection boundary.
///
/// Unlike diagnostic `HostPath` strings, this value preserves the Rust-facing
/// primitive width and signedness. Floating-point values are deliberately not
/// keys because Rust's standard ordered/hash maps require total equality.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostCollectionKey {
    Bool(bool),
    Char(char),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    String(String),
    Bytes(Vec<u8>),
    HostRef(HostRef),
}

impl HostCollectionKey {
    #[must_use]
    pub fn as_ref(&self) -> HostCollectionKeyRef<'_> {
        self.into()
    }

    #[must_use]
    pub fn into_host_value(self) -> HostValue {
        match self {
            Self::Bool(value) => HostValue::Bool(value),
            Self::Char(value) => HostValue::Char(value),
            Self::I8(value) => HostValue::Scalar(ScalarValue::I8(value)),
            Self::I16(value) => HostValue::Scalar(ScalarValue::I16(value)),
            Self::I32(value) => HostValue::Scalar(ScalarValue::I32(value)),
            Self::I64(value) => HostValue::Scalar(ScalarValue::I64(value)),
            Self::U8(value) => HostValue::Scalar(ScalarValue::U8(value)),
            Self::U16(value) => HostValue::Scalar(ScalarValue::U16(value)),
            Self::U32(value) => HostValue::Scalar(ScalarValue::U32(value)),
            Self::U64(value) => HostValue::Scalar(ScalarValue::U64(value)),
            Self::String(value) => HostValue::String(value),
            Self::Bytes(value) => HostValue::Bytes(value),
            Self::HostRef(value) => HostValue::HostRef(value),
        }
    }
}

/// A borrowed, exact key used by one resolved host operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostCollectionKeyRef<'a> {
    Bool(bool),
    Char(char),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    String(&'a str),
    Bytes(&'a [u8]),
    HostRef(HostRef),
}

impl HostCollectionKeyRef<'_> {
    #[must_use]
    pub fn to_owned_key(self) -> HostCollectionKey {
        match self {
            Self::Bool(value) => HostCollectionKey::Bool(value),
            Self::Char(value) => HostCollectionKey::Char(value),
            Self::I8(value) => HostCollectionKey::I8(value),
            Self::I16(value) => HostCollectionKey::I16(value),
            Self::I32(value) => HostCollectionKey::I32(value),
            Self::I64(value) => HostCollectionKey::I64(value),
            Self::U8(value) => HostCollectionKey::U8(value),
            Self::U16(value) => HostCollectionKey::U16(value),
            Self::U32(value) => HostCollectionKey::U32(value),
            Self::U64(value) => HostCollectionKey::U64(value),
            Self::String(value) => HostCollectionKey::String(value.to_owned()),
            Self::Bytes(value) => HostCollectionKey::Bytes(value.to_owned()),
            Self::HostRef(value) => HostCollectionKey::HostRef(value),
        }
    }

    #[must_use]
    pub fn diagnostic_label(self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Char(value) => value.to_string(),
            Self::I8(value) => ScalarValue::I8(value).to_string(),
            Self::I16(value) => ScalarValue::I16(value).to_string(),
            Self::I32(value) => ScalarValue::I32(value).to_string(),
            Self::I64(value) => ScalarValue::I64(value).to_string(),
            Self::U8(value) => ScalarValue::U8(value).to_string(),
            Self::U16(value) => ScalarValue::U16(value).to_string(),
            Self::U32(value) => ScalarValue::U32(value).to_string(),
            Self::U64(value) => ScalarValue::U64(value).to_string(),
            Self::String(value) => value.to_owned(),
            Self::Bytes(value) => format!("{value:?}"),
            Self::HostRef(value) => format!(
                "host_ref({}:{}/{})",
                value.type_id.get(),
                value.object_id.get(),
                value.generation
            ),
        }
    }
}

impl<'a> From<&'a HostCollectionKey> for HostCollectionKeyRef<'a> {
    fn from(key: &'a HostCollectionKey) -> Self {
        match key {
            HostCollectionKey::Bool(value) => Self::Bool(*value),
            HostCollectionKey::Char(value) => Self::Char(*value),
            HostCollectionKey::I8(value) => Self::I8(*value),
            HostCollectionKey::I16(value) => Self::I16(*value),
            HostCollectionKey::I32(value) => Self::I32(*value),
            HostCollectionKey::I64(value) => Self::I64(*value),
            HostCollectionKey::U8(value) => Self::U8(*value),
            HostCollectionKey::U16(value) => Self::U16(*value),
            HostCollectionKey::U32(value) => Self::U32(*value),
            HostCollectionKey::U64(value) => Self::U64(*value),
            HostCollectionKey::String(value) => Self::String(value),
            HostCollectionKey::Bytes(value) => Self::Bytes(value),
            HostCollectionKey::HostRef(value) => Self::HostRef(*value),
        }
    }
}

impl HostCollectionQuery {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Len => "len",
            Self::IsEmpty => "is_empty",
        }
    }
}
