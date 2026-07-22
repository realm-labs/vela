use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};

use crate::{
    DocumentId, LanguageServiceDatabases, query_context,
    symbol_ref::qualified_source_declaration_path,
};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, span_text_range,
    workspace_edit_for_rename,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ScriptFieldRenameTarget {
    pub(super) owner: HirDeclId,
    pub(super) field: String,
    pub(super) token: RenameToken,
}

pub(super) fn rename_script_field(
    databases: &LanguageServiceDatabases,
    target: ScriptFieldRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let graph = databases.hir_db().graph();
    if script_field_name_conflicts(graph, &target, new_name) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_script_field_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_script_field_use_edits(databases, &target, new_name, &mut edits_by_document);

    workspace_edit_for_rename(databases, edits_by_document, Vec::new())
}

pub(super) fn script_field_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    token: &RenameToken,
) -> Option<ScriptFieldRenameTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Struct
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let shape = graph.struct_shape(declaration.id)?;
        for field in &shape.fields {
            let field_range = span_text_range(field.span)?;
            if field_range.start <= token.range.start && token.range.end <= field_range.end {
                return Some(ScriptFieldRenameTarget {
                    owner: declaration.id,
                    field: field.name.clone(),
                    token: token.clone(),
                });
            }
        }
    }
    None
}

pub(super) fn script_field_target_for_receiver_fact(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    field: &str,
    token: &RenameToken,
) -> Option<ScriptFieldRenameTarget> {
    let owner = script_field_owner(graph, receiver, field)?;
    Some(ScriptFieldRenameTarget {
        owner,
        field: field.to_owned(),
        token: token.clone(),
    })
}

fn push_script_field_declaration_edit(
    databases: &LanguageServiceDatabases,
    target: &ScriptFieldRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    let graph = databases.hir_db().graph();
    let field = graph
        .struct_shape(target.owner)?
        .fields
        .iter()
        .find(|field| field.name == target.field)?;
    let source = databases.source_record_for_rename(field.span.source)?;
    let range = span_text_range(field.span)?;
    edits_by_document
        .entry(source.document_id().clone())
        .or_default()
        .push(TextEdit {
            range: diagnostic_range(source.text(), range),
            new_text: new_name.to_owned(),
        });
    Some(())
}

fn push_script_field_use_edits(
    databases: &LanguageServiceDatabases,
    target: &ScriptFieldRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let graph = databases.hir_db().graph();
    for source in databases.source_db().records().values() {
        let source_id = source.source_id();
        let text = source.text();
        for field in graph.fields_in_source(source_id) {
            if field.name != target.field {
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
                .and_then(|receiver| script_field_target(graph, &receiver, &target.field))
                .is_some_and(|found| found.owner == target.owner && found.field == target.field)
            {
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
struct ScriptFieldTarget {
    owner: HirDeclId,
    field: String,
}

fn script_field_target(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    field: &str,
) -> Option<ScriptFieldTarget> {
    let owner = script_field_owner(graph, receiver, field)?;
    Some(ScriptFieldTarget {
        owner,
        field: field.to_owned(),
    })
}

fn script_field_owner(graph: &ModuleGraph, receiver: &TypeFact, field: &str) -> Option<HirDeclId> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct {
            return None;
        }
        let matches_owner = owner_names
            .iter()
            .any(|owner| declaration_name_matches(graph, declaration, owner));
        let has_field = graph
            .struct_shape(declaration.id)
            .is_some_and(|shape| shape.fields.iter().any(|entry| entry.name == field));
        (matches_owner && has_field).then_some(declaration.id)
    })
}

fn script_field_name_conflicts(
    graph: &ModuleGraph,
    target: &ScriptFieldRenameTarget,
    new_name: &str,
) -> bool {
    graph.struct_shape(target.owner).is_some_and(|shape| {
        shape
            .fields
            .iter()
            .any(|field| field.name == new_name && field.name != target.field)
    })
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

fn declaration_name_matches(graph: &ModuleGraph, declaration: &Declaration, owner: &str) -> bool {
    declaration.name == owner
        || qualified_source_declaration_path(graph, declaration).join("::") == owner
}
