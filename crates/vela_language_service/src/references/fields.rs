use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::{LanguageServiceDatabases, query_context};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, resolved_use_reference_kind,
    source_member_symbol, span_text_range,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FieldReferenceTarget {
    owner: HirDeclId,
    field: String,
}

pub(super) fn script_field_references(
    databases: &LanguageServiceDatabases,
    target: &FieldReferenceTarget,
    include_declaration: bool,
) -> Vec<Reference> {
    let graph = databases.hir_db().graph();
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_script_field_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        references.extend(script_field_use_references_for_source(
            databases, graph, source, target,
        ));
        references.extend(script_record_field_references_for_source(
            databases, source, target,
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

pub(super) fn script_field_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    token: &ReferenceToken,
) -> Option<FieldReferenceTarget> {
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
                return Some(FieldReferenceTarget {
                    owner: declaration.id,
                    field: field.name.clone(),
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
) -> Option<FieldReferenceTarget> {
    let owner = crate::source_record_fields::receiver_owner(graph, receiver, field)?;
    Some(FieldReferenceTarget {
        owner,
        field: field.to_owned(),
    })
}

pub(super) fn script_record_field_use_target(
    databases: &LanguageServiceDatabases,
    source: &crate::SourceRecord,
    token: &ReferenceToken,
) -> Option<Option<FieldReferenceTarget>> {
    let site = crate::source_record_fields::explicit_target(databases, source, token.range)?;
    let exists = databases
        .hir_db()
        .graph()
        .struct_shape(site.owner)?
        .fields
        .iter()
        .any(|field| field.name == site.name);
    Some(exists.then_some(FieldReferenceTarget {
        owner: site.owner,
        field: site.name,
    }))
}

fn reference_for_script_field_declaration(
    databases: &LanguageServiceDatabases,
    target: &FieldReferenceTarget,
) -> Option<Reference> {
    let graph = databases.hir_db().graph();
    let field = graph
        .struct_shape(target.owner)?
        .fields
        .iter()
        .find(|field| field.name == target.field)?;
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == field.span.source)?;
    let name_range = span_text_range(field.span)?;
    Some(Reference {
        document_id: source.document_id().clone(),
        range: diagnostic_range(source.text(), name_range),
        kind: ReferenceKind::Declaration,
        symbol: source_member_symbol(graph, target.owner, &target.field)?,
    })
}

fn script_field_use_references_for_source(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    source: &crate::SourceRecord,
    target: &FieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
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
            .and_then(|receiver| {
                script_field_target_for_receiver_fact(graph, &receiver, &target.field)
            })
            .as_ref()
            == Some(target)
        {
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, member_range),
                kind: resolved_use_reference_kind(text, member_range),
                symbol: source_member_symbol(graph, target.owner, &target.field)
                    .expect("field target should have a source symbol"),
            });
        }
    }
    references
}

fn script_record_field_references_for_source(
    databases: &LanguageServiceDatabases,
    source: &crate::SourceRecord,
    target: &FieldReferenceTarget,
) -> Vec<Reference> {
    crate::source_record_fields::sites(databases, source)
        .into_iter()
        .filter(|site| site.owner == target.owner && site.name == target.field)
        .map(|site| Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(source.text(), site.range),
            kind: ReferenceKind::Read,
            symbol: source_member_symbol(databases.hir_db().graph(), target.owner, &target.field)
                .expect("source field"),
        })
        .collect()
}
