use vela_syntax::ast::{AstNode, SyntaxFieldExpr, SyntaxSourceFile};

use crate::TextRange;

pub(super) fn syntax_member_receiver(tree: &SyntaxSourceFile, offset: usize) -> Option<TextRange> {
    tree.syntax()
        .descendants()
        .filter_map(SyntaxFieldExpr::cast)
        .filter(|field| {
            field.name_token().map_or_else(
                || {
                    field
                        .dot_token()
                        .is_some_and(|dot| usize::from(dot.text_range().end()) == offset)
                },
                |name| usize::from(name.text_range().start()) == offset,
            )
        })
        .filter_map(|field| {
            let range = field.receiver()?.syntax().text_range();
            Some(TextRange::new(range.start().into(), range.end().into()))
        })
        .min_by_key(|range| range.len())
}
