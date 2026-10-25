//! Stable identities and ABI proofs for owned Rust/Vela value bindings.

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

/// Exact concrete value-binding proof carried by callable ABI.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InteropBindingContract {
    pub type_id: InteropTypeId,
    pub abi_fingerprint: TypeAbiFingerprint,
}

impl InteropBindingContract {
    #[must_use]
    pub const fn new(type_id: InteropTypeId, abi_fingerprint: TypeAbiFingerprint) -> Self {
        Self {
            type_id,
            abi_fingerprint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_binding_proof_preserves_exact_identity_and_abi() {
        let id = InteropTypeId::from_type_id(TypeId::new(u128::MAX));
        let abi = TypeAbiFingerprint::new(17);
        let proof = InteropBindingContract::new(id, abi);
        assert_eq!(proof.type_id.get(), u128::MAX);
        assert_eq!(proof.abi_fingerprint.get(), 17);
        assert_ne!(
            proof,
            InteropBindingContract::new(id, TypeAbiFingerprint::new(18))
        );
        assert_eq!(TypeBindingRegistryChecksum::new(23).get(), 23);
    }
}
