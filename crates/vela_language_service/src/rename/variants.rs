use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, Visibility};

use crate::{DocumentId, LanguageServiceDatabases};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, span_text_range,
    workspace_edit_for_rename,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct EnumVariantRenameTarget {
    pub(super) owner: HirDeclId,
    pub(super) variant: String,
    pub(super) token: RenameToken,
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
    if super::source_variant_lookup::changes_lookup(
        databases,
        target.owner,
        &target.variant,
        new_name,
    ) {
        return None;
    }
    if super::schema_collisions::source_variant_name_is_captured(
        databases,
        target.owner,
        &target.variant,
        new_name,
    ) {
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
            let variant_range = span_text_range(variant.span)?;
            if variant_range.start <= token.range.start && token.range.end <= variant_range.end {
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
    let range = span_text_range(variant.span)?;
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
    for source in databases.source_db().records().values() {
        for site in crate::source_variant_sites::sites(databases, source) {
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

pub(super) fn can_rename_enum_variant(
    graph: &ModuleGraph,
    owner: HirDeclId,
    variant: &str,
) -> bool {
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
