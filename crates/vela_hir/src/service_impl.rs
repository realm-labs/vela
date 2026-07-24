//! Backend-neutral catalog of sparse Vela service implementations.

use std::fmt;

use vela_common::Span;

use crate::attributes::{HirAttribute, HirAttributeValue};
use crate::body::HirSourceOrigin;
use crate::ids::{HirBodyId, HirDeclId, HirNodeId, ModuleId};
use crate::module_graph::{DeclarationKind, ModuleGraph};
use crate::type_hint::{FunctionSignature, ImplMetadataKind};

pub const SERVICE_IMPL_ATTRIBUTE: &str = "service_impl";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServiceImplCatalog {
    implementations: Vec<ServiceImpl>,
}

impl ServiceImplCatalog {
    pub fn from_graph(graph: &ModuleGraph) -> Result<Self, ServiceImplCatalogError> {
        let mut implementations = Vec::new();
        for declaration in graph.declarations_by_kind(DeclarationKind::Impl) {
            let attrs = graph.declaration_attrs(declaration.id);
            if !attrs
                .iter()
                .any(|attribute| attribute.name == SERVICE_IMPL_ATTRIBUTE)
            {
                continue;
            }
            let attribute = service_attribute(declaration.id, attrs)?;
            let service_path = service_path(declaration.id, attribute)?;
            let metadata = graph.impl_metadata(declaration.id).ok_or_else(|| {
                catalog_error(
                    declaration.id,
                    declaration.span,
                    ServiceImplCatalogErrorKind::MissingImplMetadata,
                )
            })?;
            if !matches!(metadata.kind, ImplMetadataKind::Inherent) {
                return Err(catalog_error(
                    declaration.id,
                    declaration.span,
                    ServiceImplCatalogErrorKind::TraitImplUnsupported,
                ));
            }
            let mut methods = Vec::with_capacity(metadata.methods.len());
            for method in &metadata.methods {
                let body = graph.impl_method_body(method.node).ok_or_else(|| {
                    catalog_error(
                        declaration.id,
                        method.span,
                        ServiceImplCatalogErrorKind::MissingMethodBody {
                            method: method.name.clone(),
                        },
                    )
                })?;
                methods.push(ServiceImplMethod {
                    node: method.node,
                    name: method.name.clone(),
                    body: body.id,
                    signature: method.signature.clone(),
                    module: declaration.module,
                    origin: body.origin,
                    name_span: method.name_span,
                });
            }
            implementations.push(ServiceImpl {
                declaration: declaration.id,
                service_path,
                implementation_path: metadata.target_path.clone(),
                module: declaration.module,
                span: declaration.span,
                methods,
            });
        }
        Ok(Self { implementations })
    }

    pub fn implementations(&self) -> impl ExactSizeIterator<Item = &ServiceImpl> {
        self.implementations.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.implementations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.implementations.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceImpl {
    declaration: HirDeclId,
    service_path: Vec<String>,
    implementation_path: Vec<String>,
    module: ModuleId,
    span: Span,
    methods: Vec<ServiceImplMethod>,
}

impl ServiceImpl {
    #[must_use]
    pub const fn declaration(&self) -> HirDeclId {
        self.declaration
    }

    #[must_use]
    pub fn service_path(&self) -> &[String] {
        &self.service_path
    }

    #[must_use]
    pub fn service_path_text(&self) -> String {
        self.service_path.join("::")
    }

    #[must_use]
    pub fn implementation_path(&self) -> &[String] {
        &self.implementation_path
    }

    #[must_use]
    pub const fn module(&self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }

    pub fn methods(&self) -> impl ExactSizeIterator<Item = &ServiceImplMethod> {
        self.methods.iter()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceImplMethod {
    node: HirNodeId,
    name: String,
    body: HirBodyId,
    signature: FunctionSignature,
    module: ModuleId,
    origin: HirSourceOrigin,
    name_span: Span,
}

impl ServiceImplMethod {
    #[must_use]
    pub const fn node(&self) -> HirNodeId {
        self.node
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn body(&self) -> HirBodyId {
        self.body
    }

    #[must_use]
    pub const fn signature(&self) -> &FunctionSignature {
        &self.signature
    }

    #[must_use]
    pub const fn module(&self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn origin(&self) -> HirSourceOrigin {
        self.origin
    }

    #[must_use]
    pub const fn name_span(&self) -> Span {
        self.name_span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceImplCatalogError {
    declaration: HirDeclId,
    span: Span,
    kind: ServiceImplCatalogErrorKind,
}

impl ServiceImplCatalogError {
    #[must_use]
    pub const fn declaration(&self) -> HirDeclId {
        self.declaration
    }

    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }

    #[must_use]
    pub const fn kind(&self) -> &ServiceImplCatalogErrorKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceImplCatalogErrorKind {
    DuplicateAttribute,
    InvalidAttribute,
    MissingImplMetadata,
    TraitImplUnsupported,
    MissingMethodBody { method: String },
}

impl fmt::Display for ServiceImplCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ServiceImplCatalogErrorKind::DuplicateAttribute => {
                formatter.write_str("service impl has duplicate #[service_impl] attributes")
            }
            ServiceImplCatalogErrorKind::InvalidAttribute => formatter.write_str(
                "#[service_impl] requires exactly one positional qualified service path",
            ),
            ServiceImplCatalogErrorKind::MissingImplMetadata => {
                formatter.write_str("service impl has no HIR metadata")
            }
            ServiceImplCatalogErrorKind::TraitImplUnsupported => {
                formatter.write_str("#[service_impl] must annotate an inherent impl block")
            }
            ServiceImplCatalogErrorKind::MissingMethodBody { method } => {
                write!(formatter, "service impl method `{method}` has no HIR body")
            }
        }
    }
}

impl std::error::Error for ServiceImplCatalogError {}

#[must_use]
pub fn is_service_impl(attrs: &[HirAttribute]) -> bool {
    attrs
        .iter()
        .any(|attribute| attribute.name == SERVICE_IMPL_ATTRIBUTE)
}

fn service_attribute(
    declaration: HirDeclId,
    attrs: &[HirAttribute],
) -> Result<&HirAttribute, ServiceImplCatalogError> {
    let mut matching = attrs
        .iter()
        .filter(|attribute| attribute.name == SERVICE_IMPL_ATTRIBUTE);
    let attribute = matching.next().expect("caller found service_impl");
    if let Some(duplicate) = matching.next() {
        return Err(catalog_error(
            declaration,
            duplicate.span,
            ServiceImplCatalogErrorKind::DuplicateAttribute,
        ));
    }
    Ok(attribute)
}

fn service_path(
    declaration: HirDeclId,
    attribute: &HirAttribute,
) -> Result<Vec<String>, ServiceImplCatalogError> {
    let [argument] = attribute.arguments.as_slice() else {
        return Err(catalog_error(
            declaration,
            attribute.span,
            ServiceImplCatalogErrorKind::InvalidAttribute,
        ));
    };
    let HirAttributeValue::Path(path) = &argument.value else {
        return Err(catalog_error(
            declaration,
            argument.value_span,
            ServiceImplCatalogErrorKind::InvalidAttribute,
        ));
    };
    if argument.name.is_some() || path.is_empty() {
        return Err(catalog_error(
            declaration,
            argument.span,
            ServiceImplCatalogErrorKind::InvalidAttribute,
        ));
    }
    Ok(path.clone())
}

const fn catalog_error(
    declaration: HirDeclId,
    span: Span,
    kind: ServiceImplCatalogErrorKind,
) -> ServiceImplCatalogError {
    ServiceImplCatalogError {
        declaration,
        span,
        kind,
    }
}
