use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_syntax::ast::{SourceFile, Visibility};

use crate::{DocumentId, LanguageServiceDatabases, TextRange, path_calls};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, document_text_edit_for_rename,
    name_range_in_text, span_text_range,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct EnumVariantRenameTarget {
    pub(super) owner: HirDeclId,
    pub(super) variant: String,
    pub(super) token: RenameToken,
}

struct EnumVariantUseEditSite<'a> {
    source: &'a crate::SourceRecord,
    text: &'a str,
    path: &'a [String],
    range: TextRange,
}

pub(super) fn rename_enum_variant(
    databases: &LanguageServiceDatabases,
    target: EnumVariantRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let graph = databases.hir_db().graph();
    if enum_variant_name_conflicts(graph, &target, new_name) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_enum_variant_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_enum_variant_use_edits(databases, &target, new_name, &mut edits_by_document);

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

pub(super) fn enum_variant_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &RenameToken,
) -> Option<EnumVariantRenameTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Enum
            || declaration.visibility == Visibility::Public
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let shape = graph.enum_shape(declaration.id)?;
        for variant in &shape.variants {
            let span_range = span_text_range(variant.span)?;
            let name_range = name_range_in_text(text, span_range, &variant.name)?;
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
                return Some(EnumVariantRenameTarget {
                    owner: declaration.id,
                    variant: variant.name.clone(),
                    token: token.clone(),
                });
            }
        }
    }
    None
}

pub(super) fn enum_variant_use_target(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    parsed: Option<&SourceFile>,
    text: &str,
    token: &RenameToken,
) -> Option<EnumVariantRenameTarget> {
    if let Some(parsed) = parsed {
        for site in path_calls::path_expression_sites(parsed, text) {
            if site.segment_range == token.range {
                return enum_variant_use_target_for_path(graph, bindings, &site.path, token);
            }
        }
        for site in path_calls::pattern_path_sites(parsed, text) {
            if site.segment_range == token.range {
                return enum_variant_use_target_for_path(graph, bindings, &site.path, token);
            }
        }
    }
    None
}

fn enum_variant_use_target_for_path(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    path: &[String],
    token: &RenameToken,
) -> Option<EnumVariantRenameTarget> {
    let variant = path.last()?;
    if let Some(BindingResolution::Declaration(owner)) = bindings.pattern_resolution(path)
        && can_rename_enum_variant(graph, *owner, variant)
    {
        return Some(EnumVariantRenameTarget {
            owner: *owner,
            variant: variant.clone(),
            token: token.clone(),
        });
    }
    match narrowest_resolution_at_token(bindings, token)? {
        BindingResolution::Declaration(owner)
            if can_rename_enum_variant(graph, *owner, variant) =>
        {
            Some(EnumVariantRenameTarget {
                owner: *owner,
                variant: variant.clone(),
                token: token.clone(),
            })
        }
        BindingResolution::Declaration(_)
        | BindingResolution::Local(_)
        | BindingResolution::Import(_)
        | BindingResolution::QualifiedPath(_) => None,
    }
}

fn push_enum_variant_declaration_edit(
    databases: &LanguageServiceDatabases,
    target: &EnumVariantRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    let graph = databases.hir_db().graph();
    let variant = graph
        .enum_shape(target.owner)?
        .variants
        .iter()
        .find(|variant| variant.name == target.variant)?;
    let source = databases.source_record_for_rename(variant.span.source)?;
    let span_range = span_text_range(variant.span)?;
    let range = name_range_in_text(source.text(), span_range, &variant.name)?;
    edits_by_document
        .entry(source.document_id().clone())
        .or_default()
        .push(TextEdit {
            range: diagnostic_range(source.text(), range),
            new_text: new_name.to_owned(),
        });
    Some(())
}

fn push_enum_variant_use_edits(
    databases: &LanguageServiceDatabases,
    target: &EnumVariantRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let graph = databases.hir_db().graph();
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
                push_enum_variant_use_edit_for_path(
                    graph,
                    EnumVariantUseEditSite {
                        source,
                        text,
                        path: &site.path,
                        range: site.segment_range,
                    },
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
                push_enum_variant_use_edit_for_path(
                    graph,
                    EnumVariantUseEditSite {
                        source,
                        text,
                        path: &site.path,
                        range: site.segment_range,
                    },
                    target,
                    new_name,
                    edits_by_document,
                );
            }
        }
    }
}

fn push_enum_variant_use_edit_for_path(
    graph: &ModuleGraph,
    site: EnumVariantUseEditSite<'_>,
    target: &EnumVariantRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let Some(start) = u32::try_from(site.range.start).ok() else {
        return;
    };
    for declaration in graph.declarations() {
        if declaration.span.source != site.source.source_id() || !declaration.span.contains(start) {
            continue;
        }
        let Some(bindings) = graph.bindings(declaration.id) else {
            continue;
        };
        if enum_variant_use_target_for_path(
            graph,
            bindings,
            site.path,
            &RenameToken { range: site.range },
        )
        .is_some_and(|found| found.owner == target.owner && found.variant == target.variant)
        {
            edits_by_document
                .entry(site.source.document_id().clone())
                .or_default()
                .push(TextEdit {
                    range: diagnostic_range(site.text, site.range),
                    new_text: new_name.to_owned(),
                });
            break;
        }
    }
}

fn can_rename_enum_variant(graph: &ModuleGraph, owner: HirDeclId, variant: &str) -> bool {
    graph.declaration(owner).is_some_and(|declaration| {
        declaration.kind == DeclarationKind::Enum && declaration.visibility != Visibility::Public
    }) && enum_variant_exists(graph, owner, variant)
}

fn enum_variant_exists(graph: &ModuleGraph, owner: HirDeclId, variant: &str) -> bool {
    graph
        .enum_shape(owner)
        .is_some_and(|shape| shape.variants.iter().any(|entry| entry.name == variant))
}

fn enum_variant_name_conflicts(
    graph: &ModuleGraph,
    target: &EnumVariantRenameTarget,
    new_name: &str,
) -> bool {
    graph.enum_shape(target.owner).is_some_and(|shape| {
        shape
            .variants
            .iter()
            .any(|variant| variant.name == new_name && variant.name != target.variant)
    })
}

fn narrowest_resolution_at_token<'a>(
    bindings: &'a BindingMap,
    token: &RenameToken,
) -> Option<&'a BindingResolution> {
    bindings
        .resolutions()
        .filter_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (start <= token.range.start && token.range.end <= end)
                .then_some((end.saturating_sub(start), resolution))
        })
        .min_by_key(|(len, _)| *len)
        .map(|(_, resolution)| resolution)
}
