use vela_analysis::{facts::AnalysisFacts, registry::RegistryFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{LanguageServiceDatabases, TextRange};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, is_call_callee,
    member_receiver_range, resolved_use_reference_kind, span_text_range, token_text,
    type_fact_for_resolution,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SchemaFieldReferenceTarget {
    owner: String,
    field: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SchemaMethodReferenceTarget {
    owner: String,
    method: String,
}

pub(super) fn schema_method_references(
    databases: &LanguageServiceDatabases,
    target: &SchemaMethodReferenceTarget,
    include_declaration: bool,
) -> Vec<Reference> {
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_schema_method_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        references.extend(schema_method_use_references_for_source(
            databases.schema_db().facts(),
            &facts,
            source,
            target,
            graph,
        ));
    }

    sort_references(&mut references);
    references
}

pub(super) fn schema_field_references(
    databases: &LanguageServiceDatabases,
    target: &SchemaFieldReferenceTarget,
    include_declaration: bool,
) -> Vec<Reference> {
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_schema_field_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        references.extend(schema_field_use_references_for_source(
            databases.schema_db().facts(),
            &facts,
            source,
            target,
            graph,
        ));
    }

    sort_references(&mut references);
    references
}

pub(super) fn schema_method_declaration_target(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    token: &ReferenceToken,
) -> Option<SchemaMethodReferenceTarget> {
    let locations = databases.schema_db().source_locations();
    let facts = databases.schema_db().facts();
    for method in facts.methods() {
        let Some(span) = locations.method_span(&method.owner, &method.name) else {
            continue;
        };
        if span.source != source_id {
            continue;
        }
        let range = span_text_range(span)?;
        if range.start <= token.range.start && token.range.end <= range.end {
            return Some(SchemaMethodReferenceTarget {
                owner: method.owner,
                method: method.name,
            });
        }
    }
    None
}

pub(super) fn schema_field_declaration_target(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    token: &ReferenceToken,
) -> Option<SchemaFieldReferenceTarget> {
    let locations = databases.schema_db().source_locations();
    let facts = databases.schema_db().facts();
    for field in facts.fields() {
        let Some(span) = locations.field_span(&field.owner, &field.name) else {
            continue;
        };
        if span.source != source_id {
            continue;
        }
        let range = span_text_range(span)?;
        if range.start <= token.range.start && token.range.end <= range.end {
            return Some(SchemaFieldReferenceTarget {
                owner: field.owner,
                field: field.name,
            });
        }
    }
    None
}

pub(super) fn schema_method_use_target(
    databases: &LanguageServiceDatabases,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    token: &ReferenceToken,
) -> Option<SchemaMethodReferenceTarget> {
    let method = token_text(text, token.range)?;
    if !is_call_callee(text, token.range) {
        return None;
    }
    schema_method_target_for_member(
        databases.schema_db().facts(),
        facts,
        text,
        source_id,
        bindings,
        method,
        token.range,
    )
}

pub(super) fn schema_field_use_target(
    databases: &LanguageServiceDatabases,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    token: &ReferenceToken,
) -> Option<SchemaFieldReferenceTarget> {
    let field = token_text(text, token.range)?;
    schema_field_target_for_member(
        databases.schema_db().facts(),
        facts,
        text,
        source_id,
        bindings,
        field,
        token.range,
    )
}

fn reference_for_schema_method_declaration(
    databases: &LanguageServiceDatabases,
    target: &SchemaMethodReferenceTarget,
) -> Option<Reference> {
    let span = databases
        .schema_db()
        .source_locations()
        .method_span(&target.owner, &target.method)?;
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == span.source)?;
    let range = span_text_range(span)?;
    Some(Reference {
        document_id: source.document_id().clone(),
        range: diagnostic_range(source.text(), range),
        kind: ReferenceKind::Declaration,
    })
}

fn reference_for_schema_field_declaration(
    databases: &LanguageServiceDatabases,
    target: &SchemaFieldReferenceTarget,
) -> Option<Reference> {
    let span = databases
        .schema_db()
        .source_locations()
        .field_span(&target.owner, &target.field)?;
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == span.source)?;
    let range = span_text_range(span)?;
    Some(Reference {
        document_id: source.document_id().clone(),
        range: diagnostic_range(source.text(), range),
        kind: ReferenceKind::Declaration,
    })
}

fn schema_method_use_references_for_source(
    schema: &RegistryFacts,
    facts: &AnalysisFacts,
    source: &crate::SourceRecord,
    target: &SchemaMethodReferenceTarget,
    graph: &vela_hir::module_graph::ModuleGraph,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let source_id = source.source_id();
    let text = source.text();
    for range in member_method_ranges(source_id, text, &target.method) {
        let Some(start) = u32::try_from(range.start).ok() else {
            continue;
        };
        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(start) {
                continue;
            }
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            if schema_method_target_for_member(
                schema,
                facts,
                text,
                source_id,
                bindings,
                &target.method,
                range,
            )
            .as_ref()
                == Some(target)
            {
                references.push(Reference {
                    document_id: source.document_id().clone(),
                    range: diagnostic_range(text, range),
                    kind: ReferenceKind::Call,
                });
                break;
            }
        }
    }
    references
}

