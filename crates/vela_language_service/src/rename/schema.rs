use std::collections::BTreeMap;

use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::module_graph::Declaration;
use vela_hir::type_hint::HirTypeHint;

use crate::{
    DocumentId, LanguageServiceDatabases, QueryContext, TextRange, member_access, path_calls,
    query_context,
};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, document_text_edit_for_rename,
    span_text_range, token_text, type_hint_name_range,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum SchemaMemberRenameKind {
    Field,
    Method,
    TraitMethod,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SchemaMemberRenameTarget {
    pub(super) owner: String,
    pub(super) member: String,
    pub(super) kind: SchemaMemberRenameKind,
    pub(super) token: RenameToken,
}

struct SchemaMemberSite {
    member_range: TextRange,
    receiver_range: TextRange,
    is_call: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum SchemaTypeRenameKind {
    Type,
    Trait,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SchemaTypeRenameTarget {
    pub(super) name: String,
    pub(super) kind: SchemaTypeRenameKind,
    pub(super) token: RenameToken,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SchemaFunctionRenameTarget {
    pub(super) name: String,
    pub(super) token: RenameToken,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SchemaVariantRenameTarget {
    pub(super) owner: String,
    pub(super) variant: String,
    pub(super) token: RenameToken,
}

pub(super) fn rename_schema_function(
    databases: &LanguageServiceDatabases,
    target: SchemaFunctionRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    if schema_function_name_conflicts(databases.schema_db().facts(), &target, new_name) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_schema_function_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_schema_function_use_edits(databases, &target, new_name, &mut edits_by_document);

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
    })
}

pub(super) fn rename_schema_variant(
    databases: &LanguageServiceDatabases,
    target: SchemaVariantRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    if schema_variant_name_conflicts(databases.schema_db().facts(), &target, new_name) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_schema_variant_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_schema_variant_use_edits(databases, &target, new_name, &mut edits_by_document);

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
    })
}

pub(super) fn rename_schema_type(
    databases: &LanguageServiceDatabases,
    target: SchemaTypeRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    if schema_type_name_conflicts(databases.schema_db().facts(), &target, new_name) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_schema_type_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_schema_type_hint_edits(databases, &target, new_name, &mut edits_by_document);

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
    })
}

pub(super) fn rename_schema_member(
    databases: &LanguageServiceDatabases,
    target: SchemaMemberRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    if schema_member_name_conflicts(databases.schema_db().facts(), &target, new_name) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_schema_member_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_schema_member_use_edits(databases, &target, new_name, &mut edits_by_document);

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
    })
}

pub(super) fn schema_type_declaration_target(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    token: &RenameToken,
) -> Option<SchemaTypeRenameTarget> {
    let locations = databases.schema_db().source_locations();
    let facts = databases.schema_db().facts();

    for (name, _) in facts.types() {
        let Some(span) = locations.type_span(name) else {
            continue;
        };
        if source_span_contains_token(span, source_id, token) {
            return Some(SchemaTypeRenameTarget {
                name: name.to_owned(),
                kind: SchemaTypeRenameKind::Type,
                token: token.clone(),
            });
        }
    }

    for (name, _) in facts.traits() {
        let Some(span) = locations.trait_span(name) else {
            continue;
        };
        if source_span_contains_token(span, source_id, token) {
            return Some(SchemaTypeRenameTarget {
                name: name.to_owned(),
                kind: SchemaTypeRenameKind::Trait,
                token: token.clone(),
            });
        }
    }

    None
}

pub(super) fn schema_function_declaration_target(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    token: &RenameToken,
) -> Option<SchemaFunctionRenameTarget> {
    let locations = databases.schema_db().source_locations();
    let facts = databases.schema_db().facts();

    for function in facts.functions() {
        let Some(span) = locations.function_span(&function.name) else {
            continue;
        };
        if source_span_contains_token(span, source_id, token) {
            return Some(SchemaFunctionRenameTarget {
                name: function.name,
                token: token.clone(),
            });
        }
    }

    None
}

pub(super) fn schema_variant_declaration_target(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    token: &RenameToken,
) -> Option<SchemaVariantRenameTarget> {
    let locations = databases.schema_db().source_locations();
    let facts = databases.schema_db().facts();

    for variant in facts.variants() {
        let Some(span) = locations.variant_span(&variant.owner, &variant.name) else {
            continue;
        };
        if source_span_contains_token(span, source_id, token) {
            return Some(SchemaVariantRenameTarget {
                owner: variant.owner,
                variant: variant.name,
                token: token.clone(),
            });
        }
    }

    None
}

