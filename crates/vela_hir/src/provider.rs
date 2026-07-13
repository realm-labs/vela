use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use vela_common::{Diagnostic, Span};
use vela_def::{
    MethodId, TraitId, TypeId, script_trait_id, script_trait_method_id, script_type_id,
};
use vela_package::PackageId;

use crate::attributes::{HirAttribute, HirAttributeValue, schema_id_attr};
use crate::ids::HirDeclId;
use crate::module_graph::{Declaration, DeclarationKind, ModuleGraph, Visibility};
use crate::type_hint::{
    FunctionSignature, ImplMetadataKind, ImplMethodMetadata, TraitMethodMetadata,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderKey {
    package: PackageId,
    service: TraitId,
    provider: ProviderId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProviderDescriptor {
    pub key: ProviderKey,
    pub provider_type: TypeId,
    pub methods: Vec<HirProviderMethodDescriptor>,
    pub source: Span,
    pub impl_declaration: HirDeclId,
    pub service_declaration: HirDeclId,
    pub target_declaration: HirDeclId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProviderMethodDescriptor {
    pub id: MethodId,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDiscoveryError {
    diagnostics: Vec<Diagnostic>,
}

struct ProviderDiagnostic(Box<Diagnostic>);

impl From<Diagnostic> for ProviderDiagnostic {
    fn from(diagnostic: Diagnostic) -> Self {
        Self(Box::new(diagnostic))
    }
}

impl ProviderId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let mut chars = value.chars();
        let valid_start = chars
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit());
        if !valid_start
            || !chars.all(|ch| {
                ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '-' | '.')
            })
        {
            return Err(format!("invalid provider id `{value}`"));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl ProviderKey {
    #[must_use]
    pub fn new(package: PackageId, service: TraitId, provider: ProviderId) -> Self {
        Self {
            package,
            service,
            provider,
        }
    }

    #[must_use]
    pub const fn package(&self) -> &PackageId {
        &self.package
    }

    #[must_use]
    pub const fn service(&self) -> TraitId {
        self.service
    }

    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }
}

impl ProviderDiscoveryError {
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for ProviderDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "provider discovery failed with {} diagnostic(s)",
            self.diagnostics.len()
        )
    }
}

impl std::error::Error for ProviderDiscoveryError {}

pub fn discover_providers(
    graph: &ModuleGraph,
) -> Result<Vec<HirProviderDescriptor>, ProviderDiscoveryError> {
    let mut diagnostics = Vec::new();
    let mut providers = Vec::new();
    for declaration in graph.declarations() {
        let attrs = graph.declaration_attrs(declaration.id);
        let provider_attrs = attrs
            .iter()
            .filter(|attribute| attribute.name == "provider")
            .collect::<Vec<_>>();
        if provider_attrs.is_empty() {
            continue;
        }
        if declaration.kind != DeclarationKind::Impl {
            diagnostics.push(provider_diagnostic(
                provider_attrs[0].span,
                "hir::provider_requires_impl",
                "provider attribute is valid only on a trait impl",
            ));
            continue;
        }
        if provider_attrs.len() != 1 {
            diagnostics.push(provider_diagnostic(
                provider_attrs[1].span,
                "hir::duplicate_provider_attribute",
                "provider impl must have exactly one provider attribute",
            ));
            continue;
        }
        match discover_provider(graph, declaration, provider_attrs[0]) {
            Ok(provider) => providers.push(provider),
            Err(diagnostic) => diagnostics.push(*diagnostic.0),
        }
    }

    let mut keys = BTreeMap::new();
    for provider in &providers {
        if let Some(previous) = keys.insert(provider.key.clone(), provider.source) {
            diagnostics.push(
                provider_diagnostic(
                    provider.source,
                    "hir::duplicate_provider_key",
                    format!(
                        "duplicate provider id `{}` for the same package and service",
                        provider.key.provider()
                    ),
                )
                .with_label(previous, "previous provider is declared here"),
            );
        }
    }
    if diagnostics.is_empty() {
        providers.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(providers)
    } else {
        Err(ProviderDiscoveryError { diagnostics })
    }
}

