use super::*;
use crate::ast::{FloatLiteral, FloatSuffix, IntRadix, IntegerLiteral, IntegerSuffix};
use crate::lexer::lex;
use crate::token::{Keyword, Symbol, TokenKind};

fn source_id() -> SourceId {
    SourceId::new(1)
}

fn int_token(text: impl Into<String>, radix: IntRadix, suffix: Option<IntegerSuffix>) -> TokenKind {
    TokenKind::Int(IntegerLiteral {
        text: text.into(),
        radix,
        suffix,
    })
}

fn float_token(text: impl Into<String>, suffix: Option<FloatSuffix>) -> TokenKind {
    TokenKind::Float(FloatLiteral {
        text: text.into(),
        suffix,
    })
}

mod items;
mod lexer;
mod snapshots;
mod statements_and_expressions;
mod types_and_schema;
