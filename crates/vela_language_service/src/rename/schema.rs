use std::collections::BTreeMap;

use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::body::HirField;
use vela_hir::module_graph::ModuleGraph;

use crate::{
    DocumentId, LanguageServiceDatabases, QueryContext, SourceRecord, TextRange, query_context,
    schema_type_sites::{self, SchemaTypeKind},
};

use super::{
    RenameRisk, RenameRiskKind, RenameToken, TextEdit, WorkspaceEdit, diagnostic_range,
    span_text_range, workspace_edit_for_rename,
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
    if schema_function_name_conflicts(databases.schema_db().facts(), &target, new_name)
        || super::schema_collisions::function_name_is_captured(databases, &target.name, new_name)
        || super::schema_lookup::changes_lookup(databases, &target.name, new_name)
    {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_schema_function_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_schema_function_use_edits(databases, &target, new_name, &mut edits_by_document);

    workspace_edit_for_rename(
        databases,
        edits_by_document,
        schema_rename_risk(&target.name),
    )
}

pub(super) fn rename_schema_variant(
    databases: &LanguageServiceDatabases,
    target: SchemaVariantRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    if schema_variant_name_conflicts(databases.schema_db().facts(), &target, new_name) {
        return None;
    }
    if super::schema_variant_lookup::changes_lookup(
        databases,
        &target.owner,
        &target.variant,
        new_name,
    ) {
        return None;
    }
    if super::schema_collisions::variant_name_is_captured(
        databases,
        &target.owner,
        &target.variant,
        new_name,
    ) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_schema_variant_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_schema_variant_use_edits(databases, &target, new_name, &mut edits_by_document);

    workspace_edit_for_rename(
        databases,
        edits_by_document,
        schema_rename_risk(&format!("{}::{}", target.owner, target.variant)),
    )
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
    schema_type_span(databases, &target)?;
    push_schema_type_site_edits(databases, &target, new_name, &mut edits_by_document);

    workspace_edit_for_rename(
        databases,
        edits_by_document,
        schema_rename_risk(&target.name),
    )
}

pub(super) fn rename_schema_member(
    databases: &LanguageServiceDatabases,
    target: SchemaMemberRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    if schema_member_name_conflicts(databases.schema_db().facts(), &target, new_name)
        || schema_method_call_conflicts(databases, &target, new_name)
    {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_schema_member_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_schema_member_use_edits(databases, &target, new_name, &mut edits_by_document);
    super::schema_records::append_edits(databases, &target, new_name, &mut edits_by_document)?;

    workspace_edit_for_rename(
        databases,
        edits_by_document,
        schema_rename_risk(&format!("{}.{}", target.owner, target.member)),
    )
}

fn schema_rename_risk(symbol: &str) -> Vec<RenameRisk> {
    vec![RenameRisk {
        kind: RenameRiskKind::SchemaAbi,
        message: format!(
            "renaming source-backed schema item `{symbol}` can break host schema compatibility and editor/client integrations"
        ),
    }]
}

pub(super) fn schema_type_site_target(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
    token: &RenameToken,
) -> Option<SchemaTypeRenameTarget> {
    let site = schema_type_sites::target(databases, source, token.range)?;
    source_backed_schema_type_target(
        databases,
        SchemaTypeRenameTarget {
            name: site.identity.name,
            kind: match site.identity.kind {
                SchemaTypeKind::Type => SchemaTypeRenameKind::Type,
                SchemaTypeKind::Trait => SchemaTypeRenameKind::Trait,
            },
            token: token.clone(),
        },
    )
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

pub(super) fn schema_function_use_target(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    _text: &str,
    token: &RenameToken,
) -> Option<SchemaFunctionRenameTarget> {
    let source = query.source_record()?;
    crate::schema_function_sites::target(databases, source, token.range)
        .map(|name| SchemaFunctionRenameTarget {
            name,
            token: token.clone(),
        })
        .and_then(|target| source_backed_schema_function_target(databases, target))
}

pub(super) fn schema_variant_use_target(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    _text: &str,
    token: &RenameToken,
) -> Option<SchemaVariantRenameTarget> {
    let source = query.source_record()?;
    let (owner, variant) = crate::schema_variant_sites::target(databases, source, token.range)?;
    source_backed_schema_variant_target(
        databases,
        SchemaVariantRenameTarget {
            owner,
            variant,
            token: token.clone(),
        },
    )
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

fn push_schema_type_site_edits(
    databases: &LanguageServiceDatabases,
    target: &SchemaTypeRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let kind = match target.kind {
        SchemaTypeRenameKind::Type => SchemaTypeKind::Type,
        SchemaTypeRenameKind::Trait => SchemaTypeKind::Trait,
    };
    for source in databases.source_db().records().values() {
        for site in schema_type_sites::sites(databases, source) {
            if site.identity.name != target.name || site.identity.kind != kind {
                continue;
            }
            let Some(range) = site.edit_range else {
                continue;
            };
            edits_by_document
                .entry(source.document_id().clone())
                .or_default()
                .push(TextEdit {
                    range: diagnostic_range(source.text(), range),
                    new_text: new_name.to_owned(),
                });
        }
    }
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

fn push_schema_function_use_edits(
    databases: &LanguageServiceDatabases,
    target: &SchemaFunctionRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    for source in databases.source_db().records().values() {
        for site in crate::schema_function_sites::sites(databases, source) {
            if site.name == target.name
                && let Some(range) = site.edit_range
            {
                edits_by_document
                    .entry(source.document_id().clone())
                    .or_default()
                    .push(TextEdit {
                        range: diagnostic_range(source.text(), range),
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
        for site in crate::schema_variant_sites::sites(databases, source) {
            if site.owner != target.owner || site.variant != target.variant {
                continue;
            }
            let Some(range) = site.edit_range else {
                continue;
            };
            edits_by_document
                .entry(source.document_id().clone())
                .or_default()
                .push(TextEdit {
                    range: diagnostic_range(source.text(), range),
                    new_text: new_name.to_owned(),
                });
        }
    }
}

fn push_schema_member_use_edits(
    databases: &LanguageServiceDatabases,
    target: &SchemaMemberRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let graph = databases.hir_db().graph();
    for source in databases.source_db().records().values() {
        match target.kind {
            SchemaMemberRenameKind::Field => {
                for field in graph.fields_in_source(source.source_id()) {
                    if field.name != target.member {
                        continue;
                    }
                    let Some(site) = schema_member_site_for_field(graph, field, false) else {
                        continue;
                    };
                    push_schema_member_site_edit(
                        databases,
                        source,
                        site,
                        target,
                        new_name,
                        edits_by_document,
                    );
                }
            }
            SchemaMemberRenameKind::Method | SchemaMemberRenameKind::TraitMethod => {
                for field in graph.member_calls_in_source(source.source_id()) {
                    if field.name != target.member {
                        continue;
                    }
                    let Some(site) = schema_member_site_for_field(graph, field, true) else {
                        continue;
                    };
                    push_schema_member_site_edit(
                        databases,
                        source,
                        site,
                        target,
                        new_name,
                        edits_by_document,
                    );
                }
            }
        }
    }
}

fn schema_method_call_conflicts(
    databases: &LanguageServiceDatabases,
    target: &SchemaMemberRenameTarget,
    new_name: &str,
) -> bool {
    if target.member == new_name || target.kind == SchemaMemberRenameKind::Field {
        return false;
    }
    let graph = databases.hir_db().graph();
    databases.source_db().records().values().any(|source| {
        graph
            .member_calls_in_source(source.source_id())
            .filter(|field| field.name == new_name)
            .any(|field| {
                let Some(site) = schema_member_site_for_field(graph, field, true) else {
                    return false;
                };
                let Some(receiver) = query_context::type_fact_for_source_range(
                    databases,
                    source.source_id(),
                    site.receiver_range,
                ) else {
                    return false;
                };
                schema_member_target_for_receiver_fact(
                    databases,
                    &receiver,
                    &target.member,
                    true,
                    &target.token,
                )
                .is_some_and(|found| found.owner == target.owner && found.kind == target.kind)
            })
    })
}

fn schema_member_site_for_field(
    graph: &ModuleGraph,
    field: &HirField,
    is_call: bool,
) -> Option<SchemaMemberSite> {
    Some(SchemaMemberSite {
        member_range: span_text_range(field.member_origin.span)?,
        receiver_range: graph
            .expression_span(field.receiver)
            .and_then(span_text_range)?,
        is_call,
    })
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
    schema_type_sites::declaration_span(
        databases,
        &schema_type_sites::SchemaTypeIdentity {
            name: target.name.clone(),
            kind: match target.kind {
                SchemaTypeRenameKind::Type => SchemaTypeKind::Type,
                SchemaTypeRenameKind::Trait => SchemaTypeKind::Trait,
            },
        },
    )
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
    if new_name == target.name.rsplit("::").next().unwrap_or(&target.name) {
        return false;
    }
    let renamed = target.name.rsplit_once("::").map_or_else(
        || new_name.to_owned(),
        |(owner, _)| format!("{owner}::{new_name}"),
    );
    schema.type_fact(&renamed).is_some() || schema.trait_fact(&renamed).is_some()
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

fn schema_function_renamed_name(name: &str, new_segment: &str) -> String {
    if let Some((prefix, _)) = name.rsplit_once("::") {
        format!("{prefix}::{new_segment}")
    } else {
        new_segment.to_owned()
    }
}
