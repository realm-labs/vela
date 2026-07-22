use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;

use crate::{DocumentId, LanguageServiceDatabases, TextRange, query_context};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, span_text_range, token_text,
    workspace_edit_for_rename,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ScriptMethodRenameTarget {
    pub(super) owner: HirDeclId,
    pub(super) method: String,
    pub(super) token: RenameToken,
    target_kind: ScriptMethodRenameTargetKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ScriptMethodRenameTargetKind {
    Impl,
    Trait,
}

pub(super) fn rename_script_method(
    databases: &LanguageServiceDatabases,
    target: ScriptMethodRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let graph = databases.hir_db().graph();
    if script_method_name_conflicts(graph, &target, new_name) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_script_method_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_script_method_use_edits(databases, &target, new_name, &mut edits_by_document);

    workspace_edit_for_rename(databases, edits_by_document, Vec::new())
}

pub(super) fn script_method_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    token: &RenameToken,
) -> Option<ScriptMethodRenameTarget> {
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Impl || declaration.span.source != source_id {
            continue;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        for method in &metadata.methods {
            let name_range = span_text_range(method.name_span)?;
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
                return Some(ScriptMethodRenameTarget {
                    owner: declaration.id,
                    method: method.name.clone(),
                    token: token.clone(),
                    target_kind: ScriptMethodRenameTargetKind::Impl,
                });
            }
        }
    }
    None
}

pub(super) fn script_method_target_for_self_receiver(
    graph: &ModuleGraph,
    scope_owner: HirDeclId,
    method: &str,
    token: &RenameToken,
) -> Option<ScriptMethodRenameTarget> {
    script_method_exists_in_inherent_impl(graph, scope_owner, method).then(|| {
        ScriptMethodRenameTarget {
            owner: scope_owner,
            method: method.to_owned(),
            token: token.clone(),
            target_kind: ScriptMethodRenameTargetKind::Impl,
        }
    })
}

pub(super) fn script_method_target_for_receiver_fact(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    method: &str,
    token: &RenameToken,
) -> Option<ScriptMethodRenameTarget> {
    let owner = script_method_owner(graph, receiver, method)?;
    Some(ScriptMethodRenameTarget {
        owner,
        method: method.to_owned(),
        token: token.clone(),
        target_kind: script_method_target_kind(graph, owner)?,
    })
}

fn push_script_method_declaration_edit(
    databases: &LanguageServiceDatabases,
    target: &ScriptMethodRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    let graph = databases.hir_db().graph();
    let name_span = match target.target_kind {
        ScriptMethodRenameTargetKind::Impl => {
            graph
                .impl_metadata(target.owner)?
                .methods
                .iter()
                .find(|method| method.name == target.method)?
                .name_span
        }
        ScriptMethodRenameTargetKind::Trait => {
            graph
                .trait_shape(target.owner)?
                .methods
                .iter()
                .find(|method| method.name == target.method)?
                .name_span
        }
    };
    let declaration = graph.declaration(target.owner)?;
    let source = databases.source_record_for_rename(declaration.span.source)?;
    let range = span_text_range(name_span)?;
    edits_by_document
        .entry(source.document_id().clone())
        .or_default()
        .push(TextEdit {
            range: diagnostic_range(source.text(), range),
            new_text: new_name.to_owned(),
        });
    Some(())
}