pub(super) fn schema_member_declaration_target(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    token: &RenameToken,
) -> Option<SchemaMemberRenameTarget> {
    let locations = databases.schema_db().source_locations();
    let facts = databases.schema_db().facts();

    for field in facts.fields() {
        let Some(span) = locations.field_span(&field.owner, &field.name) else {
            continue;
        };
        if source_span_contains_token(span, source_id, token) {
            return Some(SchemaMemberRenameTarget {
                owner: field.owner,
                member: field.name,
                kind: SchemaMemberRenameKind::Field,
                token: token.clone(),
            });
        }
    }

    for method in facts.methods() {
        let Some(span) = locations.method_span(&method.owner, &method.name) else {
            continue;
        };
        if source_span_contains_token(span, source_id, token) {
            return Some(SchemaMemberRenameTarget {
                owner: method.owner,
                member: method.name,
                kind: SchemaMemberRenameKind::Method,
                token: token.clone(),
            });
        }
    }

    for method in facts.trait_methods() {
        let Some(span) = locations.trait_method_span(&method.owner, &method.name) else {
            continue;
        };
        if source_span_contains_token(span, source_id, token) {
            return Some(SchemaMemberRenameTarget {
                owner: method.owner,
                member: method.name,
                kind: SchemaMemberRenameKind::TraitMethod,
                token: token.clone(),
            });
        }
    }

    None
}

pub(super) fn schema_type_use_target(
    databases: &LanguageServiceDatabases,
    owner: &Declaration,
    text: &str,
    token: &RenameToken,
) -> Option<SchemaTypeRenameTarget> {
    let graph = databases.hir_db().graph();
    let mut target = None;
    super::for_each_type_hint_in_declaration(graph, owner, |hint| {
        if target.is_none() {
            target = schema_type_target_for_hint_at_token(databases, text, hint, token);
        }
    });
    target
}

pub(super) fn schema_function_use_target(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    text: &str,
    token: &RenameToken,
) -> Option<SchemaFunctionRenameTarget> {
    if let Some(source) = query.source_record()
        && let Some(parsed) = databases.parse_db().parsed_source(source.document_id())
    {
        for site in path_calls::path_call_sites(parsed, text) {
            if site.segment_range != token.range {
                continue;
            }
            let callee = site.path.join("::");
            let target = schema_function_target_for_name(databases, &callee)?;
            return source_backed_schema_function_target(databases, target).map(|mut target| {
                target.token = token.clone();
                target
            });
        }
    }
    let callee = function_call_name_at(text, token.range)?;
    let target = schema_function_target_for_name(databases, &callee)?;
    source_backed_schema_function_target(databases, target).map(|mut target| {
        target.token = token.clone();
        target
    })
}

pub(super) fn schema_variant_use_target(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    text: &str,
    token: &RenameToken,
) -> Option<SchemaVariantRenameTarget> {
    if let Some(source) = query.source_record()
        && let Some(parsed) = databases.parse_db().parsed_source(source.document_id())
    {
        for site in path_calls::path_expression_sites(parsed, text) {
            if site.segment_range != token.range {
                continue;
            }
            let target = schema_variant_target_for_path(databases, &site.path)?;
            return source_backed_schema_variant_target(databases, target).map(|mut target| {
                target.token = token.clone();
                target
            });
        }
    }
    schema_variant_use_target_for_range(databases, text, token.range).map(|mut target| {
        target.token = token.clone();
        target
    })
}

pub(super) fn schema_member_target_for_receiver_fact(
    databases: &LanguageServiceDatabases,
    receiver: &TypeFact,
    member: &str,
    is_call: bool,
    token: &RenameToken,
) -> Option<SchemaMemberRenameTarget> {
    let schema = databases.schema_db().facts();
    let target = if is_call {
        let (owner, kind) = schema_method_owner(schema, receiver, member)?;
        SchemaMemberRenameTarget {
            owner,
            member: member.to_owned(),
            kind,
            token: token.clone(),
        }
    } else {
        SchemaMemberRenameTarget {
            owner: schema_field_owner(schema, receiver, member)?,
            member: member.to_owned(),
            kind: SchemaMemberRenameKind::Field,
            token: token.clone(),
        }
    };
    source_backed_schema_target(databases, target)
}

