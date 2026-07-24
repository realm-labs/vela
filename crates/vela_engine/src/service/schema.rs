//! Immutable service-contract schemas over one sealed type-binding registry.

use std::collections::BTreeSet;
use std::fmt;

use vela_common::{
    InteropBindingContract, InteropRepresentation, InteropTypeId, ServiceAbiFingerprint, ServiceId,
    ServiceMethodId, ServiceSetAbiFingerprint, ServiceSetId, TypeBindingRegistryChecksum,
    stable_id,
};

use crate::interop::{BoundaryMode, CallableContract, CallableKind, ReturnMode};
use crate::native::TypeHint;
use crate::type_binding::TypeBindingRegistry;

#[doc(hidden)]
pub type ServiceSetSchemaFactory =
    fn(&TypeBindingRegistry) -> Result<ServiceSetSchema, ServiceSchemaError>;

/// One exact Rust type representation reachable from a service boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceTypeRequirement {
    location: String,
    contract: InteropBindingContract,
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

/// One generated service method and its complete boundary type closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceMethodDescriptor {
    pub id: ServiceMethodId,
    pub path: String,
    pub callable: CallableContract,
    pub type_closure: Vec<ServiceTypeRequirement>,
}

impl ServiceMethodDescriptor {
    #[must_use]
    pub fn new(
        id: ServiceMethodId,
        path: impl Into<String>,
        callable: CallableContract,
        type_closure: Vec<ServiceTypeRequirement>,
    ) -> Self {
        Self {
            id,
            path: path.into(),
            callable,
            type_closure,
        }
    }
}

/// Immutable contract for one Rust-authored service trait.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceSchema {
    id: ServiceId,
    path: String,
    methods: Vec<ServiceMethodDescriptor>,
    abi_fingerprint: ServiceAbiFingerprint,
    type_binding_checksum: TypeBindingRegistryChecksum,
}

impl ServiceSchema {
    pub fn new(
        id: ServiceId,
        path: impl Into<String>,
        mut methods: Vec<ServiceMethodDescriptor>,
        registry: &TypeBindingRegistry,
    ) -> Result<Self, ServiceSchemaError> {
        let path = path.into();
        validate_qualified_path(&path, ServicePathKind::Service)?;
        let mut method_ids = BTreeSet::new();
        let mut method_paths = BTreeSet::new();
        for method in &mut methods {
            complete_transitive_type_closure(method, registry)?;
            if !method_ids.insert(method.id) {
                return Err(ServiceSchemaError::DuplicateMethodId {
                    service: path,
                    method_id: method.id,
                });
            }
            if !method_paths.insert(method.path.as_str()) {
                return Err(ServiceSchemaError::DuplicateMethodPath {
                    service: path,
                    method_path: method.path.clone(),
                });
            }
            validate_method(&path, method, registry)?;
        }
        let abi_fingerprint = service_abi_fingerprint(id, &path, &methods);
        Ok(Self {
            id,
            path,
            methods,
            abi_fingerprint,
            type_binding_checksum: registry.checksum(),
        })
    }

    #[must_use]
    pub const fn id(&self) -> ServiceId {
        self.id
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn methods(&self) -> &[ServiceMethodDescriptor] {
        &self.methods
    }

    #[must_use]
    pub const fn abi_fingerprint(&self) -> ServiceAbiFingerprint {
        self.abi_fingerprint
    }

    #[must_use]
    pub const fn type_binding_checksum(&self) -> TypeBindingRegistryChecksum {
        self.type_binding_checksum
    }

    pub fn validate_type_bindings(
        &self,
        registry: &TypeBindingRegistry,
    ) -> Result<(), ServiceSchemaError> {
        if self.type_binding_checksum != registry.checksum() {
            return Err(ServiceSchemaError::TypeBindingRegistryChanged {
                expected: self.type_binding_checksum,
                actual: registry.checksum(),
            });
        }
        for method in &self.methods {
            validate_method(&self.path, method, registry)?;
        }
        Ok(())
    }
}

/// Immutable manifest for one generated whole service set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceSetSchema {
    id: ServiceSetId,
    path: String,
    services: Vec<ServiceSchema>,
    abi_fingerprint: ServiceSetAbiFingerprint,
    type_binding_checksum: TypeBindingRegistryChecksum,
}

