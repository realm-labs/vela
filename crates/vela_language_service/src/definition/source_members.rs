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
    let mut definitions = Vec::new();
    for candidate in target.possible_member_targets() {
        if let Some(definition) = source_member_definition_for_owner(databases, &candidate)
            && !definitions.contains(&definition)
        {
            definitions.push(definition);
        }
    }
    (definitions.len() == 1).then(|| definitions.remove(0))
}

fn source_member_definition_for_owner(
    databases: &LanguageServiceDatabases,
    target: &SymbolTarget,
) -> Option<Definition> {
    let receiver = target.member_receiver_fact()?;
    let graph = databases.hir_db().graph();
    source_field_definition_for_target(databases, graph, target, receiver)
        .or_else(|| source_impl_method_definition_for_target(databases, graph, target, receiver))
        .or_else(|| {
            source_trait_default_method_definition_for_target(databases, graph, target, receiver)
        })
        .or_else(|| source_trait_method_definition_for_target(databases, graph, target, receiver))
}

pub(super) fn source_field_type_fact_for_target(
    databases: &LanguageServiceDatabases,
    target: &SymbolTarget,
) -> Option<TypeFact> {
    let mut facts = target
        .possible_member_targets()
        .iter()
        .filter_map(|target| source_field_type_fact_for_owner(databases, target))
        .collect::<Vec<_>>();
    facts.sort_by_key(TypeFact::display_name);
    (!facts.is_empty()).then(|| TypeFact::union(facts))
}

fn source_field_type_fact_for_owner(
    databases: &LanguageServiceDatabases,
    target: &SymbolTarget,
) -> Option<TypeFact> {
    let receiver = target.member_receiver_fact()?;
    let graph = databases.hir_db().graph();
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if target
            .member_receiver_declaration()
            .is_some_and(|owner| owner != declaration.id)
            || declaration.kind != DeclarationKind::Struct
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
        field.type_hint.as_ref().map(|hint| {
            crate::callable_context::query_type_fact_from_hint(
                graph,
                hint,
                databases.schema_db().facts(),
            )
        })
    })
}

fn source_field_definition_for_target(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    target: &SymbolTarget,
    receiver: &TypeFact,
) -> Option<Definition> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if target
            .member_receiver_declaration()
            .is_some_and(|owner| owner != declaration.id)
            || declaration.kind != DeclarationKind::Struct
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
    // Inherent methods take precedence; multiple trait implementations must
    // not select a target based on declaration iteration order.
    for inherent in [true, false] {
        let mut candidates = graph.declarations().filter_map(|declaration| {
            if declaration.kind != DeclarationKind::Impl
                || !crate::symbol_ref::source_impl_has_owner(
                    graph,
                    declaration,
                    target.member_receiver_declaration(),
                )
            {
                return None;
            }
            let metadata = graph.impl_metadata(declaration.id)?;
            if matches!(metadata.kind, ImplMetadataKind::Inherent) != inherent
                || !owner_names.iter().any(|owner| {
                    crate::symbol_ref::source_impl_owner_matches(graph, declaration.id, owner)
                })
            {
                return None;
            }
            let method = metadata
                .methods
                .iter()
                .find(|method| method.name == target.text())?;
            definition_from_named_span_with_symbol(
                databases,
                method.name_span,
                &method.name,
                source_impl_method_symbol(graph, declaration.id, &method.name),
            )
        });
        if let Some(definition) = candidates.next() {
            return candidates.next().is_none().then_some(definition);
        }
    }
    None
}

fn source_trait_method_definition_for_target(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    target: &SymbolTarget,
    receiver: &TypeFact,
) -> Option<Definition> {
    let owner_names = trait_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if target
            .member_receiver_declaration()
            .is_some_and(|owner| owner != declaration.id)
            || declaration.kind != DeclarationKind::Trait
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
            method.name_span,
            &method.name,
            source_member_symbol(graph, declaration.id, &method.name),
        )
    })
}

fn source_trait_default_method_definition_for_target(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    target: &SymbolTarget,
    receiver: &TypeFact,
) -> Option<Definition> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Impl
            || !crate::symbol_ref::source_impl_has_owner(
                graph,
                declaration,
                target.member_receiver_declaration(),
            )
        {
            return None;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
            return None;
        };
        if !owner_names
            .iter()
            .any(|owner| crate::symbol_ref::source_impl_owner_matches(graph, declaration.id, owner))
            || metadata
                .methods
                .iter()
                .any(|method| method.name == target.text())
        {
            return None;
        }
        let trait_declaration = graph.resolve_visible_declaration_path(
            declaration.module,
            trait_path,
            DeclarationKind::Trait,
        )?;
        let method = graph
            .trait_shape(trait_declaration.id)?
            .methods
            .iter()
            .find(|method| method.name == target.text() && method.has_default)?;
        definition_from_named_span_with_symbol(
            databases,
            method.name_span,
            &method.name,
            source_member_symbol(graph, trait_declaration.id, &method.name),
        )
    })
}

pub(super) fn definition_from_named_span_with_symbol(
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
        | TypeFact::ArrayView { .. }
        | TypeFact::ArrayMut { .. }
        | TypeFact::Map { .. }
        | TypeFact::MapView { .. }
        | TypeFact::MapMut { .. }
        | TypeFact::Set { .. }
        | TypeFact::SetView { .. }
        | TypeFact::SetMut { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::ScopedIterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Closure
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Tuple { .. }
        | TypeFact::LogicalRecord(_)
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
        | TypeFact::ArrayView { .. }
        | TypeFact::ArrayMut { .. }
        | TypeFact::Map { .. }
        | TypeFact::MapView { .. }
        | TypeFact::MapMut { .. }
        | TypeFact::Set { .. }
        | TypeFact::SetView { .. }
        | TypeFact::SetMut { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::ScopedIterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Closure
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Record { .. }
        | TypeFact::Tuple { .. }
        | TypeFact::LogicalRecord(_)
        | TypeFact::Module { .. } => {}
    }
}

fn push_owner_names(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|owner| owner == name) {
        names.push(name.to_owned());
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
