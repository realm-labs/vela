//! Immutable sparse-update composition for service method selections.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use vela_common::{
    ServiceAbiFingerprint, ServiceGenerationId, ServiceId, ServiceMethodId,
    ServiceSetAbiFingerprint, ServiceSetId,
};

use super::{ServiceSchema, ServiceSetSchema};

/// Stable locator for one method in a generated service set.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServiceMethodKey {
    pub service_id: ServiceId,
    pub method_id: ServiceMethodId,
}

impl ServiceMethodKey {
    #[must_use]
    pub const fn new(service_id: ServiceId, method_id: ServiceMethodId) -> Self {
        Self {
            service_id,
            method_id,
        }
    }
}

/// One fully linked method selection in a flattened generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceMethodSelection<T> {
    RustDefault,
    Vela(T),
}

/// One explicit sparse update claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceMethodUpdate<T> {
    key: ServiceMethodKey,
    expected_service_abi: ServiceAbiFingerprint,
    selection: ServiceMethodSelection<T>,
}

impl<T> ServiceMethodUpdate<T> {
    #[must_use]
    pub const fn new(
        key: ServiceMethodKey,
        expected_service_abi: ServiceAbiFingerprint,
        selection: ServiceMethodSelection<T>,
    ) -> Self {
        Self {
            key,
            expected_service_abi,
            selection,
        }
    }

    #[must_use]
    pub const fn vela(
        service_id: ServiceId,
        method_id: ServiceMethodId,
        expected_service_abi: ServiceAbiFingerprint,
        target: T,
    ) -> Self {
        Self::new(
            ServiceMethodKey::new(service_id, method_id),
            expected_service_abi,
            ServiceMethodSelection::Vela(target),
        )
    }

    #[must_use]
    pub const fn rust_default(
        service_id: ServiceId,
        method_id: ServiceMethodId,
        expected_service_abi: ServiceAbiFingerprint,
    ) -> Self {
        Self::new(
            ServiceMethodKey::new(service_id, method_id),
            expected_service_abi,
            ServiceMethodSelection::RustDefault,
        )
    }

    #[must_use]
    pub const fn key(&self) -> ServiceMethodKey {
        self.key
    }

    #[must_use]
    pub const fn expected_service_abi(&self) -> ServiceAbiFingerprint {
        self.expected_service_abi
    }

    #[must_use]
    pub const fn selection(&self) -> &ServiceMethodSelection<T> {
        &self.selection
    }
}

/// Complete immutable selection table for one service-set schema.
///
/// Every registered service method appears exactly once. Runtime dispatch
/// never consults an older table or interprets absence as fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceSelectionTable<T> {
    service_set_id: ServiceSetId,
    service_set_abi: ServiceSetAbiFingerprint,
    selections: BTreeMap<ServiceMethodKey, ServiceMethodSelection<T>>,
}

impl<T> ServiceSelectionTable<T> {
    /// Composes a complete Snapshot. Unmentioned methods select Rust.
    pub fn snapshot(
        schema: &ServiceSetSchema,
        updates: impl IntoIterator<Item = ServiceMethodUpdate<T>>,
    ) -> Result<Self, ServiceSelectionError> {
        let mut selections = rust_defaults(schema);
        apply_updates(schema, &mut selections, updates)?;
        Ok(Self {
            service_set_id: schema.id(),
            service_set_abi: schema.abi_fingerprint(),
            selections,
        })
    }

    #[must_use]
    pub const fn service_set_id(&self) -> ServiceSetId {
        self.service_set_id
    }

    #[must_use]
    pub const fn service_set_abi(&self) -> ServiceSetAbiFingerprint {
        self.service_set_abi
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.selections.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }

    #[must_use]
    pub fn get(
        &self,
        service_id: ServiceId,
        method_id: ServiceMethodId,
    ) -> Option<&ServiceMethodSelection<T>> {
        self.selections
            .get(&ServiceMethodKey::new(service_id, method_id))
    }

    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (ServiceMethodKey, &ServiceMethodSelection<T>)> + '_ {
        self.selections
            .iter()
            .map(|(key, selection)| (*key, selection))
    }
}