impl ServiceSetSchema {
    pub fn new(
        id: ServiceSetId,
        path: impl Into<String>,
        services: Vec<ServiceSchema>,
        registry: &TypeBindingRegistry,
    ) -> Result<Self, ServiceSchemaError> {
        let path = path.into();
        validate_qualified_path(&path, ServicePathKind::ServiceSet)?;
        let mut service_ids = BTreeSet::new();
        let mut service_paths = BTreeSet::new();
        for service in &services {
            if !service_ids.insert(service.id()) {
                return Err(ServiceSchemaError::DuplicateServiceId {
                    service_set: path,
                    service_id: service.id(),
                });
            }
            if !service_paths.insert(service.path()) {
                return Err(ServiceSchemaError::DuplicateServicePath {
                    service_set: path,
                    service_path: service.path().to_owned(),
                });
            }
            service.validate_type_bindings(registry)?;
        }
        let abi_fingerprint = service_set_abi_fingerprint(id, &path, &services);
        Ok(Self {
            id,
            path,
            services,
            abi_fingerprint,
            type_binding_checksum: registry.checksum(),
        })
    }

    #[must_use]
    pub const fn id(&self) -> ServiceSetId {
        self.id
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn services(&self) -> &[ServiceSchema] {
        &self.services
    }

    #[must_use]
    pub const fn abi_fingerprint(&self) -> ServiceSetAbiFingerprint {
        self.abi_fingerprint
    }

    #[must_use]
    pub const fn type_binding_checksum(&self) -> TypeBindingRegistryChecksum {
        self.type_binding_checksum
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceSchemaError {
    InvalidServicePath(String),
    InvalidServiceSetPath(String),
    InvalidMethodPath {
        service: String,
        method_path: String,
    },
    DuplicateMethodId {
        service: String,
        method_id: ServiceMethodId,
    },
    DuplicateMethodPath {
        service: String,
        method_path: String,
    },
    DuplicateServiceId {
        service_set: String,
        service_id: ServiceId,
    },
    DuplicateServicePath {
        service_set: String,
        service_path: String,
    },
    InvalidCallableKind {
        method: String,
        actual: CallableKind,
    },
    CallableIdentityMismatch {
        method: String,
    },
    MissingCallableBinding {
        method: String,
        location: String,
    },
    MissingTypeClosureEntry {
        method: String,
        location: String,
        type_id: InteropTypeId,
    },
    MissingTransitiveTypeBinding {
        method: String,
        location: String,
        type_id: InteropTypeId,
        type_name: String,
    },
    UnsupportedTransitiveTypeBinding {
        method: String,
        location: String,
        type_id: InteropTypeId,
        type_name: String,
    },
    MissingRustTypeBinding {
        location: String,
        rust_type: &'static str,
    },
    UnsupportedTypeRepresentation {
        location: String,
        rust_type: &'static str,
        representation: InteropRepresentation,
    },
    InvalidTypeBinding {
        method: String,
        location: String,
    },
    UnsupportedBoundaryType {
        method: String,
        location: String,
    },
    TypeBindingRegistryChanged {
        expected: TypeBindingRegistryChecksum,
        actual: TypeBindingRegistryChecksum,
    },
}

impl fmt::Display for ServiceSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidServicePath(path) => write!(formatter, "invalid service path {path}"),
            Self::InvalidServiceSetPath(path) => {
                write!(formatter, "invalid service-set path {path}")
            }
            Self::InvalidMethodPath {
                service,
                method_path,
            } => write!(
                formatter,
                "service method path {method_path} is not a child of {service}"
            ),
            Self::DuplicateMethodId { service, method_id } => write!(
                formatter,
                "service {service} has duplicate method ID {}",
                method_id.get()
            ),
            Self::DuplicateMethodPath {
                service,
                method_path,
            } => write!(
                formatter,
                "service {service} has duplicate method path {method_path}"
            ),
            Self::DuplicateServiceId {
                service_set,
                service_id,
            } => write!(
                formatter,
                "service set {service_set} has duplicate service ID {}",
                service_id.get()
            ),
            Self::DuplicateServicePath {
                service_set,
                service_path,
            } => write!(
                formatter,
                "service set {service_set} has duplicate service path {service_path}"
            ),
            Self::InvalidCallableKind { method, actual } => {
                write!(
                    formatter,
                    "service method {method} has callable kind {actual:?}"
                )
            }
            Self::CallableIdentityMismatch { method } => {
                write!(
                    formatter,
                    "service method {method} has a mismatched callable identity"
                )
            }
            Self::MissingCallableBinding { method, location } => write!(
                formatter,
                "service method {method} is missing an exact binding for {location}"
            ),
            Self::MissingTypeClosureEntry {
                method,
                location,
                type_id,
            } => write!(
                formatter,
                "service method {method} omits {location} type {} from its closure",
                type_id.get()
            ),
            Self::MissingTransitiveTypeBinding {
                method,
                location,
                type_id,
                type_name,
            } => write!(
                formatter,
                "service method {method} has no registered binding for transitive {location} type {type_name} ({})",
                type_id.get()
            ),
            Self::UnsupportedTransitiveTypeBinding {
                method,
                location,
                type_id,
                type_name,
            } => write!(
                formatter,
                "service method {method} transitive {location} type {type_name} ({}) has no owned representation",
                type_id.get()
            ),
            Self::MissingRustTypeBinding {
                location,
                rust_type,
            } => write!(
                formatter,
                "service boundary {location} has no binding for Rust type {rust_type}"
            ),
            Self::UnsupportedTypeRepresentation {
                location,
                rust_type,
                representation,
            } => write!(
                formatter,
                "service boundary {location} cannot use {rust_type} as {}",
                representation.abi_name()
            ),
            Self::InvalidTypeBinding { method, location } => write!(
                formatter,
                "service method {method} has an invalid binding for {location}"
            ),
            Self::UnsupportedBoundaryType { method, location } => write!(
                formatter,
                "service method {method} uses an incomplete boundary type at {location}"
            ),
            Self::TypeBindingRegistryChanged { expected, actual } => write!(
                formatter,
                "service schema expects type-binding registry {:016x}, found {:016x}",
                expected.get(),
                actual.get()
            ),
        }
    }
}

