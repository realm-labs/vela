use vela_common::Span;
use vela_hir::{
    module_graph::{Declaration, DeclarationKind, ModuleGraph},
    type_hint::{HirTypeHint, ImplMetadataKind},
};

use crate::{
    LanguageServiceDatabases, SourceRecord, TextRange,
    references::{ReferenceKind, type_hints::for_each_type_hint_in_declaration},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SchemaTypeKind {
    Type,
    Trait,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SchemaTypeIdentity {
    pub(crate) name: String,
    pub(crate) kind: SchemaTypeKind,
}

#[derive(Debug, Clone)]
pub(crate) struct SchemaTypeSite {
    pub(crate) identity: SchemaTypeIdentity,
    pub(crate) range: TextRange,
    pub(crate) edit_range: Option<TextRange>,
    pub(crate) kind: ReferenceKind,
}

pub(crate) fn declaration_span(
    databases: &LanguageServiceDatabases,
    identity: &SchemaTypeIdentity,
) -> Option<Span> {
    let locations = databases.schema_db().source_locations();
    let span = match identity.kind {
        SchemaTypeKind::Type => locations.type_span(&identity.name),
        SchemaTypeKind::Trait => locations.trait_span(&identity.name),
    }?;
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|source| source.source_id() == span.source)?;
    let range = span_range(span.start, span.end)?;
    source.text().get(range.start..range.end)?;
    Some(span)
}

pub(crate) fn target(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
    token: TextRange,
) -> Option<SchemaTypeSite> {
    sites(databases, source).into_iter().find(|site| {
        (site.range.start <= token.start && token.end <= site.range.end)
            || (site.kind == ReferenceKind::Import
                && site
                    .edit_range
                    .is_some_and(|range| range.start <= token.start && token.end <= range.end))
    })
}

pub(crate) fn sites(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
) -> Vec<SchemaTypeSite> {
    let graph = databases.hir_db().graph();
    let mut sites = Vec::new();
    let source_id = source.source_id();
    let facts = databases.schema_db().facts();

    for (name, _) in facts.types() {
        let identity = SchemaTypeIdentity {
            name: name.to_owned(),
            kind: SchemaTypeKind::Type,
        };
        if let Some(span) =
            declaration_span(databases, &identity).filter(|span| span.source == source_id)
            && let Some(range) = span_range(span.start, span.end)
        {
            sites.push(SchemaTypeSite {
                identity,
                range,
                edit_range: Some(range),
                kind: ReferenceKind::Declaration,
            });
        }
    }
    for (name, _) in facts.traits() {
        let identity = SchemaTypeIdentity {
            name: name.to_owned(),
            kind: SchemaTypeKind::Trait,
        };
        if let Some(span) =
            declaration_span(databases, &identity).filter(|span| span.source == source_id)
            && let Some(range) = span_range(span.start, span.end)
        {
            sites.push(SchemaTypeSite {
                identity,
                range,
                edit_range: Some(range),
                kind: ReferenceKind::Declaration,
            });
        }
    }

    for module in graph.module_ids() {
        let Some(imports) = graph.imports(module) else {
            continue;
        };
        for import in imports {
            if import.resolution.is_some() {
                continue;
            }
            let Some(terminal) = import.path_spans.last().copied() else {
                continue;
            };
            if terminal.source != source_id {
                continue;
            }
            let Some(identity) = identity_for_path(databases, graph, module, &import.path) else {
                continue;
            };
            let Some(edit_range) = span_range(terminal.start, terminal.end) else {
                continue;
            };
            let range = import
                .alias_span
                .and_then(|span| span_range(span.start, span.end))
                .unwrap_or(edit_range);
            sites.push(SchemaTypeSite {
                identity,
                range,
                edit_range: Some(edit_range),
                kind: ReferenceKind::Import,
            });
        }
    }

    for owner in graph.declarations() {
        if let Some(metadata) = graph.impl_metadata(owner.id)
            && let ImplMetadataKind::Trait { trait_path } = &metadata.kind
            && owner.span.source == source_id
            && let Some(identity) = identity_for_path(databases, graph, owner.module, trait_path)
            && identity.kind == SchemaTypeKind::Trait
            && let Some(span) = span_range(owner.span.start, owner.span.end)
            && let Some(range) =
                crate::references::trait_path_name_range_in_text(source.text(), span, trait_path)
        {
            let edit_range = (identity.name.rsplit("::").next()
                == trait_path.last().map(String::as_str))
            .then_some(range);
            sites.push(SchemaTypeSite {
                identity,
                range,
                edit_range,
                kind: ReferenceKind::Read,
            });
        }
        for_each_type_hint_in_declaration(graph, owner, |hint| {
            if hint.span.source != source_id {
                return;
            }
            let Some(identity) = identity_for_hint(databases, graph, owner, hint) else {
                return;
            };
            let Some(terminal) = hint.path.last() else {
                return;
            };
            let Some(span) = span_range(hint.span.start, hint.span.end) else {
                return;
            };
            let Some(range) =
                crate::references::support::last_name_range_in_text(source.text(), span, terminal)
            else {
                return;
            };
            let edit_range =
                (identity.name.rsplit("::").next() == Some(terminal.as_str())).then_some(range);
            sites.push(SchemaTypeSite {
                identity,
                range,
                edit_range,
                kind: ReferenceKind::Read,
            });
        });
    }
    sites
}

pub(crate) fn identity_for_hint(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    owner: &Declaration,
    hint: &HirTypeHint,
) -> Option<SchemaTypeIdentity> {
    if !hint.args.is_empty() {
        return None;
    }
    identity_for_path(databases, graph, owner.module, &hint.path)
}

fn identity_for_path(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    module: vela_hir::ids::ModuleId,
    path: &[String],
) -> Option<SchemaTypeIdentity> {
    let expanded = graph.expand_import_path(module, path)?;
    if [
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
    ]
    .into_iter()
    .any(|kind| {
        graph
            .resolve_visible_declaration_path(module, &expanded, kind)
            .is_some()
    }) {
        return None;
    }
    let name = expanded.join("::");
    let facts = databases.schema_db().facts();
    let kind = if facts.type_fact(&name).is_some() {
        SchemaTypeKind::Type
    } else if facts.trait_fact(&name).is_some() {
        SchemaTypeKind::Trait
    } else {
        return None;
    };
    Some(SchemaTypeIdentity { name, kind })
}

fn span_range(start: u32, end: u32) -> Option<TextRange> {
    Some(TextRange::new(
        usize::try_from(start).ok()?,
        usize::try_from(end).ok()?,
    ))
}
