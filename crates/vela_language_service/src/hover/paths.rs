use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxPathExpr, SyntaxPattern},
};

use super::Hover;
use crate::{
    DiagnosticRange, LanguageServiceDatabases, QueryContext, SymbolRef, symbol_target::SymbolTarget,
};

// A qualified expression or pattern owns its result. Resolve the selected segment's
// canonical identity rather than the whole expression's lexical binding.
pub(super) fn hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Option<Hover>> {
    let path = query
        .syntax_parse()?
        .tree()
        .syntax()
        .descendants()
        .filter_map(|node| {
            SyntaxPathExpr::cast(node.clone())
                .map(|path| path.path_tokens())
                .or_else(|| SyntaxPattern::cast(node).map(|pattern| pattern.path_tokens()))
        })
        .find_map(|tokens| {
            let tokens = tokens
                .into_iter()
                .filter(|token| matches!(token.kind(), SyntaxKind::Ident | SyntaxKind::SelfKw))
                .collect::<Vec<_>>();
            if tokens.len() < 2 {
                return None;
            }
            let index = tokens.iter().position(|token| {
                usize::from(token.text_range().start()) == target.range().start
                    && usize::from(token.text_range().end()) == target.range().end
            })?;
            Some(
                tokens[..=index]
                    .iter()
                    .map(|token| token.text().to_owned())
                    .collect::<Vec<_>>(),
            )
        })?;
    let Some(symbol) = target.symbol() else {
        return Some(None);
    };
    Some(match symbol {
        SymbolRef::Source(_) => {
            // Declaration labels are scoped module paths; separate packages can
            // have the same label. The caller's package and selected path own it.
            query
                .expand_import_path(&path)
                .and_then(|path| super::imports::path_hover(db, query, &path, range))
        }
        SymbolRef::Schema(name) => super::schema::symbol_hover(db.schema_db().facts(), name, range),
        SymbolRef::Builtin(name) => super::stdlib_symbol_hover(name, range),
        SymbolRef::Local(_) => None,
    })
}