impl std::error::Error for ServiceSchemaError {}

fn complete_transitive_type_closure(
    method: &mut ServiceMethodDescriptor,
    registry: &TypeBindingRegistry,
) -> Result<(), ServiceSchemaError> {
    let mut additions = Vec::new();
    let mut known = method
        .type_closure
        .iter()
        .map(|requirement| requirement.contract.type_id)
        .collect::<BTreeSet<_>>();
    for parameter in &method.callable.parameters {
        if parameter.mode != BoundaryMode::HiddenContext {
            collect_transitive_type_requirements(
                method.path.as_str(),
                &format!("parameter {}", parameter.name),
                &parameter.ty,
                registry,
                &mut known,
                &mut additions,
            )?;
        }
    }
    collect_transitive_type_requirements(
        method.path.as_str(),
        "return",
        &method.callable.returns.ty,
        registry,
        &mut known,
        &mut additions,
    )?;
    method.type_closure.extend(additions);
    Ok(())
}

fn collect_transitive_type_requirements(
    method: &str,
    location: &str,
    hint: &TypeHint,
    registry: &TypeBindingRegistry,
    known: &mut BTreeSet<InteropTypeId>,
    additions: &mut Vec<ServiceTypeRequirement>,
) -> Result<(), ServiceSchemaError> {
    match hint {
        TypeHint::Record(key) | TypeHint::Enum(key) => {
            let type_id = InteropTypeId::from_type_id(key.id);
            if !known.insert(type_id) {
                return Ok(());
            }
            let Some(binding) = registry.get(type_id).filter(|binding| binding.key == *key) else {
                return Err(ServiceSchemaError::MissingTransitiveTypeBinding {
                    method: method.to_owned(),
                    location: location.to_owned(),
                    type_id,
                    type_name: key.name.clone(),
                });
            };
            if !binding.supports_representation(InteropRepresentation::Owned) {
                return Err(ServiceSchemaError::UnsupportedTransitiveTypeBinding {
                    method: method.to_owned(),
                    location: location.to_owned(),
                    type_id,
                    type_name: key.name.clone(),
                });
            }
            additions.push(ServiceTypeRequirement::from_contract(
                format!("{location} -> {}", key.name),
                InteropBindingContract::new(
                    binding.id,
                    InteropRepresentation::Owned,
                    binding.abi_fingerprint,
                ),
            ));
            Ok(())
        }
        TypeHint::ArrayOf(element)
        | TypeHint::ArrayViewOf(element)
        | TypeHint::ArrayMutOf { element, .. }
        | TypeHint::SetOf(element)
        | TypeHint::SetViewOf(element)
        | TypeHint::SetMutOf { element, .. }
        | TypeHint::IteratorOf(element)
        | TypeHint::OptionOf(element) => collect_transitive_type_requirements(
            method, location, element, registry, known, additions,
        ),
        TypeHint::MapOf { key, value }
        | TypeHint::MapViewOf { key, value }
        | TypeHint::MapMutOf { key, value, .. }
        | TypeHint::ResultOf {
            ok: key,
            err: value,
        } => {
            collect_transitive_type_requirements(
                method, location, key, registry, known, additions,
            )?;
            collect_transitive_type_requirements(
                method, location, value, registry, known, additions,
            )
        }
        TypeHint::TupleOf(elements) => {
            for element in elements {
                collect_transitive_type_requirements(
                    method, location, element, registry, known, additions,
                )?;
            }
            Ok(())
        }
        TypeHint::Any
        | TypeHint::Primitive(_)
        | TypeHint::Array
        | TypeHint::Map
        | TypeHint::Set
        | TypeHint::Iterator
        | TypeHint::PathProxy
        | TypeHint::Host(_)
        | TypeHint::Trait(_)
        | TypeHint::Function => Ok(()),
    }
}

