//! Immutable Rust/Vela type-binding facts exposed to compilation and tooling.

use vela_common::{
    CollectionViewCapabilities, InteropRepresentation, InteropTypeId, ReceiverCapabilities,
    ReceiverCapability, StoragePolicy, TypeAbiFingerprint, TypeBindingRegistryChecksum,
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

    #[must_use]
    pub fn supports_representation(&self, representation: InteropRepresentation) -> bool {
        match representation {
            InteropRepresentation::Owned => self.capabilities.contains(ReceiverCapability::Owned),
            InteropRepresentation::SharedHost => {
                self.collection_views.is_none()
                    && self.capabilities.contains(ReceiverCapability::Shared)
            }
            InteropRepresentation::ExclusiveHost => {
                self.collection_views.is_none()
                    && self.capabilities.contains(ReceiverCapability::Exclusive)
            }
            InteropRepresentation::CollectionView(kind) => self
                .collection_views
                .is_some_and(|views| views.kind() == kind),
            InteropRepresentation::CollectionMut { kind, mutation } => self
                .collection_views
                .is_some_and(|views| views.kind() == kind && views.mutation() == Some(mutation)),
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
