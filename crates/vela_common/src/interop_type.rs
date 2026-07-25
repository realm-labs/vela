//! Stable identities and representation capabilities for Rust/Vela type bindings.

use vela_def::TypeId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct InteropTypeId(u128);

impl InteropTypeId {
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn from_type_id(type_id: TypeId) -> Self {
        Self(type_id.get())
    }

    #[must_use]
    pub const fn get(self) -> u128 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct TypeAbiFingerprint(u64);

impl TypeAbiFingerprint {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct TypeBindingRegistryChecksum(u64);

impl TypeBindingRegistryChecksum {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StoragePolicy {
    Value,
    Host,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostConstructionLifetime {
    CallScoped,
    RuntimeOwned,
}

impl HostConstructionLifetime {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CallScoped => "call_scoped",
            Self::RuntimeOwned => "runtime_owned",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostConstructorBinding {
    pub id: vela_def::FunctionId,
    pub lifetime: HostConstructionLifetime,
}

impl HostConstructorBinding {
    #[must_use]
    pub const fn new(id: vela_def::FunctionId, lifetime: HostConstructionLifetime) -> Self {
        Self { id, lifetime }
    }
}

/// The script-visible collection protocol carried by a borrowed Rust view.
///
/// These variants correspond to Vela's restricted `ArrayView`, `MapView`, and
/// `SetView` families. They describe a representation of one registered Rust
/// type; they do not create a second interop type identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CollectionViewKind {
    Array,
    Map,
    Set,
}

impl CollectionViewKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Array => "array",
            Self::Map => "map",
            Self::Set => "set",
        }
    }
}

/// Whether an exclusive collection view may change collection length.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CollectionViewMutation {
    Fixed,
    Growable,
}

impl CollectionViewMutation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Growable => "growable",
        }
    }
}

/// Borrowed collection representations supported by one `TypeBinding`.
///
/// Presence always implies a shared read-only view. `mutation` additionally
/// advertises an exclusive write-through view and records whether structural
/// growth is legal. An exclusive view can always be reborrowed as shared.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CollectionViewCapabilities {
    kind: CollectionViewKind,
    mutation: Option<CollectionViewMutation>,
}

/// The concrete representation selected when one registered Rust type crosses
/// a callable boundary.
///
/// A representation never creates a second type identity. In particular,
/// owned `Vec<T>`, `&Vec<T>`, and `&mut Vec<T>` retain one `InteropTypeId` and
/// select `Owned`, `CollectionView`, or `CollectionMut` respectively.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InteropRepresentation {
    Owned,
    SharedHost,
    ExclusiveHost,
    CollectionView(CollectionViewKind),
    CollectionMut {
        kind: CollectionViewKind,
        mutation: CollectionViewMutation,
    },
}

impl InteropRepresentation {
    #[must_use]
    pub const fn abi_name(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::SharedHost => "shared_host",
            Self::ExclusiveHost => "exclusive_host",
            Self::CollectionView(CollectionViewKind::Array) => "array_view",
            Self::CollectionView(CollectionViewKind::Map) => "map_view",
            Self::CollectionView(CollectionViewKind::Set) => "set_view",
            Self::CollectionMut {
                kind: CollectionViewKind::Array,
                mutation: CollectionViewMutation::Fixed,
            } => "array_mut_fixed",
            Self::CollectionMut {
                kind: CollectionViewKind::Array,
                mutation: CollectionViewMutation::Growable,
            } => "array_mut_growable",
            Self::CollectionMut {
                kind: CollectionViewKind::Map,
                mutation: CollectionViewMutation::Fixed,
            } => "map_mut_fixed",
            Self::CollectionMut {
                kind: CollectionViewKind::Map,
                mutation: CollectionViewMutation::Growable,
            } => "map_mut_growable",
            Self::CollectionMut {
                kind: CollectionViewKind::Set,
                mutation: CollectionViewMutation::Fixed,
            } => "set_mut_fixed",
            Self::CollectionMut {
                kind: CollectionViewKind::Set,
                mutation: CollectionViewMutation::Growable,
            } => "set_mut_growable",
        }
    }
}