fn validate_method(
    service_path: &str,
    method: &ServiceMethodDescriptor,
    registry: &TypeBindingRegistry,
) -> Result<(), ServiceSchemaError> {
    let expected_prefix = format!("{service_path}::");
    if !method.path.starts_with(&expected_prefix)
        || !is_simple_identifier(&method.path[expected_prefix.len()..])
    {
        return Err(ServiceSchemaError::InvalidMethodPath {
            service: service_path.to_owned(),
            method_path: method.path.clone(),
        });
    }
    if method.callable.identity.kind != CallableKind::RustTraitMethod {
        return Err(ServiceSchemaError::InvalidCallableKind {
            method: method.path.clone(),
            actual: method.callable.identity.kind,
        });
    }
    if method.callable.public_path != method.path
        || method.callable.identity.stable != method.id.get()
    {
        return Err(ServiceSchemaError::CallableIdentityMismatch {
            method: method.path.clone(),
        });
    }
    for requirement in &method.type_closure {
        if !registry.matches_contract(requirement.contract) {
            return Err(ServiceSchemaError::InvalidTypeBinding {
                method: method.path.clone(),
                location: requirement.location.clone(),
            });
        }
    }
    for parameter in &method.callable.parameters {
        if parameter.mode == BoundaryMode::HiddenContext {
            continue;
        }
        let location = format!("parameter {}", parameter.name);
        let binding =
            parameter
                .binding
                .ok_or_else(|| ServiceSchemaError::MissingCallableBinding {
                    method: method.path.clone(),
                    location: location.clone(),
                })?;
        if !parameter_mode_matches_representation(parameter.mode, binding.representation)
            || !registry.matches_contract(binding)
        {
            return Err(ServiceSchemaError::InvalidTypeBinding {
                method: method.path.clone(),
                location,
            });
        }
        require_in_closure(method, &location, binding)?;
        validate_type_hint(method, &location, &parameter.ty)?;
    }
    let return_location = "return".to_owned();
    let return_binding = method.callable.returns.binding.ok_or_else(|| {
        ServiceSchemaError::MissingCallableBinding {
            method: method.path.clone(),
            location: return_location.clone(),
        }
    })?;
    if !return_mode_matches_representation(
        method.callable.returns.mode,
        return_binding.representation,
    ) || !registry.matches_contract(return_binding)
    {
        return Err(ServiceSchemaError::InvalidTypeBinding {
            method: method.path.clone(),
            location: return_location,
        });
    }
    require_in_closure(method, "return", return_binding)?;
    validate_type_hint(method, "return", &method.callable.returns.ty)
}

