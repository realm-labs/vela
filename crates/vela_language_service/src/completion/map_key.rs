use vela_common::SourceId;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{
    AstNode, SyntaxLetStmt, SyntaxMapEntry, SyntaxMapExpr, SyntaxSourceFile, SyntaxTypeHint,
};
use vela_syntax::{SyntaxNode, SyntaxToken, TextRange as SyntaxTextRange, TextSize, TokenAtOffset};

use crate::{
    LanguageServiceDatabases, QueryContext, TextRange,
    completion::{
        CompletionItem, dedupe_and_filter_service_items, display_type_detail_parts,
        label_segment_matches, module_path::enum_variant_path_completions,
    },
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct MapKeyContext {
    pub(super) key_hint: Option<HirTypeHint>,
    pub(super) used_keys: Vec<Vec<String>>,
}

pub(super) fn map_key_completion_items(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    map_key: &MapKeyContext,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    let Some(key_hint) = map_key.key_hint.as_ref() else {
        return Vec::new();
    };
    let graph = databases.hir_db().graph();
    let Some(module) = query.module_key().and_then(|key| graph.module_id(key)) else {
        return Vec::new();
    };
    // This owner comes from a type hint; value bindings cannot shadow it.
    let Some(path) = graph.expand_import_path(module, &key_hint.path) else {
        return Vec::new();
    };
    let base = path.join("::");
    let items =
        enum_variant_path_completions(graph, databases.schema_db().facts(), module, &base, prefix)
            .into_iter()
            .map(|mut item| {
                let detail = display_type_detail_parts(key_hint.display());
                item.detail = detail.render();
                item.with_detail_parts(detail)
            })
            .collect();
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
        used_keys: map_entry_path_keys(&map, &entry),
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

fn map_entry_path_keys(map: &SyntaxMapExpr, current: &SyntaxMapEntry) -> Vec<Vec<String>> {
    map.entries()
        .filter(|entry| entry.syntax() != current.syntax())
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