fn push_schema_type_declaration_edit(
    databases: &LanguageServiceDatabases,
    target: &SchemaTypeRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    let span = schema_type_span(databases, target)?;
    let source = databases.source_record_for_rename(span.source)?;
    let range = span_text_range(span)?;
    edits_by_document
        .entry(source.document_id().clone())
        .or_default()
        .push(TextEdit {
            range: diagnostic_range(source.text(), range),
            new_text: new_name.to_owned(),
        });
    Some(())
}

fn push_schema_function_declaration_edit(
    databases: &LanguageServiceDatabases,
    target: &SchemaFunctionRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    let span = schema_function_span(databases, target)?;
    let source = databases.source_record_for_rename(span.source)?;
    let range = span_text_range(span)?;
    edits_by_document
        .entry(source.document_id().clone())
        .or_default()
        .push(TextEdit {
            range: diagnostic_range(source.text(), range),
            new_text: new_name.to_owned(),
        });
    Some(())
}

fn push_schema_member_declaration_edit(
    databases: &LanguageServiceDatabases,
    target: &SchemaMemberRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    let span = schema_member_span(databases, target)?;
    let source = databases.source_record_for_rename(span.source)?;
    let range = span_text_range(span)?;
    edits_by_document
        .entry(source.document_id().clone())
        .or_default()
        .push(TextEdit {
            range: diagnostic_range(source.text(), range),
            new_text: new_name.to_owned(),
        });
    Some(())
}

fn push_schema_variant_declaration_edit(
    databases: &LanguageServiceDatabases,
    target: &SchemaVariantRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    let span = schema_variant_span(databases, target)?;
    let source = databases.source_record_for_rename(span.source)?;
    let range = span_text_range(span)?;
    edits_by_document
        .entry(source.document_id().clone())
        .or_default()
        .push(TextEdit {
            range: diagnostic_range(source.text(), range),
            new_text: new_name.to_owned(),
        });
    Some(())
}

fn push_schema_type_hint_edits(
    databases: &LanguageServiceDatabases,
    target: &SchemaTypeRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let graph = databases.hir_db().graph();
    for owner in graph.declarations() {
        super::for_each_type_hint_in_declaration(graph, owner, |hint| {
            if schema_type_target_for_hint(databases, hint)
                .is_some_and(|found| found.name == target.name && found.kind == target.kind)
                && let Some(source) = databases.source_record_for_rename(hint.span.source)
                && let Some(range) = schema_type_hint_name_range(source.text(), hint, target)
            {
                edits_by_document
                    .entry(source.document_id().clone())
                    .or_default()
                    .push(TextEdit {
                        range: diagnostic_range(source.text(), range),
                        new_text: new_name.to_owned(),
                    });
            }
        });
    }
}

fn push_schema_function_use_edits(
    databases: &LanguageServiceDatabases,
    target: &SchemaFunctionRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let target_segment = schema_function_segment(&target.name);
    for source in databases.source_db().records().values() {
        let text = source.text();
        let Some(parsed) = databases.parse_db().parsed_source(source.document_id()) else {
            continue;
        };
        for site in path_calls::path_call_sites(parsed, text) {
            if site
                .path
                .last()
                .is_none_or(|segment| segment != target_segment)
            {
                continue;
            }
            if schema_function_target_for_name(databases, &site.path.join("::"))
                .is_some_and(|found| found.name == target.name)
            {
                edits_by_document
                    .entry(source.document_id().clone())
                    .or_default()
                    .push(TextEdit {
                        range: diagnostic_range(text, site.segment_range),
                        new_text: new_name.to_owned(),
                    });
            }
        }
    }
}

fn push_schema_variant_use_edits(
    databases: &LanguageServiceDatabases,
    target: &SchemaVariantRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    for source in databases.source_db().records().values() {
        let text = source.text();
        if let Some(parsed) = databases.parse_db().parsed_source(source.document_id()) {
            for site in path_calls::path_expression_sites(parsed, text) {
                if site
                    .path
                    .last()
                    .is_none_or(|segment| segment != &target.variant)
                {
                    continue;
                }
                let range = site.segment_range;
                push_schema_variant_use_edit_for_range(
                    databases,
                    source,
                    text,
                    range,
                    target,
                    new_name,
                    edits_by_document,
                );
            }
        }
        if let Some(parsed) = databases.parse_db().parsed_source(source.document_id()) {
            for site in path_calls::pattern_path_sites(parsed, text) {
                if site
                    .path
                    .last()
                    .is_none_or(|segment| segment != &target.variant)
                {
                    continue;
                }
                let range = site.segment_range;
                push_schema_variant_use_edit_for_range(
                    databases,
                    source,
                    text,
                    range,
                    target,
                    new_name,
                    edits_by_document,
                );
            }
        }
    }
}

