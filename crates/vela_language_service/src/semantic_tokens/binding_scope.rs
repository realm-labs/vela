//! Select signature sites and the canonical binding owner of token expressions.
use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};
use vela_common::Span;
use vela_hir::{
    binding::{BindingMap, BindingResolution, LocalBindingKind},
    module_graph::{Declaration, DeclarationKind, ModuleGraph, Visibility},
};

pub(super) fn is_parameter_declaration(
    graph: &ModuleGraph,
    declaration: &Declaration,
    name: &str,
    span: Span,
) -> bool {
    let contains = |signature: &vela_hir::type_hint::FunctionSignature| {
        signature
            .params
            .iter()
            .any(|parameter| parameter.name == name && parameter.span == span)
    };
    graph
        .function_signature(declaration.id)
        .is_some_and(contains)
        || graph.trait_shape(declaration.id).is_some_and(|shape| {
            shape
                .methods
                .iter()
                .any(|method| contains(&method.signature))
        })
        || graph.impl_metadata(declaration.id).is_some_and(|metadata| {
            metadata
                .methods
                .iter()
                .any(|method| contains(&method.signature))
        })
}

pub(super) fn self_receiver(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    schema: &RegistryFacts,
    resolution: &BindingResolution,
) -> Option<TypeFact> {
    let BindingResolution::Local(local) = resolution else {
        return None;
    };
    let local = bindings.local(*local)?;
    if local.kind != LocalBindingKind::Parameter || local.name != "self" {
        return None;
    }
    let declaration = graph.declaration(bindings.declaration)?;
    let metadata = graph.impl_metadata(declaration.id)?;
    if !metadata.methods.iter().any(|method| {
        method
            .signature
            .params
            .first()
            .is_some_and(|parameter| parameter.span == local.span && parameter.name == "self")
    }) {
        return None;
    }
    let expanded = graph.expand_import_path(declaration.module, &metadata.target_path)?;
    if let Some(target) = graph.resolve_visible_declaration_path(
        declaration.module,
        &expanded,
        DeclarationKind::Struct,
    ) && (target.module == declaration.module || target.visibility == Visibility::Public)
    {
        return Some(TypeFact::record(
            graph.qualified_declaration_name(target.id)?,
        ));
    }
    schema.type_fact(&expanded.join("::")).cloned()
}

pub(super) fn bindings<'a>(
    graph: &'a ModuleGraph,
    declaration: &Declaration,
    span: Span,
) -> Option<&'a BindingMap> {
    graph
        .body_containing_offset(span.source, span.start)
        .and_then(|body| graph.bindings_for_body(body.id))
        .or_else(|| graph.bindings(declaration.id))
}
