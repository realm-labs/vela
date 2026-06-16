use std::collections::BTreeMap;

use vela_analysis::{facts::AnalysisFacts, registry::RegistryFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::module_graph::Declaration;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{DocumentId, LanguageServiceDatabases, TextRange};

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
    text: &str,
    token: &RenameToken,
) -> Option<SchemaFunctionRenameTarget> {
    let callee = function_call_name_at(text, token.range)?;
    let target = schema_function_target_for_name(databases, &callee)?;
    source_backed_schema_function_target(databases, target).map(|mut target| {
        target.token = token.clone();
        target
    })
}

pub(super) fn schema_variant_use_target(
    databases: &LanguageServiceDatabases,
    text: &str,
    token: &RenameToken,
) -> Option<SchemaVariantRenameTarget> {
    let path = path_ending_at(text, token.range)?;
    let target = schema_variant_target_for_path(databases, &path)?;
    source_backed_schema_variant_target(databases, target).map(|mut target| {
        target.token = token.clone();
        target
    })
}

pub(super) fn schema_member_use_target(
    databases: &LanguageServiceDatabases,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    token: &RenameToken,
) -> Option<SchemaMemberRenameTarget> {
    let member = token_text(text, token.range)?;
    let schema = databases.schema_db().facts();
    let target = if is_call_callee(text, token.range) {
        schema_method_target_for_member(
            schema,
            facts,
            text,
            source_id,
            bindings,
            member,
            token.range,
        )
    } else {
        schema_field_target_for_member(
            schema,
            facts,
            text,
            source_id,
            bindings,
            member,
            token.range,
        )
    }?;
    source_backed_schema_target(databases, target).map(|mut target| {
        target.token = token.clone();
        target
    })
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
        for range in schema_function_use_ranges(source.source_id(), text, target_segment) {
            if schema_function_use_target(databases, text, &RenameToken { range })
                .is_some_and(|found| found.name == target.name)
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
    }
}

fn push_schema_variant_use_edits(
    databases: &LanguageServiceDatabases,
    target: &SchemaVariantRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    for source in databases.source_db().records().values() {
        let source_id = source.source_id();
        let text = source.text();
        for range in schema_variant_use_ranges(source_id, text, target) {
            if schema_variant_use_target(databases, text, &RenameToken { range })
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
    }
}

fn push_schema_member_use_edits(
    databases: &LanguageServiceDatabases,
    target: &SchemaMemberRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);
    for source in databases.source_db().records().values() {
        let source_id = source.source_id();
        let text = source.text();
        for range in schema_member_use_ranges(source_id, text, target) {
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
                if schema_member_use_target(
                    databases,
                    &facts,
                    text,
                    source_id,
                    bindings,
                    &RenameToken { range },
                )
                .is_some_and(|found| {
                    found.owner == target.owner
                        && found.member == target.member
                        && found.kind == target.kind
                }) {
                    edits_by_document
                        .entry(source.document_id().clone())
                        .or_default()
                        .push(TextEdit {
                            range: diagnostic_range(text, range),
                            new_text: new_name.to_owned(),
                        });
                    break;
                }
            }
        }
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

fn schema_method_target_for_member(
    schema: &RegistryFacts,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    method: &str,
    member_range: TextRange,
) -> Option<SchemaMemberRenameTarget> {
    let receiver = member_receiver_range(text, member_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = schema_type_fact_for_resolution(resolution, bindings, facts, schema)?;
    let (owner, kind) = schema_method_owner(schema, &receiver, method)?;
    Some(SchemaMemberRenameTarget {
        owner,
        member: method.to_owned(),
        kind,
        token: RenameToken {
            range: member_range,
        },
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
) -> Option<SchemaMemberRenameTarget> {
    let receiver = member_receiver_range(text, member_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = schema_type_fact_for_resolution(resolution, bindings, facts, schema)?;
    let owner = schema_field_owner(schema, &receiver, field)?;
    Some(SchemaMemberRenameTarget {
        owner,
        member: field.to_owned(),
        kind: SchemaMemberRenameKind::Field,
        token: RenameToken {
            range: member_range,
        },
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

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    facts: &AnalysisFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => facts
            .local(*local)
            .cloned()
            .filter(|fact| !matches!(fact, TypeFact::Unknown)),
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
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

fn schema_function_use_ranges(
    source_id: SourceId,
    text: &str,
    target_segment: &str,
) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) if name == target_segment => {
                let range = span_text_range(token.span)?;
                function_call_name_at(text, range).map(|_| range)
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

fn schema_variant_use_ranges(
    source_id: SourceId,
    text: &str,
    target: &SchemaVariantRenameTarget,
) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) if name == target.variant => {
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

fn schema_member_use_ranges(
    source_id: SourceId,
    text: &str,
    target: &SchemaMemberRenameTarget,
) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) if name == target.member => {
                let range = span_text_range(token.span)?;
                match target.kind {
                    SchemaMemberRenameKind::Field => {
                        member_receiver_range(text, range.start).map(|_| range)
                    }
                    SchemaMemberRenameKind::Method | SchemaMemberRenameKind::TraitMethod => {
                        (is_call_callee(text, range)
                            && member_receiver_range(text, range.start).is_some())
                        .then_some(range)
                    }
                }
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

fn member_receiver_range(text: &str, member_start: usize) -> Option<TextRange> {
    let before_member = text.get(..member_start)?.trim_end();
    let before_dot = before_member.strip_suffix('.')?.trim_end();
    let end = before_dot.len();
    let start = before_dot
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| TextRange::new(start, end))
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