fn push_schema_variant_use_edit_for_range(
    databases: &LanguageServiceDatabases,
    source: &crate::SourceRecord,
    text: &str,
    range: TextRange,
    target: &SchemaVariantRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    if schema_variant_use_target_for_range(databases, text, range)
        .is_some_and(|found| found.owner == target.owner && found.variant == target.variant)
    {
        edits_by_document
            .entry(source.document_id().clone())
            .or_default()
            .push(TextEdit {
                range: diagnostic_range(text, range),
                new_text: new_name.to_owned(),
            });
    }
}

fn schema_variant_use_target_for_range(
    databases: &LanguageServiceDatabases,
    text: &str,
    range: TextRange,
) -> Option<SchemaVariantRenameTarget> {
    let path = path_ending_at(text, range)?;
    let target = schema_variant_target_for_path(databases, &path)?;
    source_backed_schema_variant_target(databases, target)
}

fn push_schema_member_use_edits(
    databases: &LanguageServiceDatabases,
    target: &SchemaMemberRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    for source in databases.source_db().records().values() {
        let Some(parsed) = databases.parse_db().parsed_source(source.document_id()) else {
            continue;
        };
        match target.kind {
            SchemaMemberRenameKind::Field => {
                for site in member_access::member_access_sites(parsed) {
                    if site.member != target.member {
                        continue;
                    }
                    push_schema_member_site_edit(
                        databases,
                        source,
                        SchemaMemberSite {
                            member_range: site.member_range,
                            receiver_range: site.receiver_range,
                            is_call: false,
                        },
                        target,
                        new_name,
                        edits_by_document,
                    );
                }
            }
            SchemaMemberRenameKind::Method | SchemaMemberRenameKind::TraitMethod => {
                for site in member_access::member_call_sites(parsed) {
                    if site.member != target.member {
                        continue;
                    }
                    push_schema_member_site_edit(
                        databases,
                        source,
                        SchemaMemberSite {
                            member_range: site.member_range,
                            receiver_range: site.receiver_range,
                            is_call: true,
                        },
                        target,
                        new_name,
                        edits_by_document,
                    );
                }
            }
        }
    }
}

fn push_schema_member_site_edit(
    databases: &LanguageServiceDatabases,
    source: &crate::SourceRecord,
    site: SchemaMemberSite,
    target: &SchemaMemberRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let Some(receiver) = query_context::type_fact_for_source_range(
        databases,
        source.source_id(),
        site.receiver_range,
    ) else {
        return;
    };
    if schema_member_target_for_receiver_fact(
        databases,
        &receiver,
        &target.member,
        site.is_call,
        &RenameToken {
            range: site.member_range,
        },
    )
    .is_some_and(|found| {
        found.owner == target.owner && found.member == target.member && found.kind == target.kind
    }) {
        edits_by_document
            .entry(source.document_id().clone())
            .or_default()
            .push(TextEdit {
                range: diagnostic_range(source.text(), site.member_range),
                new_text: new_name.to_owned(),
            });
    }
}

fn schema_type_target_for_hint_at_token(
    databases: &LanguageServiceDatabases,
    text: &str,
    hint: &HirTypeHint,
    token: &RenameToken,
) -> Option<SchemaTypeRenameTarget> {
    let target = schema_type_target_for_hint(databases, hint)?;
    let range = schema_type_hint_name_range(text, hint, &target)?;
    (range.start <= token.range.start && token.range.end <= range.end).then(|| {
        let mut target = target;
        target.token = token.clone();
        target
    })
}

fn schema_type_target_for_hint(
    databases: &LanguageServiceDatabases,
    hint: &HirTypeHint,
) -> Option<SchemaTypeRenameTarget> {
    if !hint.args.is_empty() {
        return None;
    }
    let schema = databases.schema_db().facts();
    let qualified = hint.path.join("::");
    let candidates = [qualified.as_str(), hint.path.last()?.as_str()];
    for name in candidates {
        if schema.type_fact(name).is_some() {
            let target = SchemaTypeRenameTarget {
                name: name.to_owned(),
                kind: SchemaTypeRenameKind::Type,
                token: RenameToken {
                    range: TextRange::new(0, 0),
                },
            };
            return source_backed_schema_type_target(databases, target);
        }
        if schema.trait_fact(name).is_some() {
            let target = SchemaTypeRenameTarget {
                name: name.to_owned(),
                kind: SchemaTypeRenameKind::Trait,
                token: RenameToken {
                    range: TextRange::new(0, 0),
                },
            };
            return source_backed_schema_type_target(databases, target);
        }
    }
    None
}

