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
    use super::{ReceiverCapabilities, ReceiverCapability};

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
}
