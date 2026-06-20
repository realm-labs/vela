use vela_common::SourceId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{
    AstNode, SyntaxLetStmt, SyntaxMapEntry, SyntaxMapExpr, SyntaxSourceFile, SyntaxTypeHint,
};
use vela_syntax::{SyntaxNode, SyntaxToken, TextRange as SyntaxTextRange, TextSize, TokenAtOffset};

use crate::{
    TextRange,
    completion::{
        CompletionInsertFormat, CompletionItem, CompletionKind, dedupe_and_filter_service_items,
        display_type_detail_parts, label_segment_matches,
    },
    symbol_ref::{schema_variant_symbol, source_enum_variant_symbol},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct MapKeyContext {
    pub(super) key_hint: Option<HirTypeHint>,
    pub(super) used_keys: Vec<Vec<String>>,
    pub(super) current_module: Vec<String>,
}

pub(super) fn map_key_completion_items(
    graph: &ModuleGraph,
    schema: &vela_analysis::registry::RegistryFacts,
    map_key: &MapKeyContext,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    let Some(key_hint) = map_key.key_hint.as_ref() else {
        return Vec::new();
    };
    let mut items = script_enum_variant_key_completions(graph, map_key, key_hint);
    items.extend(schema_enum_variant_key_completions(schema, key_hint));
    let used_keys = map_key
        .used_keys
        .iter()
        .filter_map(|key| key.last().map(String::as_str))
        .collect::<Vec<_>>();
    dedupe_and_filter_service_items(items, replace_range, prefix, |item| {
        !used_keys.contains(&item.label()) && label_segment_matches(item.label(), prefix)
    })
}

pub(super) fn map_key_at(
    source: &SyntaxSourceFile,
    source_id: Option<SourceId>,
    offset: usize,
) -> Option<MapKeyContext> {
    let offset_size = syntax_offset(offset)?;
    let token = significant_token_at(source.syntax(), offset_size)?;
    let entry = token.parent_ancestors().find_map(SyntaxMapEntry::cast)?;
    let key = entry.key()?;
    range_contains_offset(key.syntax().text_range(), offset_size)?;
    let map = entry.syntax().parent().and_then(SyntaxMapExpr::cast)?;
    Some(MapKeyContext {
        key_hint: enclosing_let_map_key_hint(source_id, &map),
        used_keys: map_entry_path_keys(&map),
        current_module: Vec::new(),
    })
}

fn significant_token_at(root: &SyntaxNode, offset: TextSize) -> Option<SyntaxToken> {
    match root.token_at_offset(offset) {
        TokenAtOffset::None => None,
        TokenAtOffset::Single(token) => non_trivia_token(token),
        TokenAtOffset::Between(left, right) => {
            non_trivia_token(left).or_else(|| non_trivia_token(right))
        }
    }
}

fn non_trivia_token(token: SyntaxToken) -> Option<SyntaxToken> {
    (!token.kind().is_trivia()).then_some(token)
}

fn enclosing_let_map_key_hint(
    source_id: Option<SourceId>,
    map: &SyntaxMapExpr,
) -> Option<HirTypeHint> {
    let let_stmt = map
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(SyntaxLetStmt::cast)?;
    let initializer = let_stmt.initializer()?;
    if initializer.syntax() != map.syntax() {
        return None;
    }
    map_key_hint(source_id, &let_stmt.type_hint()?)
}

fn map_key_hint(source_id: Option<SourceId>, hint: &SyntaxTypeHint) -> Option<HirTypeHint> {
    let args = hint.type_arg_list()?;
    let mut arg_hints = args.type_hints();
    let key = arg_hints.next()?;
    let value = arg_hints.next();
    (hint.path_segments().as_slice() == ["Map"] && value.is_some() && arg_hints.next().is_none())
        .then(|| hir_type_hint_from_cst(source_id, &key))
}

fn hir_type_hint_from_cst(source_id: Option<SourceId>, hint: &SyntaxTypeHint) -> HirTypeHint {
    HirTypeHint {
        path: hint.path_segments(),
        args: hint
            .type_arg_list()
            .into_iter()
            .flat_map(|args| args.type_hints())
            .map(|arg| hir_type_hint_from_cst(source_id, &arg))
            .collect(),
        span: span_for(source_id, hint.syntax().text_range()),
    }
}

fn span_for(source_id: Option<SourceId>, range: SyntaxTextRange) -> vela_common::Span {
    vela_common::Span::new(
        source_id.unwrap_or_else(|| SourceId::new(0)),
        range.start().into(),
        range.end().into(),
    )
}

fn map_entry_path_keys(map: &SyntaxMapExpr) -> Vec<Vec<String>> {
    map.entries()
        .filter_map(|entry| {
            entry
                .key()
                .and_then(|key| key.as_path())
                .map(|path| path.path_segments())
        })
        .collect()
}

fn range_contains_offset(range: SyntaxTextRange, offset: TextSize) -> Option<()> {
    (range.start() <= offset && offset <= range.end()).then_some(())
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}

fn script_enum_variant_key_completions(
    graph: &ModuleGraph,
    map_key: &MapKeyContext,
    key_hint: &HirTypeHint,
) -> Vec<CompletionItem> {
    let Some(declaration) = script_enum_key_declaration(graph, map_key, key_hint) else {
        return Vec::new();
    };
    let Some(shape) = graph.enum_shape(declaration.id) else {
        return Vec::new();
    };
    shape
        .variants
        .iter()
        .filter_map(|variant| {
            let symbol = source_enum_variant_symbol(graph, declaration.id, &variant.name)?;
            let detail_parts = display_type_detail_parts(key_hint.display());
            Some(
                CompletionItem {
                    label: variant.name.clone(),
                    kind: CompletionKind::Variant,
                    detail: detail_parts.render(),
                    insert_text: None,
                    insert_format: CompletionInsertFormat::PlainText,
                    sort_text: None,
                    metadata: Default::default(),
                }
                .with_detail_parts(detail_parts)
                .with_symbol(symbol),
            )
        })
        .collect()
}

fn script_enum_key_declaration<'a>(
    graph: &'a ModuleGraph,
    map_key: &MapKeyContext,
    key_hint: &HirTypeHint,
) -> Option<&'a vela_hir::module_graph::Declaration> {
    graph.declaration_by_type_path(
        &key_hint.path,
        &map_key.current_module,
        DeclarationKind::Enum,
    )
}

fn schema_enum_variant_key_completions(
    schema: &vela_analysis::registry::RegistryFacts,
    key_hint: &HirTypeHint,
) -> Vec<CompletionItem> {
    let owner = key_hint.path.join("::");
    schema
        .variants_for_owner_or_short_name(&owner)
        .into_iter()
        .map(|variant| {
            let owner = variant.owner;
            let name = variant.name;
            let detail_parts = display_type_detail_parts(key_hint.display());
            CompletionItem {
                label: name.clone(),
                kind: CompletionKind::Variant,
                detail: detail_parts.render(),
                insert_text: None,
                insert_format: CompletionInsertFormat::PlainText,
                sort_text: None,
                metadata: Default::default(),
            }
            .with_detail_parts(detail_parts)
            .with_symbol(schema_variant_symbol(&owner, &name))
        })
        .collect()
}
