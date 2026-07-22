//! Immutable Rust/Vela type-binding facts exposed to compilation and tooling.

use vela_common::{
    InteropTypeId, ReceiverCapabilities, StoragePolicy, TypeAbiFingerprint,
    TypeBindingRegistryChecksum,
};

use crate::registry::TypeKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeBindingDesc {
    pub id: InteropTypeId,
    pub key: TypeKey,
    pub storage: StoragePolicy,
    pub capabilities: ReceiverCapabilities,
    pub abi_fingerprint: TypeAbiFingerprint,
}

impl TypeBindingDesc {
    #[must_use]
    pub const fn new(
        id: InteropTypeId,
        key: TypeKey,
        storage: StoragePolicy,
        capabilities: ReceiverCapabilities,
        abi_fingerprint: TypeAbiFingerprint,
    ) -> Self {
        Self {
            id,
            key,
            storage,
            capabilities,
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