fn schema_field_use_references_for_source(
    schema: &RegistryFacts,
    facts: &AnalysisFacts,
    source: &crate::SourceRecord,
    target: &SchemaFieldReferenceTarget,
    graph: &vela_hir::module_graph::ModuleGraph,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let source_id = source.source_id();
    let text = source.text();
    for range in member_field_ranges(source_id, text, &target.field) {
        let Some(start) = u32::try_from(range.start).ok() else {
            continue;
        };
        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(start) {
                continue;
            }
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            if schema_field_target_for_member(
                schema,
                facts,
                text,
                source_id,
                bindings,
                &target.field,
                range,
            )
            .as_ref()
                == Some(target)
            {
                references.push(Reference {
                    document_id: source.document_id().clone(),
                    range: diagnostic_range(text, range),
                    kind: resolved_use_reference_kind(text, range),
                });
                break;
            }
        }
    }
    references
}

fn schema_method_target_for_member(
    schema: &RegistryFacts,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    method: &str,
    member_range: TextRange,
) -> Option<SchemaMethodReferenceTarget> {
    let receiver = member_receiver_range(text, member_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = schema_type_fact_for_resolution(resolution, bindings, facts, schema)?;
    let owner = schema_method_owner(schema, &receiver, method)?;
    Some(SchemaMethodReferenceTarget {
        owner,
        method: method.to_owned(),
    })
}

fn schema_field_target_for_member(
    schema: &RegistryFacts,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    field: &str,
    member_range: TextRange,
) -> Option<SchemaFieldReferenceTarget> {
    let receiver = member_receiver_range(text, member_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = schema_type_fact_for_resolution(resolution, bindings, facts, schema)?;
    let owner = schema_field_owner(schema, &receiver, field)?;
    Some(SchemaFieldReferenceTarget {
        owner,
        field: field.to_owned(),
    })
}

fn schema_type_fact_for_resolution(
    resolution: &BindingResolution,
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    schema: &RegistryFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            type_fact_for_resolution(resolution, facts)
                .or_else(|| schema_fact_for_hint(binding.type_hint.as_ref(), schema))
        }
        BindingResolution::Declaration(_) => type_fact_for_resolution(resolution, facts),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn schema_fact_for_hint(
    hint: Option<&vela_hir::type_hint::HirTypeHint>,
    schema: &RegistryFacts,
) -> Option<TypeFact> {
    let hint = hint?;
    if !hint.args.is_empty() {
        return None;
    }
    let qualified = hint.path.join("::");
    schema
        .type_fact(&qualified)
        .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
        .cloned()
}

fn schema_method_owner(
    schema: &RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<String> {
    owner_names(receiver)
        .into_iter()
        .find(|owner| schema.method_fact(owner, method).is_some())
}

fn schema_field_owner(schema: &RegistryFacts, receiver: &TypeFact, field: &str) -> Option<String> {
    owner_names(receiver)
        .into_iter()
        .find(|owner| schema.field_fact(owner, field).is_some())
}

fn owner_names(receiver: &TypeFact) -> Vec<String> {
    let Some(owner) = receiver_owner_name(receiver) else {
        return Vec::new();
    };
    let mut names = vec![owner.clone()];
    if let Some(short) = owner.rsplit("::").next()
        && short != owner
    {
        names.push(short.to_owned());
    }
    names
}

fn receiver_owner_name(receiver: &TypeFact) -> Option<String> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } | TypeFact::Trait { name } => {
            Some(name.clone())
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => Some(format!("{name}::{variant}")),
        TypeFact::Enum {
            name,
            variant: None,
        } => Some(name.clone()),
        _ => None,
    }
}

fn member_method_ranges(source_id: SourceId, text: &str, method: &str) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) if name == method => {
                let range = span_text_range(token.span)?;
                (is_call_callee(text, range) && member_receiver_range(text, range.start).is_some())
                    .then_some(range)
            }
            TokenKind::Ident(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Char(_)
            | TokenKind::String(_)
            | TokenKind::InterpolatedString(_)
            | TokenKind::Bytes(_)
            | TokenKind::Keyword(_)
            | TokenKind::Symbol(_)
            | TokenKind::Eof => None,
        })
        .collect()
}

fn member_field_ranges(source_id: SourceId, text: &str, field: &str) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) if name == field => {
                let range = span_text_range(token.span)?;
                member_receiver_range(text, range.start).map(|_| range)
            }
            TokenKind::Ident(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Char(_)
            | TokenKind::String(_)
            | TokenKind::InterpolatedString(_)
            | TokenKind::Bytes(_)
            | TokenKind::Keyword(_)
            | TokenKind::Symbol(_)
            | TokenKind::Eof => None,
        })
        .collect()
}

fn sort_references(references: &mut [Reference]) {
    references.sort_by_key(|reference| {
        let start = reference.range().start();
        (
            reference.document_id().as_str().to_owned(),
            start.line,
            start.character,
            reference.kind(),
        )
    });
}
