//! Immutable Rust/Vela value-binding facts exposed to compilation and tooling.

use vela_common::{InteropTypeId, TypeAbiFingerprint, TypeBindingRegistryChecksum};

use crate::registry::TypeKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeBindingDesc {
    pub id: InteropTypeId,
    pub key: TypeKey,
    pub abi_fingerprint: TypeAbiFingerprint,
}

impl TypeBindingDesc {
    #[must_use]
    pub const fn new(id: InteropTypeId, key: TypeKey, abi_fingerprint: TypeAbiFingerprint) -> Self {
        Self {
            id,
            key,
            abi_fingerprint,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeBindingSnapshot {
    checksum: TypeBindingRegistryChecksum,
}

impl TypeBindingSnapshot {
    #[must_use]
    pub const fn new(checksum: TypeBindingRegistryChecksum) -> Self {
        Self { checksum }
    }

    #[must_use]
    pub const fn checksum(self) -> TypeBindingRegistryChecksum {
        self.checksum
    }
}
