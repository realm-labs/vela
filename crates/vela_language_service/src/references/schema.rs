use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};
use vela_common::SourceId;
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::SyntaxSourceFile;

use crate::{
    LanguageServiceDatabases, SymbolRef, TextRange, member_access, path_calls, query_context,
    symbol_ref::{
        schema_member_symbol as shared_schema_member_symbol,
        schema_variant_symbol as shared_schema_variant_symbol,
    },
};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, record_fields,
    record_variant_patterns, resolved_use_reference_kind, span_text_range, token_text,
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
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_schema_method_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        references.extend(schema_method_use_references_for_source(
            databases,
            databases.schema_db().facts(),
            source,
            target,
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
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_schema_field_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        references.extend(schema_field_use_references_for_source(
            databases,
            databases.schema_db().facts(),
            source,
            target,
        ));
        if let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) {
            references.extend(schema_record_field_references_for_source(
                databases.schema_db().facts(),
                parsed,
                source,
                target,
            ));
            references.extend(schema_record_variant_pattern_field_references_for_source(
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
            databases,
            databases.schema_db().facts(),
            source,
            target,
        ));
    }

    sort_references(&mut references);
    references
}

fn schema_field_symbol(target: &SchemaFieldReferenceTarget) -> SymbolRef {
    shared_schema_member_symbol(&target.owner, &target.field)
}

fn schema_method_symbol(target: &SchemaMethodReferenceTarget) -> SymbolRef {
    shared_schema_member_symbol(&target.owner, &target.method)
}

fn schema_variant_symbol(target: &SchemaVariantReferenceTarget) -> SymbolRef {
    shared_schema_variant_symbol(&target.owner, &target.variant)
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

pub(super) fn schema_variant_use_target(
    databases: &LanguageServiceDatabases,
    parsed: Option<&SyntaxParse<SyntaxSourceFile>>,
    _text: &str,
    token: &ReferenceToken,
) -> Option<SchemaVariantReferenceTarget> {
    let parsed = parsed?;
    for site in path_calls::path_expression_sites(parsed) {
        if site.segment_range == token.range {
            return schema_variant_target_for_path(databases.schema_db().facts(), &site.path);
        }
    }
    for site in path_calls::pattern_path_sites(parsed) {
        if site.segment_range == token.range {
            return schema_variant_target_for_path(databases.schema_db().facts(), &site.path);
        }
    }
    None
}

pub(super) fn schema_record_field_use_target(
    databases: &LanguageServiceDatabases,
    syntax_parse: Option<&SyntaxParse<SyntaxSourceFile>>,
    text: &str,
    token: &ReferenceToken,
) -> Option<SchemaFieldReferenceTarget> {
    let field = token_text(text, token.range)?;
    let parsed = syntax_parse?;
    record_fields::record_field_sites(parsed)
        .into_iter()
        .find(|site| {
            site.name == field
                && site.name_range.start <= token.range.start
                && token.range.end <= site.name_range.end
        })
        .and_then(|site| {
            schema_field_target_for_constructor_path(
                databases.schema_db().facts(),
                &site.path,
                field,
            )
        })
        .or_else(|| {
            schema_record_variant_pattern_field_use_target(
                databases.schema_db().facts(),
                parsed,
                token,
                field,
            )
        })
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
        symbol: schema_method_symbol(target),
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
        symbol: schema_field_symbol(target),
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
        symbol: schema_variant_symbol(target),
    })
}

fn schema_method_use_references_for_source(
    databases: &LanguageServiceDatabases,
    schema: &RegistryFacts,
    source: &crate::SourceRecord,
    target: &SchemaMethodReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let source_id = source.source_id();
    let text = source.text();
    let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) else {
        return references;
    };
    for site in member_access::member_call_sites(parsed) {
        if site.member != target.method {
            continue;
        }
        if query_context::type_fact_for_source_range(databases, source_id, site.receiver_range)
            .and_then(|receiver| {
                schema_method_target_for_receiver_fact(schema, &receiver, &target.method)
            })
            .as_ref()
            == Some(target)
        {
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, site.member_range),
                kind: ReferenceKind::Call,
                symbol: schema_method_symbol(target),
            });
        }
    }
    references
}

