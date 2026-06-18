use vela_analysis::registry::RegistryFacts;
use vela_hir::{
    binding::BindingMap,
    module_graph::{Declaration, DeclarationKind, ModuleGraph},
    type_hint::{EnumVariantFieldsHint, FunctionSignature, HirTypeHint},
};

use crate::TextRange;

use super::{
    SemanticTokenClassification, SemanticTokenModifiers, SemanticTokenType, span_contains_range,
    token_text,
};

pub(super) fn classification(
    graph: &ModuleGraph,
    declaration: &Declaration,
    schema: &RegistryFacts,
    text: &str,
    name: &str,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    let mut classification = None;
    for_each_type_hint_in_declaration(graph, declaration, |hint| {
        if classification.is_none() {
            classification = type_hint_classification(graph, schema, text, hint, name, range);
        }
    });
    classification
}

fn type_hint_classification(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    text: &str,
    hint: &HirTypeHint,
    name: &str,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    let path_name = hint.path.last()?;
    if path_name != name || !span_contains_range(hint.span, range) {
        return None;
    }
    if token_text(text, range) != Some(name) {
        return None;
    }

    let token_type = if is_builtin_type_hint(hint) {
        SemanticTokenType::BuiltinType
    } else {
        SemanticTokenType::Type
    };
    let modifiers = type_hint_modifiers(graph, schema, hint);
    Some(SemanticTokenClassification::new(token_type, modifiers))
}

fn type_hint_modifiers(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    hint: &HirTypeHint,
) -> SemanticTokenModifiers {
    if is_builtin_type_hint(hint) {
        return SemanticTokenModifiers::BUILTIN;
    }
    let qualified = hint.path.join("::");
    if schema.type_fact(&qualified).is_some()
        || schema.trait_fact(&qualified).is_some()
        || hint.path.last().is_some_and(|name| {
            schema.type_fact(name).is_some() || schema.trait_fact(name).is_some()
        })
    {
        return SemanticTokenModifiers::HOST;
    }
    if is_source_type_hint(graph, hint) {
        return SemanticTokenModifiers::SOURCE;
    }
    SemanticTokenModifiers::NONE
}

fn is_source_type_hint(graph: &ModuleGraph, hint: &HirTypeHint) -> bool {
    graph.declarations().any(|declaration| {
        matches!(
            declaration.kind,
            DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
        ) && type_hint_matches_declaration(graph, hint, declaration)
    })
}

fn type_hint_matches_declaration(
    graph: &ModuleGraph,
    hint: &HirTypeHint,
    declaration: &Declaration,
) -> bool {
    if hint.path.len() == 1 {
        return hint
            .path
            .first()
            .is_some_and(|name| name == &declaration.name);
    }
    graph.module_path(declaration.module).is_some_and(|module| {
        module
            .segments()
            .iter()
            .chain(std::iter::once(&declaration.name))
            .eq(hint.path.iter())
    })
}

fn is_builtin_type_hint(hint: &HirTypeHint) -> bool {
    let [name] = hint.path.as_slice() else {
        return false;
    };
    matches!(
        name.as_str(),
        "Any"
            | "Array"
            | "Bytes"
            | "Function"
            | "Iterator"
            | "Map"
            | "Option"
            | "Result"
            | "Set"
            | "String"
            | "bool"
            | "char"
            | "f32"
            | "f64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
    )
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

fn visit_binding_type_hints(bindings: &BindingMap, visit: &mut impl FnMut(&HirTypeHint)) {
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
