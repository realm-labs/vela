use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;

use crate::{LanguageServiceDatabases, TextRange, member_access, query_context};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, is_identifier_boundary,
    is_identifier_continue, record_owner_names, source_impl_method_symbol, source_member_symbol,
    span_text_range,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct MethodReferenceTarget {
    owner: HirDeclId,
    method: String,
    target_kind: MethodReferenceTargetKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum MethodReferenceTargetKind {
    Impl,
    Trait,
}

pub(super) fn script_method_references(
    databases: &LanguageServiceDatabases,
    target: &MethodReferenceTarget,
    include_declaration: bool,
) -> Vec<Reference> {
    let graph = databases.hir_db().graph();
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_script_method_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        references.extend(script_method_use_references_for_source(
            databases, graph, source, target,
        ));
    }

    references.sort_by_key(|reference| {
        let start = reference.range().start();
        (
            reference.document_id().as_str().to_owned(),
            start.line,
            start.character,
            reference.kind(),
        )
    });
    references
}

pub(super) fn script_method_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<MethodReferenceTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Impl
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        for method in &metadata.methods {
            let span_range = span_text_range(declaration.span)?;
            let name_range = method_name_range_in_text(text, span_range, &method.name)?;
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
                return Some(MethodReferenceTarget {
                    owner: declaration.id,
                    method: method.name.clone(),
                    target_kind: MethodReferenceTargetKind::Impl,
                });
            }
        }
    }
    None
}

pub(super) fn script_method_target_for_receiver_fact(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    method: &str,
) -> Option<MethodReferenceTarget> {
    let owner = script_method_owner(graph, receiver, method)?;
    Some(MethodReferenceTarget {
        owner,
        method: method.to_owned(),
        target_kind: method_reference_target_kind(graph, owner)?,
    })
}

fn reference_for_script_method_declaration(
    databases: &LanguageServiceDatabases,
    target: &MethodReferenceTarget,
) -> Option<Reference> {
    let graph = databases.hir_db().graph();
    let declaration = graph.declaration(target.owner)?;
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == declaration.span.source)?;
    let span_range = span_text_range(declaration.span)?;
    let method_name = match target.target_kind {
        MethodReferenceTargetKind::Impl => graph
            .impl_metadata(target.owner)?
            .methods
            .iter()
            .find(|method| method.name == target.method)?
            .name
            .as_str(),
        MethodReferenceTargetKind::Trait => graph
            .trait_shape(target.owner)?
            .methods
            .iter()
            .find(|method| method.name == target.method)?
            .name
            .as_str(),
    };
    let name_range =
        method_name_range_in_text(source.text(), span_range, method_name).unwrap_or(span_range);
    Some(Reference {
        document_id: source.document_id().clone(),
        range: diagnostic_range(source.text(), name_range),
        kind: ReferenceKind::Declaration,
        symbol: method_target_symbol(graph, target)?,
    })
}

fn script_method_use_references_for_source(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    source: &crate::SourceRecord,
    target: &MethodReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let source_id = source.source_id();
    let text = source.text();
    let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) else {
        return references;
    };
    for site in member_access::member_call_sites(parsed) {
        if site.member != target.method {
            continue;
        }
        if query_context::type_fact_for_source_range(databases, source_id, site.receiver_range)
            .and_then(|receiver| {
                script_method_target_for_receiver_fact(graph, &receiver, &target.method)
            })
            .as_ref()
            == Some(target)
        {
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, site.member_range),
                kind: ReferenceKind::Call,
                symbol: method_target_symbol(graph, target)
                    .expect("method target should have a source symbol"),
            });
        }
    }
    references
}

fn script_method_owner(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    method: &str,
) -> Option<HirDeclId> {
    let owner_names = record_owner_names(receiver);
    graph
        .declarations()
        .find_map(|declaration| inherent_method_owner(graph, declaration.id, &owner_names, method))
        .or_else(|| source_trait_default_method_owner(graph, &owner_names, method))
}

