use vela_common::{InteropBindingContract, InteropRepresentation, InteropTypeId};

use super::ServiceSchemaError;
use crate::type_binding::TypeBindingRegistry;

/// One exact Rust type representation reachable from a service boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceTypeRequirement {
    pub(super) location: String,
    pub(super) contract: InteropBindingContract,
}

impl ServiceTypeRequirement {
    /// Resolves a concrete Rust type against the sealed registry while the
    /// generated service schema is being built.
    pub fn for_rust_type<T: 'static>(
        registry: &TypeBindingRegistry,
        location: impl Into<String>,
        representation: InteropRepresentation,
    ) -> Result<Self, ServiceSchemaError> {
        let location = location.into();
        let Some(binding) = registry.get_for::<T>() else {
            return Err(ServiceSchemaError::MissingRustTypeBinding {
                location,
                rust_type: std::any::type_name::<T>(),
            });
        };
        if !binding.supports_representation(representation) {
            return Err(ServiceSchemaError::UnsupportedTypeRepresentation {
                location,
                rust_type: std::any::type_name::<T>(),
                representation,
            });
        }
        let contract =
            InteropBindingContract::new(binding.id, representation, binding.abi_fingerprint);
        Ok(Self { location, contract })
    }

    /// Resolves a Host contract by its stable schema identity without asking
    /// the registry for a concrete Rust `TypeId`.
    pub fn for_host_type<T>(
        registry: &TypeBindingRegistry,
        location: impl Into<String>,
        representation: InteropRepresentation,
    ) -> Result<Self, ServiceSchemaError>
    where
        T: crate::interop::VelaHostBoundary,
    {
        let location = location.into();
        let type_desc = T::script_host_type_desc();
        let type_id = InteropTypeId::from_type_id(type_desc.key.id);
        let Some(binding) = registry
            .get(type_id)
            .filter(|binding| binding.key == type_desc.key)
        else {
            return Err(ServiceSchemaError::MissingHostTypeBinding {
                location,
                type_name: type_desc.key.name,
                type_id,
            });
        };
        if !binding.supports_representation(representation) {
            return Err(ServiceSchemaError::UnsupportedHostTypeRepresentation {
                location,
                type_name: type_desc.key.name,
                type_id,
                representation,
            });
        }
        Ok(Self {
            location,
            contract: InteropBindingContract::new(
                binding.id,
                representation,
                binding.abi_fingerprint,
            ),
        })
    }

    #[must_use]
    pub fn from_contract(location: impl Into<String>, contract: InteropBindingContract) -> Self {
        Self {
            location: location.into(),
            contract,
        }
    }

    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }

    #[must_use]
    pub const fn contract(&self) -> InteropBindingContract {
        self.contract
    }
}
