use vela_syntax::{
    SyntaxKind, TextSize,
    ast::{AstNode, SyntaxMapEntry},
};

use crate::QueryContext;

pub(super) fn is_symbol_position(query: &QueryContext<'_>) -> bool {
    let Some(parse) = query.syntax_parse() else {
        return false;
    };
    let Some(range) = query.identifier_range() else {
        return false;
    };
    let Ok(offset) = u32::try_from(query.cursor().replace_range().end) else {
        return false;
    };
    let offset = TextSize::from(offset);
    // Completion's text prefix can include a literal word or the identifier to
    // the left of punctuation. Hover owns the actual token under the cursor.
    let Some(token) = parse
        .tree()
        .syntax()
        .token_at_offset(offset)
        .into_iter()
        .find(|token| {
            matches!(token.kind(), SyntaxKind::Ident | SyntaxKind::SelfKw)
                && token.text_range().contains(offset)
                && usize::from(token.text_range().start()) == range.start
                && usize::from(token.text_range().end()) == range.end
        })
    else {
        return false;
    };
    // HIR treats path-shaped map keys as static key spellings, including
    // qualified paths. Values and interpolation expressions remain code.
    !token
        .parent_ancestors()
        .filter_map(SyntaxMapEntry::cast)
        .any(|entry| {
            entry.key().is_some_and(|key| {
                key.as_path().is_some() && key.syntax().text_range().contains(offset)
            })
        })
}