fn require_in_closure(
    method: &ServiceMethodDescriptor,
    location: &str,
    binding: InteropBindingContract,
) -> Result<(), ServiceSchemaError> {
    if method
        .type_closure
        .iter()
        .any(|requirement| requirement.contract == binding)
    {
        Ok(())
    } else {
        Err(ServiceSchemaError::MissingTypeClosureEntry {
            method: method.path.clone(),
            location: location.to_owned(),
            type_id: binding.type_id,
        })
    }
}

fn validate_type_hint(
    method: &ServiceMethodDescriptor,
    location: &str,
    hint: &TypeHint,
) -> Result<(), ServiceSchemaError> {
    match hint {
        TypeHint::Any
        | TypeHint::Array
        | TypeHint::Map
        | TypeHint::Set
        | TypeHint::Iterator
        | TypeHint::PathProxy
        | TypeHint::Trait(_)
        | TypeHint::Function => Err(ServiceSchemaError::UnsupportedBoundaryType {
            method: method.path.clone(),
            location: location.to_owned(),
        }),
        TypeHint::Primitive(_) => Ok(()),
        TypeHint::Record(key) | TypeHint::Enum(key) | TypeHint::Host(key) => {
            let type_id = InteropTypeId::from_type_id(key.id);
            if method
                .type_closure
                .iter()
                .any(|requirement| requirement.contract.type_id == type_id)
            {
                Ok(())
            } else {
                Err(ServiceSchemaError::MissingTypeClosureEntry {
                    method: method.path.clone(),
                    location: location.to_owned(),
                    type_id,
                })
            }
        }
        TypeHint::ArrayOf(element)
        | TypeHint::ArrayViewOf(element)
        | TypeHint::ArrayMutOf { element, .. }
        | TypeHint::SetOf(element)
        | TypeHint::SetViewOf(element)
        | TypeHint::SetMutOf { element, .. }
        | TypeHint::IteratorOf(element)
        | TypeHint::OptionOf(element) => validate_type_hint(method, location, element),
        TypeHint::MapOf { key, value }
        | TypeHint::MapViewOf { key, value }
        | TypeHint::MapMutOf { key, value, .. }
        | TypeHint::ResultOf {
            ok: key,
            err: value,
        } => {
            validate_type_hint(method, location, key)?;
            validate_type_hint(method, location, value)
        }
        TypeHint::TupleOf(elements) => {
            for element in elements {
                validate_type_hint(method, location, element)?;
            }
            Ok(())
        }
    }
}

fn parameter_mode_matches_representation(
    mode: BoundaryMode,
    representation: InteropRepresentation,
) -> bool {
    match representation {
        InteropRepresentation::Owned => {
            matches!(
                mode,
                BoundaryMode::Value | BoundaryMode::ReadOnlyValueBorrow
            )
        }
        InteropRepresentation::SharedHost | InteropRepresentation::CollectionView(_) => {
            mode == BoundaryMode::SharedHost
        }
        InteropRepresentation::ExclusiveHost | InteropRepresentation::CollectionMut { .. } => {
            mode == BoundaryMode::ExclusiveHost
        }
    }
}

fn return_mode_matches_representation(
    mode: ReturnMode,
    representation: InteropRepresentation,
) -> bool {
    match representation {
        InteropRepresentation::Owned => {
            matches!(mode, ReturnMode::OwnedValue | ReturnMode::StructuredValue)
        }
        InteropRepresentation::SharedHost
        | InteropRepresentation::ExclusiveHost
        | InteropRepresentation::CollectionView(_)
        | InteropRepresentation::CollectionMut { .. } => {
            matches!(mode, ReturnMode::ScopedHost { .. })
        }
    }
}

