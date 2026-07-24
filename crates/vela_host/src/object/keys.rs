use crate::error::HostResult;
use crate::path::HostRef;
use crate::protocol::{HostCollectionKey, HostCollectionKeyRef};

use super::{ScriptHostKey, invalid_arg};

impl ScriptHostKey for String {
    fn script_host_key_shape() -> Option<&'static str> {
        Some("String")
    }

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
                fn script_host_key_shape() -> Option<&'static str> {
                    Some(stringify!($ty))
                }

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
    fn script_host_key_shape() -> Option<&'static str> {
        Some("Bytes")
    }

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
