use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::SyntaxSourceFile;

use crate::{LanguageServiceDatabases, member_access, query_context};

use super::{
    Reference, ReferenceKind, ReferenceToken, declaration_name_matches, diagnostic_range,
    name_range_in_text, record_fields, record_owner_names, resolved_use_reference_kind,
    source_member_symbol, span_text_range, token_text,
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
        if let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) {
            references.extend(script_record_field_references_for_source(
                graph, parsed, source, target,
            ));
        }
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
    text: &str,
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
            let span_range = span_text_range(field.span)?;
            let name_range = name_range_in_text(text, span_range, &field.name)?;
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
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
    let owner = script_field_owner(graph, receiver, field)?;
    Some(FieldReferenceTarget {
        owner,
        field: field.to_owned(),
    })
}

pub(super) fn script_record_field_use_target(
    graph: &ModuleGraph,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    text: &str,
    token: &ReferenceToken,
) -> Option<FieldReferenceTarget> {
    let field = token_text(text, token.range)?;
    record_fields::record_field_sites(parsed)
        .into_iter()
        .find(|site| {
            site.name == field
                && site.name_range.start <= token.range.start
                && token.range.end <= site.name_range.end
        })
        .and_then(|site| script_field_target_for_constructor_path(graph, &site.path, field))
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
    let span_range = span_text_range(field.span)?;
    let name_range =
        name_range_in_text(source.text(), span_range, &field.name).unwrap_or(span_range);
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
    let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) else {
        return references;
    };
    for site in member_access::member_access_sites(parsed) {
        if site.member != target.field {
            continue;
        }
        if query_context::type_fact_for_source_range(databases, source_id, site.receiver_range)
            .and_then(|receiver| {
                script_field_target_for_receiver_fact(graph, &receiver, &target.field)
            })
            .as_ref()
            == Some(target)
        {
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, site.member_range),
                kind: resolved_use_reference_kind(text, site.member_range),
                symbol: source_member_symbol(graph, target.owner, &target.field)
                    .expect("field target should have a source symbol"),
            });
        }
    }
    references
}

fn script_record_field_references_for_source(
    graph: &ModuleGraph,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    source: &crate::SourceRecord,
    target: &FieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    for field in record_fields::record_field_sites(parsed) {
        if field.name != target.field {
            continue;
        }
        if script_field_target_for_constructor_path(graph, &field.path, &target.field).as_ref()
            != Some(target)
        {
            continue;
        };
        references.push(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(text, field.name_range),
            kind: ReferenceKind::Read,
            symbol: source_member_symbol(graph, target.owner, &target.field)
                .expect("field target should have a source symbol"),
        });
    }
    references
}

fn script_field_target_for_constructor_path(
    graph: &ModuleGraph,
    path: &[String],
    field: &str,
) -> Option<FieldReferenceTarget> {
    let owner = graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct
            || !constructor_path_matches(graph, declaration, path)
        {
            return None;
        }
        let has_field = graph
            .struct_shape(declaration.id)
            .is_some_and(|shape| shape.fields.iter().any(|entry| entry.name == field));
        has_field.then_some(declaration.id)
    })?;
    Some(FieldReferenceTarget {
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

fn constructor_path_matches(
    graph: &ModuleGraph,
    declaration: &Declaration,
    path: &[String],
) -> bool {
    match path {
        [name] => declaration_name_matches(graph, declaration, name),
        segments => graph
            .module_path(declaration.module)
            .is_some_and(|module_path| {
                module_path
                    .segments()
                    .iter()
                    .chain(std::iter::once(&declaration.name))
                    .eq(segments.iter())
            }),
    }
}
