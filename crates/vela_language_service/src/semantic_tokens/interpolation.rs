//! Expand interpolation expressions without classifying literal text as code.
use vela_common::Span;
use vela_syntax::{
    lexer::lex,
    token::{InterpolatedStringTokenPart, Symbol, Token, TokenKind},
};

pub(super) fn expand(tokens: Vec<Token>) -> Vec<Token> {
    let mut result = Vec::new();
    let mut pending: Vec<_> = tokens.into_iter().rev().map(|token| (token, 0)).collect();
    while let Some((mut token, offset)) = pending.pop() {
        token.span.start += offset;
        token.span.end += offset;
        let TokenKind::InterpolatedString(parts) = &token.kind else {
            if !matches!(token.kind, TokenKind::Eof) {
                result.push(token);
            }
            continue;
        };
        let mut cursor = token.span.start;
        let mut expanded = Vec::new();
        for part in parts {
            let InterpolatedStringTokenPart::Expr { source, span } = part else {
                continue;
            };
            let start = span.start + offset;
            let end = span.end + offset;
            push_chunk(&mut expanded, token.span, cursor, start - 1);
            expanded.push((
                Token {
                    kind: TokenKind::Symbol(Symbol::LBrace),
                    span: Span::new(token.span.source, start - 1, start),
                },
                0,
            ));
            expanded.extend(
                lex(span.source, source)
                    .tokens
                    .into_iter()
                    .map(|token| (token, start)),
            );
            expanded.push((
                Token {
                    kind: TokenKind::Symbol(Symbol::RBrace),
                    span: Span::new(token.span.source, end, end + 1),
                },
                0,
            ));
            cursor = end + 1;
        }
        push_chunk(&mut expanded, token.span, cursor, token.span.end);
        pending.extend(expanded.into_iter().rev());
    }
    result
}

fn push_chunk(tokens: &mut Vec<(Token, u32)>, span: Span, start: u32, end: u32) {
    if start < end {
        tokens.push((
            Token {
                kind: TokenKind::String(String::new()),
                span: Span::new(span.source, start, end),
            },
            0,
        ));
    }
}
