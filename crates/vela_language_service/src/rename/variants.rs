use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_syntax::ast::Visibility;
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{DocumentId, LanguageServiceDatabases, TextRange};

use super::{
    DocumentTextEdit, RenameToken, TextEdit, WorkspaceEdit, diagnostic_range,
    is_identifier_continue, name_range_in_text, span_text_range, token_text,
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
            DocumentTextEdit { document_id, edits }
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
    text: &str,
    token: &RenameToken,
) -> Option<EnumVariantRenameTarget> {
    let path = path_ending_at(text, token.range)?;
    let variant = path.last()?;
    if let Some(BindingResolution::Declaration(owner)) = bindings.pattern_resolution(&path)
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
        let source_id = source.source_id();
        let text = source.text();
        for range in path_segment_ranges(source_id, text, &target.variant) {
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
                if enum_variant_use_target(graph, bindings, text, &RenameToken { range })
                    .is_some_and(|found| {
                        found.owner == target.owner && found.variant == target.variant
                    })
                {
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

fn path_segment_ranges(source_id: SourceId, text: &str, name: &str) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(token_name) if token_name == name => {
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
