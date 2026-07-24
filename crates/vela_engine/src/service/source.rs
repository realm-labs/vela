//! Schema-linked sparse service declarations from Vela HIR.

use std::collections::BTreeMap;
use std::fmt;

use vela_common::{CallableAsyncness, Span};
use vela_def::{FunctionId, script_function_id};
use vela_hir::ids::{HirBodyId, HirDeclId, HirNodeId, ModuleId};
use vela_hir::module_graph::ModuleGraph;
use vela_hir::service_impl::{ServiceImplCatalog, ServiceImplCatalogError};
use vela_hir::type_hint::FunctionSignature;

use super::{
    ServiceMethodKey, ServiceMethodUpdate, ServiceSchema, ServiceSelectionError,
    ServiceSelectionTable, ServiceSetSchema,
};

/// One Vela method body resolved against an imported Rust service schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VelaServiceMethod {
    implementation: String,
    declaration: HirDeclId,
    node: HirNodeId,
    body: HirBodyId,
    module: ModuleId,
    signature: FunctionSignature,
    function: FunctionId,
    symbol: String,
    span: Span,
}

impl VelaServiceMethod {
    #[must_use]
    pub fn implementation(&self) -> &str {
        &self.implementation
    }

    #[must_use]
    pub const fn declaration(&self) -> HirDeclId {
        self.declaration
    }

    #[must_use]
    pub const fn node(&self) -> HirNodeId {
        self.node
    }

    #[must_use]
    pub const fn body(&self) -> HirBodyId {
        self.body
    }

    #[must_use]
    pub const fn module(&self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn signature(&self) -> &FunctionSignature {
        &self.signature
    }

    #[must_use]
    pub const fn function(&self) -> FunctionId {
        self.function
    }

    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }
}

/// Sparse source claims resolved to stable service and method IDs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServiceSourceManifest {
    updates: Vec<ServiceMethodUpdate<VelaServiceMethod>>,
}

impl ServiceSourceManifest {
    pub fn link(
        graph: &ModuleGraph,
        schema: &ServiceSetSchema,
    ) -> Result<Self, ServiceSourceError> {
        let catalog = ServiceImplCatalog::from_graph(graph).map_err(ServiceSourceError::catalog)?;
        let services = schema
            .services()
            .iter()
            .map(|service| (service.path(), service))
            .collect::<BTreeMap<_, _>>();
        let mut claims = BTreeMap::new();
        let mut updates = Vec::new();

        for implementation in catalog.implementations() {
            let service_path = implementation.service_path_text();
            let service = services.get(service_path.as_str()).ok_or_else(|| {
                ServiceSourceError::new(
                    implementation.span(),
                    ServiceSourceErrorKind::UnknownService {
                        service: service_path.clone(),
                    },
                )
            })?;
            if implementation.methods().len() == 0 {
                return Err(ServiceSourceError::new(
                    implementation.span(),
                    ServiceSourceErrorKind::EmptyImplementation {
                        service: service_path,
                    },
                ));
            }
            let implementation_path = implementation.implementation_path().join("::");
            let package = graph
                .module_package(implementation.module())
                .expect("catalogued service impl module has a package");
            for method in implementation.methods() {
                let descriptor = find_method(service, method.name()).ok_or_else(|| {
                    ServiceSourceError::new(
                        method.name_span(),
                        ServiceSourceErrorKind::UnknownMethod {
                            service: service.path().to_owned(),
                            method: method.name().to_owned(),
                        },
                    )
                })?;
                validate_signature(service, descriptor, method.signature(), method.name_span())?;
                let key = ServiceMethodKey::new(service.id(), descriptor.id);
                if let Some(previous) = claims.insert(key, method.name_span()) {
                    return Err(ServiceSourceError::new(
                        method.name_span(),
                        ServiceSourceErrorKind::DuplicateMethodClaim {
                            service: service.path().to_owned(),
                            method: method.name().to_owned(),
                            previous,
                        },
                    ));
                }
                let symbol = implementation.method_symbol(method);
                updates.push(ServiceMethodUpdate::vela(
                    service.id(),
                    descriptor.id,
                    service.abi_fingerprint(),
                    VelaServiceMethod {
                        implementation: implementation_path.clone(),
                        declaration: implementation.declaration(),
                        node: method.node(),
                        body: method.body(),
                        module: method.module(),
                        signature: method.signature().clone(),
                        function: script_function_id(package.as_str(), &symbol),
                        symbol,
                        span: method.origin().span,
                    },
                ));
            }
        }
        Ok(Self { updates })
    }

