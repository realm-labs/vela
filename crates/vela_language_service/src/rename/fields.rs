use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};

use crate::{
    DocumentId, LanguageServiceDatabases, member_access, query_context,
    symbol_ref::qualified_source_declaration_path,
};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, document_text_edit_for_rename,
    name_range_in_text, span_text_range,
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

    let document_edits = edits_by_document
        .into_iter()
        .map(|(document_id, mut edits)| {
            edits.sort_by_key(|edit| {
                let start = edit.range.start();
                (start.line, start.character)
            });
            edits.dedup();
            document_text_edit_for_rename(databases, document_id, edits)
        })
        .collect::<Vec<_>>();

    Some(WorkspaceEdit {
        document_edits,
        risks: Vec::new(),
        symbol: None,
    })
}

pub(super) fn script_field_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
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
            let span_range = span_text_range(field.span)?;
            let name_range = name_range_in_text(text, span_range, &field.name)?;
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
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
    let span_range = span_text_range(field.span)?;
    let range = name_range_in_text(source.text(), span_range, &field.name)?;
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
        let Some(parsed) = databases.parse_db().parsed_source(source.document_id()) else {
            continue;
        };
        for site in member_access::member_access_sites(parsed) {
            if site.member != target.field {
                continue;
            }
            if query_context::type_fact_for_source_range(databases, source_id, site.receiver_range)
                .and_then(|receiver| script_field_target(graph, &receiver, &target.field))
                .is_some_and(|found| found.owner == target.owner && found.field == target.field)
            {
                edits_by_document
                    .entry(source.document_id().clone())
                    .or_default()
                    .push(TextEdit {
                        range: diagnostic_range(text, site.member_range),
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

fn push_owner_name(owners: &mut Vec<String>, name: &str) {
    if !owners.iter().any(|owner| owner == name) {
        owners.push(name.to_owned());
    }
}

fn declaration_name_matches(graph: &ModuleGraph, declaration: &Declaration, owner: &str) -> bool {
    declaration.name == owner
        || qualified_source_declaration_path(graph, declaration).join("::") == owner
}
