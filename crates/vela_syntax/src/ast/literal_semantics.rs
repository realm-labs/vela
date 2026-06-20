use vela_common::SourceId;

use crate::SyntaxKind;
use crate::ast::Literal;
use crate::lexer::lex;
use crate::token::TokenKind;

pub(crate) fn literal_from_token(kind: SyntaxKind, text: &str) -> Option<Literal> {
    match kind {
        SyntaxKind::TrueKw => Some(Literal::Bool(true)),
        SyntaxKind::FalseKw => Some(Literal::Bool(false)),
        SyntaxKind::NullKw => Some(Literal::Null),
        SyntaxKind::Int
        | SyntaxKind::Float
        | SyntaxKind::Char
        | SyntaxKind::String
        | SyntaxKind::Bytes => literal_from_token_text(text),
        SyntaxKind::InterpolatedString => None,
        _ => None,
    }
}

fn literal_from_token_text(text: &str) -> Option<Literal> {
    lex(SourceId::new(0), text)
        .tokens
        .into_iter()
        .find_map(|token| match token.kind {
            TokenKind::Int(value) => Some(Literal::Integer(value)),
            TokenKind::Float(value) => Some(Literal::Float(value)),
            TokenKind::Char(value) => Some(Literal::Char(value)),
            TokenKind::String(value) => Some(Literal::String(value)),
            TokenKind::Bytes(value) => Some(Literal::Bytes(value)),
            _ => None,
        })
}