fn push_script_method_use_edits(
    databases: &LanguageServiceDatabases,
    target: &ScriptMethodRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let graph = databases.hir_db().graph();
    for source in databases.source_db().records().values() {
        let text = source.text();
        for field in graph.member_calls_in_source(source.source_id()) {
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
            if script_method_target_for_call_field(
                databases,
                graph,
                source,
                receiver_range,
                member_range,
                &target.method,
            )
            .is_some_and(|found| {
                found.owner == target.owner
                    && found.method == target.method
                    && found.target_kind == target.target_kind
            }) {
                edits_by_document
                    .entry(source.document_id().clone())
                    .or_default()
                    .push(TextEdit {
                        range: diagnostic_range(text, member_range),
                        new_text: new_name.to_owned(),
                    });
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ScriptMethodTarget {
    owner: HirDeclId,
    method: String,
    target_kind: ScriptMethodRenameTargetKind,
}

fn script_method_target_for_call_field(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    source: &crate::SourceRecord,
    receiver_range: TextRange,
    member_range: TextRange,
    method: &str,
) -> Option<ScriptMethodTarget> {
    if token_text(source.text(), receiver_range) == Some("self") {
        let start = u32::try_from(member_range.start).ok()?;
        for declaration in graph.declarations() {
            if declaration.span.source != source.source_id() || !declaration.span.contains(start) {
                continue;
            }
            if script_method_exists_in_inherent_impl(graph, declaration.id, method) {
                return Some(ScriptMethodTarget {
                    owner: declaration.id,
                    method: method.to_owned(),
                    target_kind: ScriptMethodRenameTargetKind::Impl,
                });
            }
        }
    }
    let receiver =
        query_context::type_fact_for_source_range(databases, source.source_id(), receiver_range)?;
    let owner = script_method_owner(graph, &receiver, method)?;
    Some(ScriptMethodTarget {
        owner,
        method: method.to_owned(),
        target_kind: script_method_target_kind(graph, owner)?,
    })
}

fn script_method_exists_in_inherent_impl(
    graph: &ModuleGraph,
    owner: HirDeclId,
    method: &str,
) -> bool {
    let Some(metadata) = graph.impl_metadata(owner) else {
        return false;
    };
    matches!(metadata.kind, ImplMetadataKind::Inherent)
        && metadata.methods.iter().any(|entry| entry.name == method)
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
    let matches_owner = owner_names
        .iter()
        .any(|owner| impl_target_matches(&metadata.target_path, owner));
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
        let matches_owner = owner_names
            .iter()
            .any(|owner| impl_target_matches(&metadata.target_path, owner));
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

fn script_method_target_kind(
    graph: &ModuleGraph,
    owner: HirDeclId,
) -> Option<ScriptMethodRenameTargetKind> {
    let declaration = graph.declaration(owner)?;
    match declaration.kind {
        DeclarationKind::Impl => Some(ScriptMethodRenameTargetKind::Impl),
        DeclarationKind::Trait => Some(ScriptMethodRenameTargetKind::Trait),
        DeclarationKind::Const
        | DeclarationKind::Enum
        | DeclarationKind::Function
        | DeclarationKind::State
        | DeclarationKind::Struct => None,
    }
}

fn script_method_name_conflicts(
    graph: &ModuleGraph,
    target: &ScriptMethodRenameTarget,
    new_name: &str,
) -> bool {
    match target.target_kind {
        ScriptMethodRenameTargetKind::Impl => {
            graph.impl_metadata(target.owner).is_some_and(|metadata| {
                metadata
                    .methods
                    .iter()
                    .any(|method| method.name == new_name && method.name != target.method)
            })
        }
        ScriptMethodRenameTargetKind::Trait => {
            graph.trait_shape(target.owner).is_some_and(|shape| {
                shape
                    .methods
                    .iter()
                    .any(|method| method.name == new_name && method.name != target.method)
            })
        }
    }
}

fn impl_target_matches(target_path: &[String], owner: &str) -> bool {
    target_path.last().is_some_and(|name| name == owner) || target_path.join("::") == owner
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

fn record_owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = Vec::new();
    collect_record_owner_names(receiver, &mut owners);
    owners
}

fn collect_record_owner_names(receiver: &TypeFact, owners: &mut Vec<String>) {
    match receiver {
        TypeFact::Record { name } => {
            push_owner_name(owners, name);
            if let Some(short) = name.rsplit("::").next()
                && short != name
            {
                push_owner_name(owners, short);
            }
        }
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_record_owner_names(fact, owners);
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

fn push_owner_name(owners: &mut Vec<String>, name: &str) {
    if !owners.iter().any(|owner| owner == name) {
        owners.push(name.to_owned());
    }
}
