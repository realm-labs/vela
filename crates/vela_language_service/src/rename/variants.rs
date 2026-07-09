use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::body::HirPathKind;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_syntax::ast::Visibility;

use crate::{
    DocumentId, LanguageServiceDatabases, TextRange, hir_path_sites,
    query_context::binding_resolution_for_source_range,
};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, name_range_in_text, span_text_range,
    workspace_edit_for_rename,
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
    is_pattern: bool,
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

    workspace_edit_for_rename(databases, edits_by_document, Vec::new())
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
    source_id: SourceId,
    _text: &str,
    token: &RenameToken,
) -> Option<EnumVariantRenameTarget> {
    graph
        .paths_in_source(source_id)
        .filter(|path| {
            hir_path_sites::is_expression_path(path.kind) || path.kind == HirPathKind::Pattern
        })
        .find_map(|path| {
            let site = hir_path_sites::site(path)?;
            (site.segment_range == token.range).then(|| {
                enum_variant_use_target_for_path(
                    graph,
                    bindings,
                    site.path,
                    token,
                    path.kind == HirPathKind::Pattern,
                )
            })?
        })
}

fn enum_variant_use_target_for_path(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    path: &[String],
    token: &RenameToken,
    is_pattern: bool,
) -> Option<EnumVariantRenameTarget> {
    let variant = path.last()?;
    if is_pattern
        && let Some(BindingResolution::Declaration(owner)) = bindings.pattern_resolution(path)
        && can_rename_enum_variant(graph, *owner, variant)
    {
        return Some(EnumVariantRenameTarget {
            owner: *owner,
            variant: variant.clone(),
            token: token.clone(),
        });
    }
    match binding_resolution_for_source_range(graph, bindings, token.range)? {
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
        for path in graph.paths_in_source(source.source_id()) {
            if !(hir_path_sites::is_expression_path(path.kind) || path.kind == HirPathKind::Pattern)
            {
                continue;
            }
            let Some(site) = hir_path_sites::site(path) else {
                continue;
            };
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
                    path: site.path,
                    range: site.segment_range,
                    is_pattern: path.kind == HirPathKind::Pattern,
                },
                target,
                new_name,
                edits_by_document,
            );
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
            site.is_pattern,
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