fn schema_field_use_references_for_source(
    databases: &LanguageServiceDatabases,
    schema: &RegistryFacts,
    source: &crate::SourceRecord,
    target: &SchemaFieldReferenceTarget,
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
                schema_field_target_for_receiver_fact(schema, &receiver, &target.field)
            })
            .as_ref()
            == Some(target)
        {
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, site.member_range),
                kind: resolved_use_reference_kind(text, site.member_range),
                symbol: schema_field_symbol(target),
            });
        }
    }
    references
}

fn schema_record_field_references_for_source(
    schema: &RegistryFacts,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    source: &crate::SourceRecord,
    target: &SchemaFieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    for field in record_fields::record_field_sites(parsed) {
        if field.name != target.field {
            continue;
        }
        if schema_field_target_for_constructor_path(schema, &field.path, &target.field).as_ref()
            != Some(target)
        {
            continue;
        };
        references.push(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(text, field.name_range),
            kind: ReferenceKind::Read,
            symbol: schema_field_symbol(target),
        });
    }
    references
}

fn schema_record_variant_pattern_field_references_for_source(
    schema: &RegistryFacts,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    source: &crate::SourceRecord,
    target: &SchemaFieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    for field in record_variant_patterns::record_pattern_field_sites(parsed) {
        if field.name != target.field {
            continue;
        }
        if schema_field_target_for_constructor_path(schema, &field.path, &target.field).as_ref()
            != Some(target)
        {
            continue;
        }
        references.push(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(text, field.name_range),
            kind: ReferenceKind::Pattern,
            symbol: schema_field_symbol(target),
        });
    }
    references
}

fn schema_variant_use_references_for_source(
    databases: &LanguageServiceDatabases,
    schema: &RegistryFacts,
    source: &crate::SourceRecord,
    target: &SchemaVariantReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    if let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) {
        for site in path_calls::path_expression_sites(parsed) {
            if site
                .path
                .last()
                .is_none_or(|segment| segment != &target.variant)
            {
                continue;
            }
            if schema_variant_target_for_path(schema, &site.path).as_ref() != Some(target) {
                continue;
            }
            let range = site.segment_range;
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, range),
                kind: schema_variant_reference_kind(text, range),
                symbol: schema_variant_symbol(target),
            });
        }
    }
    if let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) {
        for site in path_calls::pattern_path_sites(parsed) {
            if site
                .path
                .last()
                .is_none_or(|segment| segment != &target.variant)
            {
                continue;
            }
            if schema_variant_target_for_path(schema, &site.path).as_ref() != Some(target) {
                continue;
            }
            let range = site.segment_range;
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, range),
                kind: schema_variant_reference_kind(text, range),
                symbol: schema_variant_symbol(target),
            });
        }
    }
    references
}

fn schema_record_variant_pattern_field_use_target(
    schema: &RegistryFacts,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    token: &ReferenceToken,
    field: &str,
) -> Option<SchemaFieldReferenceTarget> {
    record_variant_patterns::record_pattern_field_sites(parsed)
        .into_iter()
        .find(|site| {
            site.name == field
                && site.name_range.start <= token.range.start
                && token.range.end <= site.name_range.end
        })
        .and_then(|site| schema_field_target_for_constructor_path(schema, &site.path, field))
}

pub(crate) fn schema_method_target_for_receiver_fact(
    schema: &RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<SchemaMethodReferenceTarget> {
    let (owner, kind) = schema_method_owner(schema, receiver, method)?;
    Some(SchemaMethodReferenceTarget {
        owner,
        method: method.to_owned(),
        kind,
    })
}

pub(super) fn schema_field_target_for_receiver_fact(
    schema: &RegistryFacts,
    receiver: &TypeFact,
    field: &str,
) -> Option<SchemaFieldReferenceTarget> {
    let owner = schema_field_owner(schema, receiver, field)?;
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
