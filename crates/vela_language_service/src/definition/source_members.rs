use vela_analysis::type_fact::TypeFact;
use vela_common::Span;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;

use crate::symbol_ref::{source_impl_method_symbol, source_member_symbol};
use crate::{
    Definition, LanguageServiceDatabases, TextRange, definition::diagnostic_range,
    definition::name_range_in_text, symbol_target::SymbolTarget,
};

pub(super) fn source_member_definition_for_target(
    databases: &LanguageServiceDatabases,
    target: &SymbolTarget,
) -> Option<Definition> {
    let receiver = target.member_receiver_fact()?;
    let graph = databases.hir_db().graph();
    source_field_definition_for_target(databases, graph, target, receiver)
        .or_else(|| source_impl_method_definition_for_target(databases, graph, target, receiver))
        .or_else(|| source_trait_method_definition_for_target(databases, graph, target, receiver))
}

fn source_field_definition_for_target(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    target: &SymbolTarget,
    receiver: &TypeFact,
) -> Option<Definition> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct
            || !owner_names
                .iter()
                .any(|owner| declaration_name_matches(graph, declaration, owner))
        {
            return None;
        }
        let field = graph
            .struct_shape(declaration.id)?
            .fields
            .iter()
            .find(|field| field.name == target.text())?;
        definition_from_named_span_with_symbol(
            databases,
            field.span,
            &field.name,
            source_member_symbol(graph, declaration.id, &field.name),
        )
    })
}

fn source_impl_method_definition_for_target(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    target: &SymbolTarget,
    receiver: &TypeFact,
) -> Option<Definition> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Impl {
            return None;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        if !matches!(metadata.kind, ImplMetadataKind::Inherent)
            || !owner_names
                .iter()
                .any(|owner| impl_target_matches(&metadata.target_path, owner))
        {
            return None;
        }
        let method = metadata
            .methods
            .iter()
            .find(|method| method.name == target.text())?;
        definition_from_named_span_with_symbol(
            databases,
            method.span,
            &method.name,
            source_impl_method_symbol(graph, declaration.id, &method.name),
        )
    })
}

fn source_trait_method_definition_for_target(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    target: &SymbolTarget,
    receiver: &TypeFact,
) -> Option<Definition> {
    let owner_names = trait_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Trait
            || !owner_names
                .iter()
                .any(|owner| declaration_name_matches(graph, declaration, owner))
        {
            return None;
        }
        let method = graph
            .trait_shape(declaration.id)?
            .methods
            .iter()
            .find(|method| method.name == target.text())?;
        definition_from_named_span_with_symbol(
            databases,
            method.span,
            &method.name,
            source_member_symbol(graph, declaration.id, &method.name),
        )
    })
}

fn definition_from_named_span_with_symbol(
    databases: &LanguageServiceDatabases,
    span: Span,
    name: &str,
    symbol: Option<crate::SymbolRef>,
) -> Option<Definition> {
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == span.source)?;
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    let range = name_range_in_text(source.text(), TextRange::new(start, end), name)
        .unwrap_or(TextRange::new(start, end));
    Some(Definition {
        document_id: source.document_id().clone(),
        range: diagnostic_range(source.text(), range),
        symbol,
    })
}

fn record_owner_names(fact: &TypeFact) -> Vec<String> {
    let mut names = Vec::new();
    collect_record_owner_names(fact, &mut names);
    names
}

fn collect_record_owner_names(fact: &TypeFact, names: &mut Vec<String>) {
    match fact {
        TypeFact::Record { name } => push_owner_names(names, name),
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_record_owner_names(fact, names);
            }
        }
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn trait_owner_names(fact: &TypeFact) -> Vec<String> {
    let mut names = Vec::new();
    collect_trait_owner_names(fact, &mut names);
    names
}

fn collect_trait_owner_names(fact: &TypeFact, names: &mut Vec<String>) {
    match fact {
        TypeFact::Trait { name } => push_owner_names(names, name),
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_trait_owner_names(fact, names);
            }
        }
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Record { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn push_owner_names(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|owner| owner == name) {
        names.push(name.to_owned());
    }
    if let Some(short) = name.rsplit("::").next()
        && short != name
        && !names.iter().any(|owner| owner == short)
    {
        names.push(short.to_owned());
    }
}

fn declaration_name_matches(graph: &ModuleGraph, declaration: &Declaration, owner: &str) -> bool {
    declaration.name == owner
        || graph
            .module_path(declaration.module)
            .map(|path| {
                let module = path.join();
                if module.is_empty() {
                    declaration.name.clone()
                } else {
                    format!("{module}::{}", declaration.name)
                }
            })
            .is_some_and(|qualified| qualified == owner)
}

fn impl_target_matches(path: &[String], owner: &str) -> bool {
    path.last().is_some_and(|name| name == owner) || path.join("::") == owner
}
