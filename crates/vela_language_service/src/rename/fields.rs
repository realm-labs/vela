use std::collections::BTreeMap;

use crate::source_record_fields::RecordOwner;
use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::module_graph::ModuleGraph;

use crate::{DocumentId, LanguageServiceDatabases, query_context};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, span_text_range,
    workspace_edit_for_rename,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ScriptFieldRenameTarget {
    pub(super) owner: RecordOwner,
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

    for source in databases.source_db().records().values() {
        for site in crate::source_record_fields::sites(databases, source)
            .into_iter()
            .filter(|site| site.owner == target.owner)
        {
            if new_name != target.field && site.name == new_name {
                return None;
            }
            if site.name != target.field {
                continue;
            }
            let new_text = if site.shorthand && new_name != target.field {
                format!("{new_name}: {}", site.name)
            } else {
                new_name.to_owned()
            };
            edits_by_document
                .entry(source.document_id().clone())
                .or_default()
                .push(TextEdit {
                    range: diagnostic_range(source.text(), site.range),
                    new_text,
                });
        }
    }
    workspace_edit_for_rename(databases, edits_by_document, Vec::new())
}

pub(super) fn script_field_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    token: &RenameToken,
) -> Option<ScriptFieldRenameTarget> {
    let (owner, field) =
        crate::source_record_fields::declaration_target(graph, source_id, token.range)?;
    Some(ScriptFieldRenameTarget {
        owner,
        field,
        token: token.clone(),
    })
}

pub(super) fn script_record_field_target(
    databases: &LanguageServiceDatabases,
    query: &crate::QueryContext<'_>,
    token: &RenameToken,
) -> Option<Option<ScriptFieldRenameTarget>> {
    let site = crate::source_record_fields::explicit_target(
        databases,
        query.source_record()?,
        token.range,
    )?;
    let exists = site
        .owner
        .fields(databases.hir_db().graph())
        .is_some_and(|fields| fields.iter().any(|field| field.name == site.name));
    Some(exists.then_some(ScriptFieldRenameTarget {
        owner: site.owner,
        field: site.name,
        token: token.clone(),
    }))
}

pub(super) fn script_field_target_for_receiver_fact(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    field: &str,
    token: &RenameToken,
) -> Option<ScriptFieldRenameTarget> {
    let owner = crate::source_record_fields::receiver_owner(graph, receiver, field)?;
    Some(ScriptFieldRenameTarget {
        owner: RecordOwner {
            declaration: owner,
            variant: None,
        },
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
    let field = target
        .owner
        .fields(graph)?
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
    owner: RecordOwner,
    field: String,
}

fn script_field_target(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    field: &str,
) -> Option<ScriptFieldTarget> {
    let owner = crate::source_record_fields::receiver_owner(graph, receiver, field)?;
    Some(ScriptFieldTarget {
        owner: RecordOwner {
            declaration: owner,
            variant: None,
        },
        field: field.to_owned(),
    })
}

fn script_field_name_conflicts(
    graph: &ModuleGraph,
    target: &ScriptFieldRenameTarget,
    new_name: &str,
) -> bool {
    target.owner.fields(graph).is_some_and(|fields| {
        fields
            .iter()
            .any(|field| field.name == new_name && field.name != target.field)
    })
}