fn schema_type_hint_name_range(
    text: &str,
    hint: &HirTypeHint,
    target: &SchemaTypeRenameTarget,
) -> Option<TextRange> {
    let name = target.name.rsplit("::").next().unwrap_or(&target.name);
    type_hint_name_range(text, hint, name)
}

fn schema_function_span(
    databases: &LanguageServiceDatabases,
    target: &SchemaFunctionRenameTarget,
) -> Option<Span> {
    databases
        .schema_db()
        .source_locations()
        .function_span(&target.name)
}

fn schema_variant_span(
    databases: &LanguageServiceDatabases,
    target: &SchemaVariantRenameTarget,
) -> Option<Span> {
    databases
        .schema_db()
        .source_locations()
        .variant_span(&target.owner, &target.variant)
}

fn source_span_contains_token(span: Span, source_id: SourceId, token: &RenameToken) -> bool {
    span.source == source_id
        && span_text_range(span)
            .is_some_and(|range| range.start <= token.range.start && token.range.end <= range.end)
}

fn schema_type_span(
    databases: &LanguageServiceDatabases,
    target: &SchemaTypeRenameTarget,
) -> Option<Span> {
    let locations = databases.schema_db().source_locations();
    match target.kind {
        SchemaTypeRenameKind::Type => locations.type_span(&target.name),
        SchemaTypeRenameKind::Trait => locations.trait_span(&target.name),
    }
}

fn schema_member_span(
    databases: &LanguageServiceDatabases,
    target: &SchemaMemberRenameTarget,
) -> Option<Span> {
    let locations = databases.schema_db().source_locations();
    match target.kind {
        SchemaMemberRenameKind::Field => locations.field_span(&target.owner, &target.member),
        SchemaMemberRenameKind::Method => locations.method_span(&target.owner, &target.member),
        SchemaMemberRenameKind::TraitMethod => {
            locations.trait_method_span(&target.owner, &target.member)
        }
    }
}

fn source_backed_schema_type_target(
    databases: &LanguageServiceDatabases,
    target: SchemaTypeRenameTarget,
) -> Option<SchemaTypeRenameTarget> {
    let span = schema_type_span(databases, &target)?;
    databases.source_record_for_rename(span.source)?;
    Some(target)
}

fn source_backed_schema_function_target(
    databases: &LanguageServiceDatabases,
    target: SchemaFunctionRenameTarget,
) -> Option<SchemaFunctionRenameTarget> {
    let span = schema_function_span(databases, &target)?;
    databases.source_record_for_rename(span.source)?;
    Some(target)
}

fn source_backed_schema_variant_target(
    databases: &LanguageServiceDatabases,
    target: SchemaVariantRenameTarget,
) -> Option<SchemaVariantRenameTarget> {
    let span = schema_variant_span(databases, &target)?;
    databases.source_record_for_rename(span.source)?;
    Some(target)
}

fn source_backed_schema_target(
    databases: &LanguageServiceDatabases,
    target: SchemaMemberRenameTarget,
) -> Option<SchemaMemberRenameTarget> {
    let span = schema_member_span(databases, &target)?;
    databases.source_record_for_rename(span.source)?;
    Some(target)
}

fn schema_type_name_conflicts(
    schema: &RegistryFacts,
    target: &SchemaTypeRenameTarget,
    new_name: &str,
) -> bool {
    if new_name == target.name {
        return false;
    }
    schema.type_fact(new_name).is_some() || schema.trait_fact(new_name).is_some()
}

fn schema_function_name_conflicts(
    schema: &RegistryFacts,
    target: &SchemaFunctionRenameTarget,
    new_name: &str,
) -> bool {
    let renamed = schema_function_renamed_name(&target.name, new_name);
    if renamed == target.name {
        return false;
    }
    schema.function_fact(&renamed).is_some()
}

