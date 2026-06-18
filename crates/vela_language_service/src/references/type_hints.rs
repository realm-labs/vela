use vela_common::SourceId;
use vela_hir::{
    ids::HirDeclId,
    module_graph::{Declaration, DeclarationKind, ImportResolution, ModuleGraph},
    type_hint::{EnumVariantFieldsHint, FunctionSignature, HirTypeHint},
};

use crate::{LanguageServiceDatabases, SymbolRef, TextRange};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, last_name_range_in_text,
    span_text_range,
};

pub(super) fn source_type_hint_reference_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<HirDeclId> {
    for owner in graph.declarations() {
        let mut target = None;
        for_each_type_hint_in_declaration(graph, owner, |hint| {
            if target.is_some() {
                return;
            }
            if let Some(declaration) = source_type_hint_target(graph, owner, hint)
                && hint.span.source == source_id
                && let Some(range) =
                    source_type_hint_name_range(text, hint, graph.declaration(declaration))
                && range.start <= token.range.start
                && token.range.end <= range.end
            {
                target = Some(declaration);
            }
        });
        if target.is_some() {
            return target;
        }
    }
    None
}

pub(super) fn source_type_hint_references_for_declaration(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    owner: &Declaration,
    target: HirDeclId,
    symbol: SymbolRef,
) -> Vec<Reference> {
    let mut references = Vec::new();
    for_each_type_hint_in_declaration(graph, owner, |hint| {
        if source_type_hint_target(graph, owner, hint) != Some(target) {
            return;
        }
        let Some(source) = databases.source_record_for_reference(hint.span.source) else {
            return;
        };
        let Some(range) =
            source_type_hint_name_range(source.text(), hint, graph.declaration(target))
        else {
            return;
        };
        references.push(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(source.text(), range),
            kind: ReferenceKind::Read,
            symbol: symbol.clone(),
        });
    });
    references
}

pub(super) fn is_type_declaration_id(graph: &ModuleGraph, declaration: HirDeclId) -> bool {
    graph.declaration(declaration).is_some_and(|declaration| {
        matches!(
            declaration.kind,
            DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
        )
    })
}

fn source_type_hint_target(
    graph: &ModuleGraph,
    owner: &Declaration,
    hint: &HirTypeHint,
) -> Option<HirDeclId> {
    if !hint.args.is_empty() {
        return None;
    }
    type_declaration_for_hint_path(graph, owner, &hint.path).map(|declaration| declaration.id)
}

fn type_declaration_for_hint_path<'a>(
    graph: &'a ModuleGraph,
    owner: &Declaration,
    path: &[String],
) -> Option<&'a Declaration> {
    [
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
    ]
    .into_iter()
    .find_map(|kind| {
        graph
            .module_path(owner.module)
            .and_then(|module_path| {
                graph.declaration_by_type_path(path, module_path.segments(), kind)
            })
            .or_else(|| imported_type_declaration_for_hint_path(graph, owner, path, kind))
    })
}

fn imported_type_declaration_for_hint_path<'a>(
    graph: &'a ModuleGraph,
    owner: &Declaration,
    path: &[String],
    kind: DeclarationKind,
) -> Option<&'a Declaration> {
    let [name] = path else {
        return None;
    };
    graph.imports(owner.module)?.iter().find_map(|import| {
        let binding_name = import.alias.as_ref().or_else(|| import.path.last())?;
        if binding_name != name {
            return None;
        }
        let ImportResolution::Declaration(declaration) = import.resolution?;
        graph
            .declaration(declaration)
            .filter(|declaration| declaration.kind == kind)
    })
}

fn source_type_hint_name_range(
    text: &str,
    hint: &HirTypeHint,
    declaration: Option<&Declaration>,
) -> Option<TextRange> {
    let name = declaration
        .map(|declaration| declaration.name.as_str())
        .or_else(|| hint.path.last().map(String::as_str))?;
    let candidate = hint.path.last().map_or(name, String::as_str);
    let span_range = span_text_range(hint.span)?;
    last_name_range_in_text(text, span_range, candidate)
        .or_else(|| last_name_range_in_text(text, span_range, name))
}

fn for_each_type_hint_in_declaration(
    graph: &ModuleGraph,
    declaration: &Declaration,
    mut visit: impl FnMut(&HirTypeHint),
) {
    if let Some(metadata) = graph.const_metadata(declaration.id)
        && let Some(type_hint) = &metadata.type_hint
    {
        visit_type_hint_and_args(type_hint, &mut visit);
    }
    if let Some(metadata) = graph.global_metadata(declaration.id) {
        visit_type_hint_and_args(&metadata.type_hint, &mut visit);
    }
    if let Some(signature) = graph.function_signature(declaration.id) {
        visit_signature_type_hints(signature, &mut visit);
    }
    if let Some(shape) = graph.struct_shape(declaration.id) {
        for field in &shape.fields {
            if let Some(type_hint) = &field.type_hint {
                visit_type_hint_and_args(type_hint, &mut visit);
            }
        }
    }
    if let Some(shape) = graph.enum_shape(declaration.id) {
        for variant in &shape.variants {
            match &variant.fields {
                EnumVariantFieldsHint::Unit => {}
                EnumVariantFieldsHint::Tuple(params) => {
                    for param in params {
                        if let Some(type_hint) = &param.type_hint {
                            visit_type_hint_and_args(type_hint, &mut visit);
                        }
                    }
                }
                EnumVariantFieldsHint::Record(fields) => {
                    for field in fields {
                        if let Some(type_hint) = &field.type_hint {
                            visit_type_hint_and_args(type_hint, &mut visit);
                        }
                    }
                }
            }
        }
    }
    if let Some(shape) = graph.trait_shape(declaration.id) {
        for method in &shape.methods {
            visit_signature_type_hints(&method.signature, &mut visit);
            if let Some(node) = method.default_body_node
                && let Some(bindings) = graph.trait_default_method_bindings(node)
            {
                visit_binding_type_hints(bindings, &mut visit);
            }
        }
    }
    if let Some(metadata) = graph.impl_metadata(declaration.id) {
        for method in &metadata.methods {
            visit_signature_type_hints(&method.signature, &mut visit);
            if let Some(bindings) = graph.impl_method_bindings(method.node) {
                visit_binding_type_hints(bindings, &mut visit);
            }
        }
    }
    if let Some(bindings) = graph.bindings(declaration.id) {
        visit_binding_type_hints(bindings, &mut visit);
    }
}

fn visit_signature_type_hints(signature: &FunctionSignature, visit: &mut impl FnMut(&HirTypeHint)) {
    for param in &signature.params {
        if let Some(type_hint) = &param.type_hint {
            visit_type_hint_and_args(type_hint, visit);
        }
    }
    if let Some(type_hint) = &signature.return_type {
        visit_type_hint_and_args(type_hint, visit);
    }
}

fn visit_binding_type_hints(
    bindings: &vela_hir::binding::BindingMap,
    visit: &mut impl FnMut(&HirTypeHint),
) {
    for binding in bindings.locals() {
        if let Some(type_hint) = &binding.type_hint {
            visit_type_hint_and_args(type_hint, visit);
        }
    }
}

fn visit_type_hint_and_args(hint: &HirTypeHint, visit: &mut impl FnMut(&HirTypeHint)) {
    visit(hint);
    for arg in &hint.args {
        visit_type_hint_and_args(arg, visit);
    }
}
