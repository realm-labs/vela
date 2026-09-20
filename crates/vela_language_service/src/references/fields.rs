use crate::source_record_fields::RecordOwner;
use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::module_graph::ModuleGraph;

use crate::{LanguageServiceDatabases, query_context};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, resolved_use_reference_kind,
    span_text_range,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FieldReferenceTarget {
    owner: RecordOwner,
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
    let (owner, field) =
        crate::source_record_fields::declaration_target(graph, source_id, token.range)?;
    Some(FieldReferenceTarget { owner, field })
}

pub(super) fn script_field_target_for_receiver_fact(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    field: &str,
) -> Option<FieldReferenceTarget> {
    let owner = crate::source_record_fields::receiver_owner(graph, receiver, field)?;
    Some(FieldReferenceTarget {
        owner: RecordOwner {
            declaration: owner,
            variant: None,
        },
        field: field.to_owned(),
    })
}

pub(super) fn script_record_field_use_target(
    databases: &LanguageServiceDatabases,
    source: &crate::SourceRecord,
    token: &ReferenceToken,
) -> Option<Option<FieldReferenceTarget>> {
    let site = crate::source_record_fields::explicit_target(databases, source, token.range)?;
    let exists = site
        .owner
        .fields(databases.hir_db().graph())
        .is_some_and(|fields| fields.iter().any(|field| field.name == site.name));
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
    let field = target
        .owner
        .fields(graph)?
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
        symbol: target.owner.symbol(graph, &target.field)?,
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
                symbol: target
                    .owner
                    .symbol(graph, &target.field)
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
            kind: if site.pattern && site.owner.variant.is_some() {
                ReferenceKind::Pattern
            } else {
                ReferenceKind::Read
            },
            symbol: target
                .owner
                .symbol(databases.hir_db().graph(), &target.field)
                .expect("source field"),
        })
        .collect()
}
