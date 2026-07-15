use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;

use crate::{LanguageServiceDatabases, query_context};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, record_owner_names,
    source_impl_method_symbol, source_member_symbol, span_text_range,
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
    token: &ReferenceToken,
) -> Option<MethodReferenceTarget> {
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Impl || declaration.span.source != source_id {
            continue;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        for method in &metadata.methods {
            let name_range = span_text_range(method.name_span)?;
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
    let name_span = match target.target_kind {
        MethodReferenceTargetKind::Impl => {
            graph
                .impl_metadata(target.owner)?
                .methods
                .iter()
                .find(|method| method.name == target.method)?
                .name_span
        }
        MethodReferenceTargetKind::Trait => {
            graph
                .trait_shape(target.owner)?
                .methods
                .iter()
                .find(|method| method.name == target.method)?
                .name_span
        }
    };
    let name_range = span_text_range(name_span)?;
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
    for field in graph.member_calls_in_source(source_id) {
        if field.name != target.method {
            continue;
        }
        let Some(receiver_range) = graph
            .expression_span(field.receiver)
            .and_then(span_text_range)
        else {
            continue;
        };
        let Some(member_range) = span_text_range(field.member_origin.span) else {
            continue;
        };
        if query_context::type_fact_for_source_range(databases, source_id, receiver_range)
            .and_then(|receiver| {
                script_method_target_for_receiver_fact(graph, &receiver, &target.method)
            })
            .as_ref()
            == Some(target)
        {
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, member_range),
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
        | DeclarationKind::State
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