fn inherent_method_owner(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    owner_names: &[String],
    method: &str,
) -> Option<HirDeclId> {
    let declaration = graph.declaration(declaration)?;
    if declaration.kind != DeclarationKind::Impl {
        return None;
    }
    let metadata = graph.impl_metadata(declaration.id)?;
    if !matches!(metadata.kind, ImplMetadataKind::Inherent) {
        return None;
    }
    let matches_owner = owner_names.iter().any(|owner| {
        metadata
            .target_path
            .last()
            .is_some_and(|name| name == owner)
            || metadata.target_path.join("::") == *owner
    });
    let has_method = metadata.methods.iter().any(|entry| entry.name == method);
    (matches_owner && has_method).then_some(declaration.id)
}

fn source_trait_default_method_owner(
    graph: &ModuleGraph,
    owner_names: &[String],
    method: &str,
) -> Option<HirDeclId> {
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Impl {
            return None;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
            return None;
        };
        let matches_owner = owner_names.iter().any(|owner| {
            metadata
                .target_path
                .last()
                .is_some_and(|name| name == owner)
                || metadata.target_path.join("::") == *owner
        });
        if !matches_owner || metadata.methods.iter().any(|entry| entry.name == method) {
            return None;
        }
        let trait_declaration = trait_declaration_for_path(graph, trait_path)?;
        graph
            .trait_shape(trait_declaration)
            .is_some_and(|shape| {
                shape
                    .methods
                    .iter()
                    .any(|entry| entry.name == method && entry.has_default)
            })
            .then_some(trait_declaration)
    })
}

fn trait_declaration_for_path(graph: &ModuleGraph, trait_path: &[String]) -> Option<HirDeclId> {
    let owner = trait_path.join("::");
    graph
        .declarations()
        .find(|declaration| {
            declaration.kind == DeclarationKind::Trait
                && (declaration.name == owner
                    || qualified_declaration_name(graph, declaration) == owner)
        })
        .map(|declaration| declaration.id)
}

fn method_reference_target_kind(
    graph: &ModuleGraph,
    owner: HirDeclId,
) -> Option<MethodReferenceTargetKind> {
    let declaration = graph.declaration(owner)?;
    match declaration.kind {
        DeclarationKind::Impl => Some(MethodReferenceTargetKind::Impl),
        DeclarationKind::Trait => Some(MethodReferenceTargetKind::Trait),
        DeclarationKind::Const
        | DeclarationKind::Enum
        | DeclarationKind::Function
        | DeclarationKind::Global
        | DeclarationKind::Struct => None,
    }
}

fn method_target_symbol(
    graph: &ModuleGraph,
    target: &MethodReferenceTarget,
) -> Option<crate::SymbolRef> {
    match target.target_kind {
        MethodReferenceTargetKind::Impl => {
            source_impl_method_symbol(graph, target.owner, &target.method)
        }
        MethodReferenceTargetKind::Trait => {
            source_member_symbol(graph, target.owner, &target.method)
        }
    }
}

fn qualified_declaration_name(
    graph: &ModuleGraph,
    declaration: &vela_hir::module_graph::Declaration,
) -> String {
    graph
        .module_path(declaration.module)
        .map(|path| {
            path.segments()
                .iter()
                .chain(std::iter::once(&declaration.name))
                .cloned()
                .collect::<Vec<_>>()
                .join("::")
        })
        .unwrap_or_else(|| declaration.name.clone())
}

fn method_name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    slice.match_indices(name).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        (is_identifier_boundary(text, start, end) && preceded_by_fn_keyword(text, start))
            .then(|| TextRange::new(start, end))
    })
}

fn preceded_by_fn_keyword(text: &str, start: usize) -> bool {
    let Some(before_name) = text.get(..start).map(str::trim_end) else {
        return false;
    };
    let end = before_name.len();
    let word_start = before_name
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    if before_name.get(word_start..end) != Some("fn") {
        return false;
    }
    before_name
        .get(..word_start)
        .and_then(|prefix| prefix.chars().next_back())
        .is_none_or(|ch| !is_identifier_continue(ch))
}