    pub fn updates(
        &self,
    ) -> impl ExactSizeIterator<Item = &ServiceMethodUpdate<VelaServiceMethod>> {
        self.updates.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.updates.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    pub fn into_snapshot(
        self,
        schema: &ServiceSetSchema,
    ) -> Result<ServiceSelectionTable<VelaServiceMethod>, ServiceSelectionError> {
        ServiceSelectionTable::snapshot(schema, self.updates)
    }

    pub fn into_updates(self) -> Vec<ServiceMethodUpdate<VelaServiceMethod>> {
        self.updates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceSourceError {
    span: Span,
    kind: ServiceSourceErrorKind,
}

impl ServiceSourceError {
    const fn new(span: Span, kind: ServiceSourceErrorKind) -> Self {
        Self { span, kind }
    }

    fn catalog(error: ServiceImplCatalogError) -> Self {
        Self {
            span: error.span(),
            kind: ServiceSourceErrorKind::InvalidDeclaration(error.to_string()),
        }
    }

    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }

    #[must_use]
    pub const fn kind(&self) -> &ServiceSourceErrorKind {
        &self.kind
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self.kind {
            ServiceSourceErrorKind::InvalidDeclaration(_) => "service.source.invalid_declaration",
            ServiceSourceErrorKind::UnknownService { .. } => "service.source.unknown_service",
            ServiceSourceErrorKind::EmptyImplementation { .. } => {
                "service.source.empty_implementation"
            }
            ServiceSourceErrorKind::UnknownMethod { .. } => "service.source.unknown_method",
            ServiceSourceErrorKind::DuplicateMethodClaim { .. } => {
                "service.source.duplicate_method"
            }
            ServiceSourceErrorKind::AsyncnessMismatch { .. } => "service.source.asyncness_mismatch",
            ServiceSourceErrorKind::ParameterCountMismatch { .. } => {
                "service.source.parameter_count"
            }
            ServiceSourceErrorKind::ParameterDefaultUnsupported { .. } => {
                "service.source.parameter_default"
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceSourceErrorKind {
    InvalidDeclaration(String),
    UnknownService {
        service: String,
    },
    EmptyImplementation {
        service: String,
    },
    UnknownMethod {
        service: String,
        method: String,
    },
    DuplicateMethodClaim {
        service: String,
        method: String,
        previous: Span,
    },
    AsyncnessMismatch {
        service: String,
        method: String,
        expected: CallableAsyncness,
        actual: CallableAsyncness,
    },
    ParameterCountMismatch {
        service: String,
        method: String,
        expected: usize,
        actual: usize,
    },
    ParameterDefaultUnsupported {
        service: String,
        method: String,
        parameter: String,
    },
}

impl fmt::Display for ServiceSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ServiceSourceErrorKind::InvalidDeclaration(reason) => {
                write!(formatter, "invalid service implementation: {reason}")
            }
            ServiceSourceErrorKind::UnknownService { service } => {
                write!(formatter, "unknown imported Rust service `{service}`")
            }
            ServiceSourceErrorKind::EmptyImplementation { service } => {
                write!(
                    formatter,
                    "service implementation for `{service}` has no methods"
                )
            }
            ServiceSourceErrorKind::UnknownMethod { service, method } => {
                write!(formatter, "service `{service}` has no method `{method}`")
            }
            ServiceSourceErrorKind::DuplicateMethodClaim {
                service, method, ..
            } => write!(
                formatter,
                "service method `{service}::{method}` is implemented more than once"
            ),
            ServiceSourceErrorKind::AsyncnessMismatch {
                service,
                method,
                expected,
                actual,
            } => write!(
                formatter,
                "service method `{service}::{method}` expects {expected:?}, found {actual:?}"
            ),
            ServiceSourceErrorKind::ParameterCountMismatch {
                service,
                method,
                expected,
                actual,
            } => write!(
                formatter,
                "service method `{service}::{method}` expects {expected} parameters, found {actual}"
            ),
            ServiceSourceErrorKind::ParameterDefaultUnsupported {
                service,
                method,
                parameter,
            } => write!(
                formatter,
                "service method `{service}::{method}` parameter `{parameter}` cannot declare a default"
            ),
        }
    }
}

impl std::error::Error for ServiceSourceError {}

fn find_method<'schema>(
    service: &'schema ServiceSchema,
    method_name: &str,
) -> Option<&'schema super::ServiceMethodDescriptor> {
    service
        .methods()
        .iter()
        .find(|method| method.path.rsplit("::").next() == Some(method_name))
}

fn validate_signature(
    service: &ServiceSchema,
    method: &super::ServiceMethodDescriptor,
    actual: &FunctionSignature,
    span: Span,
) -> Result<(), ServiceSourceError> {
    let expected = &method.callable;
    if actual.asyncness != expected.asyncness {
        return Err(ServiceSourceError::new(
            span,
            ServiceSourceErrorKind::AsyncnessMismatch {
                service: service.path().to_owned(),
                method: method.path.clone(),
                expected: expected.asyncness,
                actual: actual.asyncness,
            },
        ));
    }
    if actual.params.len() != expected.parameters.len() {
        return Err(ServiceSourceError::new(
            span,
            ServiceSourceErrorKind::ParameterCountMismatch {
                service: service.path().to_owned(),
                method: method.path.clone(),
                expected: expected.parameters.len(),
                actual: actual.params.len(),
            },
        ));
    }
    if let Some(parameter) = actual
        .params
        .iter()
        .find(|parameter| parameter.default_value_span.is_some())
    {
        return Err(ServiceSourceError::new(
            parameter.default_value_span.unwrap_or(parameter.span),
            ServiceSourceErrorKind::ParameterDefaultUnsupported {
                service: service.path().to_owned(),
                method: method.path.clone(),
                parameter: parameter.name.clone(),
            },
        ));
    }
    Ok(())
}