impl<T: Clone> ServiceSelectionTable<T> {
    /// Composes a flattened Delta over one exact base generation.
    ///
    /// Unmentioned methods inherit the base selection. An explicit
    /// [`ServiceMethodSelection::RustDefault`] removes an inherited Vela
    /// implementation.
    pub fn delta(
        schema: &ServiceSetSchema,
        expected_base_generation: ServiceGenerationId,
        actual_base_generation: ServiceGenerationId,
        base: &Self,
        updates: impl IntoIterator<Item = ServiceMethodUpdate<T>>,
    ) -> Result<Self, ServiceSelectionError> {
        if expected_base_generation != actual_base_generation {
            return Err(ServiceSelectionError::BaseGenerationMismatch {
                expected: expected_base_generation,
                actual: actual_base_generation,
            });
        }
        validate_base(schema, base)?;
        let mut selections = base.selections.clone();
        apply_updates(schema, &mut selections, updates)?;
        Ok(Self {
            service_set_id: schema.id(),
            service_set_abi: schema.abi_fingerprint(),
            selections,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceSelectionError {
    BaseGenerationMismatch {
        expected: ServiceGenerationId,
        actual: ServiceGenerationId,
    },
    ForeignServiceSet {
        expected: ServiceSetId,
        actual: ServiceSetId,
    },
    IncompatibleServiceSetSchema {
        expected: ServiceSetAbiFingerprint,
        actual: ServiceSetAbiFingerprint,
    },
    UnknownService {
        service_id: ServiceId,
    },
    UnknownMethod {
        service_id: ServiceId,
        method_id: ServiceMethodId,
    },
    IncompatibleServiceSchema {
        service_id: ServiceId,
        expected: ServiceAbiFingerprint,
        actual: ServiceAbiFingerprint,
    },
    DuplicateMethodUpdate {
        service_id: ServiceId,
        method_id: ServiceMethodId,
    },
}

impl fmt::Display for ServiceSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseGenerationMismatch { expected, actual } => write!(
                formatter,
                "service Delta expects base generation {}, found {}",
                expected.get(),
                actual.get()
            ),
            Self::ForeignServiceSet { expected, actual } => write!(
                formatter,
                "service selection expects set {}, found {}",
                expected.get(),
                actual.get()
            ),
            Self::IncompatibleServiceSetSchema { expected, actual } => write!(
                formatter,
                "service selection expects set ABI {:016x}, found {:016x}",
                expected.get(),
                actual.get()
            ),
            Self::UnknownService { service_id } => {
                write!(formatter, "unknown service ID {}", service_id.get())
            }
            Self::UnknownMethod {
                service_id,
                method_id,
            } => write!(
                formatter,
                "service {} has no method ID {}",
                service_id.get(),
                method_id.get()
            ),
            Self::IncompatibleServiceSchema {
                service_id,
                expected,
                actual,
            } => write!(
                formatter,
                "service {} expects ABI {:016x}, update declares {:016x}",
                service_id.get(),
                expected.get(),
                actual.get()
            ),
            Self::DuplicateMethodUpdate {
                service_id,
                method_id,
            } => write!(
                formatter,
                "service {} method {} is updated more than once",
                service_id.get(),
                method_id.get()
            ),
        }
    }
}

impl std::error::Error for ServiceSelectionError {}

fn rust_defaults<T>(
    schema: &ServiceSetSchema,
) -> BTreeMap<ServiceMethodKey, ServiceMethodSelection<T>> {
    schema
        .services()
        .iter()
        .flat_map(|service| {
            service.methods().iter().map(|method| {
                (
                    ServiceMethodKey::new(service.id(), method.id),
                    ServiceMethodSelection::RustDefault,
                )
            })
        })
        .collect()
}

fn apply_updates<T>(
    schema: &ServiceSetSchema,
    selections: &mut BTreeMap<ServiceMethodKey, ServiceMethodSelection<T>>,
    updates: impl IntoIterator<Item = ServiceMethodUpdate<T>>,
) -> Result<(), ServiceSelectionError> {
    let services = schema
        .services()
        .iter()
        .map(|service| (service.id(), service))
        .collect::<BTreeMap<_, _>>();
    let mut claimed = BTreeSet::new();
    for update in updates {
        let key = update.key;
        if !claimed.insert(key) {
            return Err(ServiceSelectionError::DuplicateMethodUpdate {
                service_id: key.service_id,
                method_id: key.method_id,
            });
        }
        let service =
            services
                .get(&key.service_id)
                .ok_or(ServiceSelectionError::UnknownService {
                    service_id: key.service_id,
                })?;
        validate_service_abi(service, update.expected_service_abi)?;
        if !service
            .methods()
            .iter()
            .any(|method| method.id == key.method_id)
        {
            return Err(ServiceSelectionError::UnknownMethod {
                service_id: key.service_id,
                method_id: key.method_id,
            });
        }
        selections.insert(key, update.selection);
    }
    Ok(())
}

fn validate_base<T>(
    schema: &ServiceSetSchema,
    base: &ServiceSelectionTable<T>,
) -> Result<(), ServiceSelectionError> {
    if base.service_set_id != schema.id() {
        return Err(ServiceSelectionError::ForeignServiceSet {
            expected: schema.id(),
            actual: base.service_set_id,
        });
    }
    if base.service_set_abi != schema.abi_fingerprint() {
        return Err(ServiceSelectionError::IncompatibleServiceSetSchema {
            expected: schema.abi_fingerprint(),
            actual: base.service_set_abi,
        });
    }
    Ok(())
}

fn validate_service_abi(
    service: &ServiceSchema,
    actual: ServiceAbiFingerprint,
) -> Result<(), ServiceSelectionError> {
    let expected = service.abi_fingerprint();
    if actual == expected {
        return Ok(());
    }
    Err(ServiceSelectionError::IncompatibleServiceSchema {
        service_id: service.id(),
        expected,
        actual,
    })
}