fn discover_provider(
    graph: &ModuleGraph,
    declaration: &Declaration,
    provider_attr: &HirAttribute,
) -> Result<HirProviderDescriptor, ProviderDiagnostic> {
    let provider_id = provider_id(provider_attr)?;
    let metadata = graph.impl_metadata(declaration.id).ok_or_else(|| {
        provider_diagnostic(
            declaration.span,
            "hir::missing_provider_impl_metadata",
            "provider impl has no semantic impl metadata",
        )
    })?;
    let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
        return Err(provider_diagnostic(
            declaration.span,
            "hir::provider_requires_trait_impl",
            "provider attribute is not valid on an inherent impl",
        )
        .into());
    };
    let service = graph
        .resolve_visible_declaration_path(declaration.module, trait_path, DeclarationKind::Trait)
        .ok_or_else(|| {
            provider_diagnostic(
                declaration.span,
                "hir::unresolved_provider_service",
                "provider service trait did not resolve",
            )
        })?;
    let target = graph
        .resolve_visible_declaration_path(
            declaration.module,
            &metadata.target_path,
            DeclarationKind::Struct,
        )
        .ok_or_else(|| {
            provider_diagnostic(
                declaration.span,
                "hir::unresolved_provider_target",
                "provider target did not resolve to a script record",
            )
        })?;
    let shape = graph.struct_shape(target.id).ok_or_else(|| {
        provider_diagnostic(
            target.span,
            "hir::missing_provider_target_shape",
            "provider target has no record shape",
        )
    })?;
    if target.visibility != Visibility::Public || !shape.fields.is_empty() {
        return Err(provider_diagnostic(
            target.span,
            "hir::invalid_provider_target",
            "provider target must be a public zero-field script record",
        )
        .into());
    }
    let trait_shape = graph.trait_shape(service.id).ok_or_else(|| {
        provider_diagnostic(
            service.span,
            "hir::missing_provider_service_shape",
            "provider service has no trait shape",
        )
    })?;
    validate_methods(
        trait_shape.methods.as_slice(),
        &metadata.methods,
        declaration.span,
    )?;

    let package = graph
        .module_package(declaration.module)
        .expect("declaration module always has a package")
        .clone();
    let service_package = graph
        .module_package(service.module)
        .expect("service module always has a package");
    let service_symbol = graph
        .qualified_declaration_name(service.id)
        .expect("service declaration always has a qualified name");
    let target_package = graph
        .module_package(target.module)
        .expect("target module always has a package");
    let target_symbol = graph
        .qualified_declaration_name(target.id)
        .expect("target declaration always has a qualified name");
    let service_id = script_trait_id(service_package.as_str(), &service_symbol);
    let methods = trait_shape
        .methods
        .iter()
        .map(|method| HirProviderMethodDescriptor {
            id: script_trait_method_id(service_package.as_str(), &service_symbol, &method.name),
            name: method.name.clone(),
        })
        .collect();
    Ok(HirProviderDescriptor {
        key: ProviderKey::new(package, service_id, provider_id),
        provider_type: script_type_id(
            target_package.as_str(),
            &target_symbol,
            schema_id_attr(graph.declaration_attrs(target.id)).map(u128::from),
        ),
        methods,
        source: provider_attr.span,
        impl_declaration: declaration.id,
        service_declaration: service.id,
        target_declaration: target.id,
    })
}

fn provider_id(attribute: &HirAttribute) -> Result<ProviderId, ProviderDiagnostic> {
    if attribute.arguments.len() != 1 {
        return Err(provider_diagnostic(
            attribute.span,
            "hir::invalid_provider_arguments",
            "provider attribute requires exactly one named `id` argument",
        )
        .into());
    }
    let argument = &attribute.arguments[0];
    if argument.name.as_deref() != Some("id") {
        return Err(provider_diagnostic(
            argument.span,
            "hir::invalid_provider_argument_name",
            "provider attribute accepts only the named `id` argument",
        )
        .into());
    }
    let HirAttributeValue::String(value) = &argument.value else {
        return Err(provider_diagnostic(
            argument.value_span,
            "hir::invalid_provider_id_value",
            "provider id must be a string literal",
        )
        .into());
    };
    ProviderId::new(value.clone()).map_err(|message| {
        provider_diagnostic(argument.value_span, "hir::invalid_provider_id", message).into()
    })
}

fn validate_methods(
    service: &[TraitMethodMetadata],
    implementation: &[ImplMethodMetadata],
    impl_span: Span,
) -> Result<(), ProviderDiagnostic> {
    let service_names = service
        .iter()
        .map(|method| method.name.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(method) = implementation
        .iter()
        .find(|method| !service_names.contains(method.name.as_str()))
    {
        return Err(provider_diagnostic(
            method.name_span,
            "hir::unknown_provider_method",
            format!(
                "method `{}` is not part of the provider service",
                method.name
            ),
        )
        .into());
    }
    for required in service {
        let Some(actual) = implementation
            .iter()
            .find(|method| method.name == required.name)
        else {
            if required.has_default {
                continue;
            }
            return Err(provider_diagnostic(
                impl_span,
                "hir::missing_provider_method",
                format!(
                    "provider does not implement required method `{}`",
                    required.name
                ),
            )
            .into());
        };
        if actual.visibility != Visibility::Public {
            return Err(provider_diagnostic(
                actual.name_span,
                "hir::private_provider_method",
                format!("provider method `{}` must be public", actual.name),
            )
            .into());
        }
        if !signatures_match(&required.signature, &actual.signature)
            || effect_contract(&required.attrs) != effect_contract(&actual.attrs)
        {
            return Err(provider_diagnostic(
                actual.span,
                "hir::provider_method_contract_mismatch",
                format!(
                    "provider method `{}` does not match the service contract",
                    actual.name
                ),
            )
            .into());
        }
    }
    Ok(())
}

fn signatures_match(expected: &FunctionSignature, actual: &FunctionSignature) -> bool {
    expected.params.len() == actual.params.len()
        && expected
            .params
            .iter()
            .zip(&actual.params)
            .all(|(left, right)| {
                left.name == right.name
                    && left.type_hint.as_ref().map(|hint| hint.display())
                        == right.type_hint.as_ref().map(|hint| hint.display())
                    && left.default_value_span.is_some() == right.default_value_span.is_some()
            })
        && expected.return_type.as_ref().map(|hint| hint.display())
            == actual.return_type.as_ref().map(|hint| hint.display())
}

fn effect_contract(attrs: &[HirAttribute]) -> Option<String> {
    attrs
        .iter()
        .find(|attribute| attribute.name == "effect")
        .map(HirAttribute::string_value)
}

fn provider_diagnostic(span: Span, code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(message.into())
        .with_code(code)
        .with_span(span)
}

#[cfg(test)]
mod tests;
