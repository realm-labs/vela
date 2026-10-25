use vela_syntax::ast::AstNode;
use vela_syntax::{SyntaxKind, TextSize};

use crate::{CursorContextKind, ModulePathRole, QueryContext};

use super::{CompletionItem, CompletionKind};

pub(super) fn adjust_call_insertion(query: &QueryContext<'_>, items: &mut [CompletionItem]) {
    let range = query
        .identifier_range()
        .unwrap_or(query.cursor().replace_range());
    if query.cursor().kind() != CursorContextKind::UseImport
        && query.cursor().module_path_role() != ModulePathRole::Import
        && !has_argument_list(query, range.end)
    {
        return;
    }
    for item in items {
        if !matches!(
            item.kind(),
            CompletionKind::Function | CompletionKind::Method
        ) {
            continue;
        }
        let Some(path) = item
            .insert_text()
            .and_then(|text| text.strip_suffix("($0)"))
        else {
            continue;
        };
        super::call_operand::set_path_insertion(item, query, path.to_owned());
    }
}

pub(super) fn has_argument_list(query: &QueryContext<'_>, end: usize) -> bool {
    let Some(parse) = query.syntax_parse() else {
        return false;
    };
    let Ok(end) = u32::try_from(end) else {
        return false;
    };
    let end = TextSize::from(end);
    let first = parse.tree().syntax().token_at_offset(end).right_biased();
    std::iter::successors(first, |token| token.next_token())
        .find(|token| token.text_range().start() >= end && !token.kind().is_trivia())
        .is_some_and(|token| token.kind() == SyntaxKind::LParen)
}