fn service_abi_fingerprint(
    id: ServiceId,
    path: &str,
    methods: &[ServiceMethodDescriptor],
) -> ServiceAbiFingerprint {
    let facts = methods
        .iter()
        .map(|method| {
            let mut closure = method
                .type_closure
                .iter()
                .map(|requirement| {
                    let contract = requirement.contract;
                    format!(
                        "{:032x}:{}:{:016x}",
                        contract.type_id.get(),
                        contract.representation.abi_name(),
                        contract.abi_fingerprint.get()
                    )
                })
                .collect::<Vec<_>>();
            closure.sort_unstable();
            format!(
                "{:032x}:{:016x}:{}",
                method.id.get(),
                method.callable.abi_fingerprint().get(),
                closure.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    ServiceAbiFingerprint::new(stable_id(
        "vela_service_abi_v1",
        path,
        &format!("{:032x}:{facts}", id.get()),
    ))
}

fn service_set_abi_fingerprint(
    id: ServiceSetId,
    path: &str,
    services: &[ServiceSchema],
) -> ServiceSetAbiFingerprint {
    let facts = services
        .iter()
        .map(|service| {
            format!(
                "{:032x}:{:016x}",
                service.id().get(),
                service.abi_fingerprint().get()
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    ServiceSetAbiFingerprint::new(stable_id(
        "vela_service_set_abi_v1",
        path,
        &format!("{:032x}:{facts}", id.get()),
    ))
}

#[derive(Clone, Copy)]
enum ServicePathKind {
    Service,
    ServiceSet,
}

fn validate_qualified_path(path: &str, kind: ServicePathKind) -> Result<(), ServiceSchemaError> {
    if !path.is_empty() && path.split("::").all(is_simple_identifier) {
        return Ok(());
    }
    match kind {
        ServicePathKind::Service => Err(ServiceSchemaError::InvalidServicePath(path.to_owned())),
        ServicePathKind::ServiceSet => {
            Err(ServiceSchemaError::InvalidServiceSetPath(path.to_owned()))
        }
    }
}

fn is_simple_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use vela_common::{CallableAsyncness, InteropRepresentation, ServiceId, ServiceMethodId};
    use vela_def::TypeId;
    use vela_reflect::registry::TypeKey;

    use super::{
        ServiceMethodDescriptor, ServiceSchema, ServiceSchemaError, ServiceSetSchema,
        ServiceTypeRequirement,
    };
    use crate::engine::Engine;
    use crate::interop::{
        BoundaryMode, CallableAccess, CallableContract, CallableIdentity, CallableKind,
        CallableLanguage, CallableOrigin, CallableParameter, CallableReturn, ErrorMode, ReturnMode,
    };
    use crate::native::{EffectSet, TypeHint};

    const SERVICE_ID: ServiceId = ServiceId::new(0x51);
    const METHOD_ID: ServiceMethodId = ServiceMethodId::new(0x71);

    fn registry() -> std::sync::Arc<crate::type_binding::TypeBindingRegistry> {
        Engine::builder()
            .register_rust_value_closure::<i64>()
            .register_rust_value_closure::<String>()
            .register_rust_value_closure::<()>()
            .build()
            .expect("standard scalar bindings should seal")
            .type_bindings()
    }

    fn method(registry: &crate::type_binding::TypeBindingRegistry) -> ServiceMethodDescriptor {
        let amount = ServiceTypeRequirement::for_rust_type::<i64>(
            registry,
            "parameter amount",
            InteropRepresentation::Owned,
        )
        .expect("i64 binding");
        let returns = ServiceTypeRequirement::for_rust_type::<()>(
            registry,
            "return",
            InteropRepresentation::Owned,
        )
        .expect("unit binding");
        ServiceMethodDescriptor::new(
            METHOD_ID,
            "game::reward::apply",
            CallableContract {
                identity: CallableIdentity::new(CallableKind::RustTraitMethod, METHOD_ID.get()),
                public_path: "game::reward::apply".to_owned(),
                parameters: vec![
                    CallableParameter::new(1, "amount", TypeHint::i64(), BoundaryMode::Value)
                        .with_binding(amount.contract()),
                ],
                returns: CallableReturn::new(
                    TypeHint::unit(),
                    ReturnMode::OwnedValue,
                    ErrorMode::Value,
                )
                .with_binding(returns.contract()),
                asyncness: CallableAsyncness::Sync,
                effects: EffectSet::pure(),
                access: CallableAccess::default(),
                docs: None,
                origin: CallableOrigin {
                    language: CallableLanguage::Rust,
                    source_span: None,
                },
            },
            vec![amount, returns],
        )
    }

    #[test]
    fn service_schema_seals_exact_callable_and_type_binding_abi() {
        let registry = registry();
        let schema = ServiceSchema::new(
            SERVICE_ID,
            "game::reward",
            vec![method(&registry)],
            &registry,
        )
        .expect("complete service schema");
        let service_set = ServiceSetSchema::new(
            vela_common::ServiceSetId::new(0x91),
            "game::services",
            vec![schema.clone()],
            &registry,
        )
        .expect("complete service set");

        assert_eq!(schema.methods()[0].id, METHOD_ID);
        assert_ne!(schema.abi_fingerprint().get(), 0);
        assert_eq!(service_set.services()[0].id(), SERVICE_ID);
        assert_eq!(service_set.type_binding_checksum(), registry.checksum());
    }

    #[test]
    fn missing_concrete_rust_binding_is_rejected_before_schema_creation() {
        struct Unregistered;

        let registry = registry();
        let error = ServiceTypeRequirement::for_rust_type::<Unregistered>(
            &registry,
            "parameter request",
            InteropRepresentation::Owned,
        )
        .expect_err("unregistered service type must fail");

        assert!(matches!(
            error,
            ServiceSchemaError::MissingRustTypeBinding {
                location,
                rust_type
            } if location == "parameter request"
                && rust_type.ends_with("Unregistered")
        ));
    }

    #[test]
    fn nested_custom_type_must_have_a_registered_transitive_binding() {
        let registry = registry();
        let mut method = method(&registry);
        method.callable.parameters[0].ty = TypeHint::array_of(TypeHint::Record(TypeKey::new(
            TypeId::new(0x404),
            "game::Reward",
        )));

        let error = ServiceSchema::new(SERVICE_ID, "game::reward", vec![method], &registry)
            .expect_err("nested custom type omitted from closure must fail");

        assert!(matches!(
            error,
            ServiceSchemaError::MissingTransitiveTypeBinding {
                location,
                type_id,
                ..
            } if location == "parameter amount"
                && type_id == vela_common::InteropTypeId::from_type_id(TypeId::new(0x404))
        ));
    }

    #[test]
    fn sealed_registry_completes_alias_hidden_transitive_value_types() {
        let registry = registry();
        let mut method = method(&registry);
        let string = registry
            .get_for::<String>()
            .expect("String binding in test registry");
        method.callable.parameters[0].ty = TypeHint::array_of(TypeHint::Record(string.key.clone()));

        let schema = ServiceSchema::new(SERVICE_ID, "game::reward", vec![method], &registry)
            .expect("registered alias-hidden type should complete the closure");

        assert!(
            schema.methods()[0]
                .type_closure
                .iter()
                .any(|requirement| requirement.contract().type_id == string.id)
        );
    }

    #[test]
    fn callable_binding_must_be_present_in_declared_type_closure() {
        let registry = registry();
        let mut method = method(&registry);
        method.type_closure.remove(0);

        let error = ServiceSchema::new(SERVICE_ID, "game::reward", vec![method], &registry)
            .expect_err("top-level callable binding omitted from closure must fail");

        assert!(matches!(
            error,
            ServiceSchemaError::MissingTypeClosureEntry { location, .. }
                if location == "parameter amount"
        ));
    }
}
