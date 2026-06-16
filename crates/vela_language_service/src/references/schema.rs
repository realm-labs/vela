use vela_analysis::{facts::AnalysisFacts, registry::RegistryFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_syntax::ast::SourceFile;
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{LanguageServiceDatabases, TextRange};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, is_call_callee,
    member_receiver_range, name_range_in_text, path_ending_at, record_fields,
    resolved_use_reference_kind, span_text_range, token_text, type_fact_for_resolution,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SchemaFieldReferenceTarget {
    owner: String,
    field: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SchemaVariantReferenceTarget {
    owner: String,
    variant: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SchemaMethodReferenceTarget {
    pub(crate) owner: String,
    pub(crate) method: String,
    pub(crate) kind: SchemaMethodReferenceKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SchemaMethodReferenceKind {
    Method,
    TraitMethod,
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
        if let Some(parsed) = databases.parse_db().parsed_source(source.document_id()) {
            references.extend(schema_record_field_references_for_source(
                databases.schema_db().facts(),
                parsed,
                source,
                target,
            ));
        }
    }

    sort_references(&mut references);
    references
}

pub(super) fn schema_variant_references(
    databases: &LanguageServiceDatabases,
    target: &SchemaVariantReferenceTarget,
    include_declaration: bool,
) -> Vec<Reference> {
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_schema_variant_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        references.extend(schema_variant_use_references_for_source(
            databases.schema_db().facts(),
            source,
            target,
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
                kind: SchemaMethodReferenceKind::Method,
            });
        }
    }
    for method in facts.trait_methods() {
        let Some(span) = locations.trait_method_span(&method.owner, &method.name) else {
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
                kind: SchemaMethodReferenceKind::TraitMethod,
            });
        }
    }
    None
}