fn schema_variant_name_conflicts(
    schema: &RegistryFacts,
    target: &SchemaVariantRenameTarget,
    new_name: &str,
) -> bool {
    if new_name == target.variant {
        return false;
    }
    schema.variant_fact(&target.owner, new_name).is_some()
}

fn schema_member_name_conflicts(
    schema: &RegistryFacts,
    target: &SchemaMemberRenameTarget,
    new_name: &str,
) -> bool {
    if new_name == target.member {
        return false;
    }
    match target.kind {
        SchemaMemberRenameKind::Field => schema.field_fact(&target.owner, new_name).is_some(),
        SchemaMemberRenameKind::Method => schema.method_fact(&target.owner, new_name).is_some(),
        SchemaMemberRenameKind::TraitMethod => {
            schema.trait_method_fact(&target.owner, new_name).is_some()
        }
    }
}

fn schema_method_owner(
    schema: &RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<(String, SchemaMemberRenameKind)> {
    owner_names(receiver).into_iter().find_map(|owner| {
        if schema.method_fact(&owner, method).is_some() {
            Some((owner, SchemaMemberRenameKind::Method))
        } else if schema.trait_method_fact(&owner, method).is_some() {
            Some((owner, SchemaMemberRenameKind::TraitMethod))
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

fn schema_variant_target_for_path(
    databases: &LanguageServiceDatabases,
    path: &[String],
) -> Option<SchemaVariantRenameTarget> {
    let (variant, owner_segments) = path.split_last()?;
    if owner_segments.is_empty() {
        return None;
    }
    let owner = owner_segments.join("::");
    let schema = databases.schema_db().facts();
    if schema.variant_fact(&owner, variant).is_some() {
        return Some(SchemaVariantRenameTarget {
            owner,
            variant: variant.clone(),
            token: RenameToken {
                range: TextRange::new(0, 0),
            },
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
        .then_some(SchemaVariantRenameTarget {
            owner: matched_owner,
            variant: variant.clone(),
            token: RenameToken {
                range: TextRange::new(0, 0),
            },
        })
}

fn schema_function_target_for_name(
    databases: &LanguageServiceDatabases,
    callee: &str,
) -> Option<SchemaFunctionRenameTarget> {
    let schema = databases.schema_db().facts();
    if schema.function_fact(callee).is_some() {
        return Some(SchemaFunctionRenameTarget {
            name: callee.to_owned(),
            token: RenameToken {
                range: TextRange::new(0, 0),
            },
        });
    }

    if callee.contains("::") {
        return None;
    }

    let mut matches = schema.functions().filter_map(|function| {
        (schema_function_segment(&function.name) == callee).then_some(function.name)
    });
    let name = matches.next()?;
    matches
        .next()
        .is_none()
        .then_some(SchemaFunctionRenameTarget {
            name,
            token: RenameToken {
                range: TextRange::new(0, 0),
            },
        })
}

fn function_call_name_at(text: &str, token_range: TextRange) -> Option<String> {
    if !is_call_callee(text, token_range) {
        return None;
    }
    let before_token = text.get(..token_range.start)?;
    let start = before_token
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_function_path_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    if before_token.get(..start)?.trim_end().ends_with('.') {
        return None;
    }
    text.get(start..token_range.end).map(str::to_owned)
}

fn path_ending_at(text: &str, range: TextRange) -> Option<Vec<String>> {
    let mut path = vec![token_text(text, range)?.to_owned()];
    let mut cursor = range.start;
    loop {
        let before_segment = text.get(..cursor)?.trim_end();
        let Some(before_separator) = before_segment.strip_suffix("::").map(str::trim_end) else {
            break;
        };
        let end = before_separator.len();
        let start = before_separator
            .char_indices()
            .rev()
            .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
            .unwrap_or(0);
        if start == end {
            break;
        }
        path.push(text.get(start..end)?.to_owned());
        cursor = start;
    }
    (path.len() > 1).then(|| {
        path.reverse();
        path
    })
}

fn schema_function_segment(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

fn schema_function_renamed_name(name: &str, new_segment: &str) -> String {
    if let Some((prefix, _)) = name.rsplit_once("::") {
        format!("{prefix}::{new_segment}")
    } else {
        new_segment.to_owned()
    }
}

fn is_call_callee(text: &str, range: TextRange) -> bool {
    text.get(range.end..)
        .is_some_and(|suffix| suffix.trim_start().starts_with('('))
}

fn is_function_path_continue(ch: char) -> bool {
    ch == '_' || ch == ':' || ch.is_ascii_alphanumeric()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
