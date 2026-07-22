//! Immutable Rust/Vela type-binding facts exposed to compilation and tooling.

use vela_common::{
    CollectionViewCapabilities, InteropTypeId, ReceiverCapabilities, StoragePolicy,
    TypeAbiFingerprint, TypeBindingRegistryChecksum,
};
use vela_def::FunctionId;

use crate::registry::TypeKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeBindingDesc {
    pub id: InteropTypeId,
    pub key: TypeKey,
    pub storage: StoragePolicy,
    pub capabilities: ReceiverCapabilities,
    pub collection_views: Option<CollectionViewCapabilities>,
    pub constructor_ids: Vec<FunctionId>,
    pub abi_fingerprint: TypeAbiFingerprint,
}

impl TypeBindingDesc {
    #[must_use]
    pub fn new(
        id: InteropTypeId,
        key: TypeKey,
        storage: StoragePolicy,
        capabilities: ReceiverCapabilities,
        collection_views: Option<CollectionViewCapabilities>,
        constructor_ids: Vec<FunctionId>,
        abi_fingerprint: TypeAbiFingerprint,
    ) -> Self {
        Self {
            id,
            key,
            storage,
            capabilities,
            collection_views,
            constructor_ids,
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