pub(super) fn schema_variant_declaration_target(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    token: &ReferenceToken,
) -> Option<SchemaVariantReferenceTarget> {
    let locations = databases.schema_db().source_locations();
    let facts = databases.schema_db().facts();
    for variant in facts.variants() {
        let Some(span) = locations.variant_span(&variant.owner, &variant.name) else {
            continue;
        };
        if span.source != source_id {
            continue;
        }
        let range = span_text_range(span)?;
        if range.start <= token.range.start && token.range.end <= range.end {
            return Some(SchemaVariantReferenceTarget {
                owner: variant.owner,
                variant: variant.name,
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

pub(super) fn schema_variant_use_target(
    databases: &LanguageServiceDatabases,
    text: &str,
    token: &ReferenceToken,
) -> Option<SchemaVariantReferenceTarget> {
    let path = path_ending_at(text, token.range)?;
    schema_variant_target_for_path(databases.schema_db().facts(), &path)
}

pub(super) fn schema_record_field_use_target(
    databases: &LanguageServiceDatabases,
    parsed: &SourceFile,
    text: &str,
    token: &ReferenceToken,
) -> Option<SchemaFieldReferenceTarget> {
    let field = token_text(text, token.range)?;
    let mut target = None;
    record_fields::for_each_explicit_record_field(parsed, |path, record_field| {
        if target.is_some() || record_field.name != field {
            return;
        }
        let Some(span_range) = span_text_range(record_field.span) else {
            return;
        };
        let Some(name_range) = name_range_in_text(text, span_range, &record_field.name) else {
            return;
        };
        if name_range.start <= token.range.start && token.range.end <= name_range.end {
            target = schema_field_target_for_constructor_path(
                databases.schema_db().facts(),
                path,
                field,
            );
        }
    });
    target
}

fn reference_for_schema_method_declaration(
    databases: &LanguageServiceDatabases,
    target: &SchemaMethodReferenceTarget,
) -> Option<Reference> {
    let span = databases
        .schema_db()
        .source_locations()
        .method_span(&target.owner, &target.method)
        .filter(|_| target.kind == SchemaMethodReferenceKind::Method)
        .or_else(|| {
            (target.kind == SchemaMethodReferenceKind::TraitMethod)
                .then(|| {
                    databases
                        .schema_db()
                        .source_locations()
                        .trait_method_span(&target.owner, &target.method)
                })
                .flatten()
        })?;
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

fn reference_for_schema_variant_declaration(
    databases: &LanguageServiceDatabases,
    target: &SchemaVariantReferenceTarget,
) -> Option<Reference> {
    let span = databases
        .schema_db()
        .source_locations()
        .variant_span(&target.owner, &target.variant)?;
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

fn schema_record_field_references_for_source(
    schema: &RegistryFacts,
    parsed: &SourceFile,
    source: &crate::SourceRecord,
    target: &SchemaFieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    record_fields::for_each_explicit_record_field(parsed, |path, field| {
        if field.name != target.field {
            return;
        }
        if schema_field_target_for_constructor_path(schema, path, &target.field).as_ref()
            != Some(target)
        {
            return;
        }
        let Some(span_range) = span_text_range(field.span) else {
            return;
        };
        let Some(name_range) = name_range_in_text(text, span_range, &field.name) else {
            return;
        };
        references.push(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(text, name_range),
            kind: ReferenceKind::Read,
        });
    });
    references
}

fn schema_variant_use_references_for_source(
    schema: &RegistryFacts,
    source: &crate::SourceRecord,
    target: &SchemaVariantReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let source_id = source.source_id();
    let text = source.text();
    for range in schema_variant_ranges(source_id, text, &target.variant) {
        if schema_variant_target_for_path_range(schema, text, range).as_ref() == Some(target) {
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, range),
                kind: schema_variant_reference_kind(text, range),
            });
        }
    }
    references
}

pub(crate) fn schema_method_target_for_member(
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
    let (owner, kind) = schema_method_owner(schema, &receiver, method)?;
    Some(SchemaMethodReferenceTarget {
        owner,
        method: method.to_owned(),
        kind,
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

fn schema_field_target_for_constructor_path(
    schema: &RegistryFacts,
    path: &[String],
    field: &str,
) -> Option<SchemaFieldReferenceTarget> {
    let mut owners = schema_constructor_owner_candidates(schema, path, field).into_iter();
    let owner = owners.next()?;
    owners
        .next()
        .is_none()
        .then_some(SchemaFieldReferenceTarget {
            owner,
            field: field.to_owned(),
        })
}

fn schema_constructor_owner_candidates(
    schema: &RegistryFacts,
    path: &[String],
    field: &str,
) -> Vec<String> {
    let qualified = path.join("::");
    let short = path.last().cloned();
    schema
        .fields()
        .filter_map(move |candidate| {
            if candidate.name != field {
                return None;
            }
            let exact = candidate.owner == qualified;
            let short_match = short
                .as_deref()
                .is_some_and(|name| candidate.owner.rsplit("::").next() == Some(name));
            (exact || short_match).then_some(candidate.owner)
        })
        .collect()
}

fn schema_variant_target_for_path(
    schema: &RegistryFacts,
    path: &[String],
) -> Option<SchemaVariantReferenceTarget> {
    let (variant, owner_segments) = path.split_last()?;
    if owner_segments.is_empty() {
        return None;
    }
    let owner = owner_segments.join("::");
    if schema.variant_fact(&owner, variant).is_some() {
        return Some(SchemaVariantReferenceTarget {
            owner,
            variant: variant.clone(),
        });
    }

    if owner.contains("::") {
        return None;
    }

    let mut matches = schema.variants().filter_map(|candidate| {
        (candidate.name == *variant
            && candidate
                .owner
                .rsplit("::")
                .next()
                .is_some_and(|short| short == owner))
        .then_some(candidate.owner)
    });
    let matched_owner = matches.next()?;
    matches
        .next()
        .is_none()
        .then_some(SchemaVariantReferenceTarget {
            owner: matched_owner,
            variant: variant.clone(),
        })
}

fn schema_variant_target_for_path_range(
    schema: &RegistryFacts,
    text: &str,
    range: TextRange,
) -> Option<SchemaVariantReferenceTarget> {
    let path = path_ending_at(text, range)?;
    schema_variant_target_for_path(schema, &path)
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
        .or_else(|| schema.trait_fact(&qualified))
        .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
        .or_else(|| hint.path.last().and_then(|name| schema.trait_fact(name)))
        .cloned()
}

fn schema_method_owner(
    schema: &RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<(String, SchemaMethodReferenceKind)> {
    owner_names(receiver).into_iter().find_map(|owner| {
        if schema.method_fact(&owner, method).is_some() {
            Some((owner, SchemaMethodReferenceKind::Method))
        } else if schema.trait_method_fact(&owner, method).is_some() {
            Some((owner, SchemaMethodReferenceKind::TraitMethod))
        } else {
            None
        }
    })
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

fn schema_variant_ranges(source_id: SourceId, text: &str, variant: &str) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) if name == variant => {
                let range = span_text_range(token.span)?;
                path_ending_at(text, range).map(|_| range)
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

fn schema_variant_reference_kind(text: &str, range: TextRange) -> ReferenceKind {
    let line_end = text
        .get(range.end..)
        .and_then(|suffix| suffix.find('\n').map(|end| range.end + end))
        .unwrap_or(text.len());
    if text
        .get(range.end..line_end)
        .is_some_and(|suffix| suffix.contains("=>"))
    {
        ReferenceKind::Pattern
    } else {
        ReferenceKind::Read
    }
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