/// Exact type-binding proof carried by callable ABI.
///
/// The surface type hint answers what Vela code may do. This proof separately
/// answers which concrete Rust binding and representation the adapter expects.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InteropBindingContract {
    pub type_id: InteropTypeId,
    pub representation: InteropRepresentation,
    pub abi_fingerprint: TypeAbiFingerprint,
}

impl InteropBindingContract {
    #[must_use]
    pub const fn new(
        type_id: InteropTypeId,
        representation: InteropRepresentation,
        abi_fingerprint: TypeAbiFingerprint,
    ) -> Self {
        Self {
            type_id,
            representation,
            abi_fingerprint,
        }
    }
}

impl CollectionViewCapabilities {
    #[must_use]
    pub const fn read_only(kind: CollectionViewKind) -> Self {
        Self {
            kind,
            mutation: None,
        }
    }

    #[must_use]
    pub const fn mutable(kind: CollectionViewKind, mutation: CollectionViewMutation) -> Self {
        Self {
            kind,
            mutation: Some(mutation),
        }
    }

    #[must_use]
    pub const fn kind(self) -> CollectionViewKind {
        self.kind
    }

    #[must_use]
    pub const fn mutation(self) -> Option<CollectionViewMutation> {
        self.mutation
    }
}

impl StoragePolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Host => "host",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ReceiverCapability {
    Owned = 1 << 0,
    Shared = 1 << 1,
    Exclusive = 1 << 2,
    Construct = 1 << 3,
}

impl ReceiverCapability {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::Shared => "shared",
            Self::Exclusive => "exclusive",
            Self::Construct => "construct",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ReceiverCapabilities(u8);

impl ReceiverCapabilities {
    pub const NONE: Self = Self(0);
    pub const OWNED: Self = Self(ReceiverCapability::Owned as u8);
    pub const SHARED: Self = Self(ReceiverCapability::Shared as u8);
    pub const EXCLUSIVE: Self = Self(ReceiverCapability::Exclusive as u8);
    pub const CONSTRUCT: Self = Self(ReceiverCapability::Construct as u8);
    pub const OWNED_VALUE: Self = Self(
        ReceiverCapability::Owned as u8
            | ReceiverCapability::Shared as u8
            | ReceiverCapability::Exclusive as u8,
    );
    pub const HOST_OBJECT: Self = Self(
        ReceiverCapability::Owned as u8
            | ReceiverCapability::Shared as u8
            | ReceiverCapability::Exclusive as u8,
    );

    #[must_use]
    pub const fn with(self, capability: ReceiverCapability) -> Self {
        Self(self.0 | capability as u8)
    }

    #[must_use]
    pub const fn contains(self, capability: ReceiverCapability) -> bool {
        self.0 & capability as u8 != 0
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CollectionViewCapabilities, CollectionViewKind, CollectionViewMutation,
        ReceiverCapabilities, ReceiverCapability,
    };

    #[test]
    fn receiver_capabilities_are_composable_without_implying_construct() {
        let capabilities = ReceiverCapabilities::OWNED_VALUE;
        assert!(capabilities.contains(ReceiverCapability::Owned));
        assert!(capabilities.contains(ReceiverCapability::Shared));
        assert!(capabilities.contains(ReceiverCapability::Exclusive));
        assert!(!capabilities.contains(ReceiverCapability::Construct));
        assert!(
            capabilities
                .with(ReceiverCapability::Construct)
                .contains(ReceiverCapability::Construct)
        );
    }

    #[test]
    fn collection_views_keep_shared_and_structural_mutation_facts_separate() {
        let shared = CollectionViewCapabilities::read_only(CollectionViewKind::Array);
        assert_eq!(shared.kind(), CollectionViewKind::Array);
        assert_eq!(shared.mutation(), None);

        let mutable = CollectionViewCapabilities::mutable(
            CollectionViewKind::Array,
            CollectionViewMutation::Fixed,
        );
        assert_eq!(mutable.kind(), CollectionViewKind::Array);
        assert_eq!(mutable.mutation(), Some(CollectionViewMutation::Fixed));
    }
}
