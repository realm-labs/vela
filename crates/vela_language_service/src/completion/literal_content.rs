use vela_syntax::ast::AstNode;
use vela_syntax::{SyntaxKind, TextSize};

use crate::QueryContext;

pub(super) fn is_literal_content(query: &QueryContext<'_>) -> bool {
    let Some(parsed) = query.syntax_parse() else {
        return false;
    };
    let Ok(offset) = u32::try_from(query.cursor().replace_range().start) else {
        return false;
    };
    let offset = TextSize::from(offset);
    parsed
        .tree()
        .syntax()
        .token_at_offset(offset)
        .into_iter()
        .any(|token| {
            // Interpolation expressions have their own CST tokens. Only the raw
            // string chunks are suppressed; code inside braces remains eligible.
            let malformed_literal = token.kind() == SyntaxKind::Unknown
                && ["\"", "'", "b\"", "f\""]
                    .iter()
                    .any(|prefix| token.text().starts_with(prefix));
            if malformed_literal
                && token.text_range().end() == offset
                && !token.text().ends_with(['\r', '\n'])
            {
                return true;
            }
            token.text_range().contains(offset)
                && (malformed_literal
                    || matches!(
                        token.kind(),
                        SyntaxKind::String
                            | SyntaxKind::Bytes
                            | SyntaxKind::Char
                            | SyntaxKind::InterpolatedString
                    ))
        })
}
