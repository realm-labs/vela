//! Keep diagnosed literal envelopes opaque without changing lexer semantics.
use std::collections::BTreeMap;

use vela_syntax::{
    SyntaxKind,
    lexer::Lexed,
    token::{Token, TokenKind},
};

pub(super) fn tokens(mut lexed: Lexed) -> Vec<Token> {
    let original_len = lexed.tokens.len();
    let envelopes: BTreeMap<_, _> = lexed
        .diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let bytes = match diagnostic.code.as_deref()? {
                "E_LEX_BYTE_STRING" => true,
                "E_LEX_STRING" | "E_LEX_MULTILINE_STRING" | "E_LEX_CHAR_LITERAL" => false,
                _ => return None,
            };
            let span = diagnostic.span?;
            Some(((span.source, span.start, span.end), bytes))
        })
        .collect();
    for literal in &lexed.lossless_tokens {
        if literal.kind != SyntaxKind::Unknown {
            continue;
        }
        let Some(bytes) =
            envelopes.get(&(literal.span.source, literal.span.start, literal.span.end))
        else {
            continue;
        };
        let kind = match bytes {
            true if literal.text.starts_with("b\"") => TokenKind::Bytes(Vec::new()),
            _ if literal.text.starts_with(['\"', '\'']) || literal.text.starts_with("f\"") => {
                TokenKind::String(String::new())
            }
            _ => continue,
        };
        lexed.tokens.push(Token {
            kind,
            span: literal.span,
        });
    }
    if lexed.tokens.len() != original_len {
        lexed
            .tokens
            .sort_by_key(|token| (token.span.start, token.span.end));
    }
    lexed.tokens
}
