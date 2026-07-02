use vela_common::{Diagnostic, SourceId, Span};

use crate::ast::{
    Argument, AssignOp, Attribute, BinaryOp, Block, ElseBranch, Expr, ExprKind, IfExpr,
    InterpolatedStringPart, Literal, MapEntry, MatchArm, MatchExpr, Param, Pattern, RecordField,
    RecordPatternField, Stmt, StmtKind, TypeHint, UnaryOp,
};
use crate::attribute::normalize_attribute_value;
use crate::lexer::lex_at;
use crate::token::{InterpolatedStringTokenPart, Keyword, Symbol, Token, TokenKind};

#[must_use]
pub fn parse_body_blocks_at_spans(source: SourceId, text: &str, spans: &[Span]) -> Vec<Block> {
    spans
        .iter()
        .filter(|span| span.source == source)
        .filter_map(|span| parse_body_block_at_span(source, text, *span))
        .collect()
}

fn parse_body_block_at_span(source: SourceId, text: &str, span: Span) -> Option<Block> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    let body_text = text.get(start..end)?;
    let lexed = lex_at(source, body_text, span.start);
    Parser::new(lexed.tokens, lexed.diagnostics)
        .parse_block()
        .filter(|body| body.span == span)
}

fn parse_expression_fragment(
    source: SourceId,
    text: &str,
    base_offset: u32,
) -> (Expr, Vec<Diagnostic>) {
    let lexed = lex_at(source, text, base_offset);
    let start_span = lexed.tokens.first().map_or_else(
        || Span::new(source, base_offset, base_offset),
        |token| token.span,
    );
    let mut parser = Parser::new(lexed.tokens, lexed.diagnostics);
    if parser.at_eof() {
        parser.diagnostics.push(
            Diagnostic::error("expected expression in string interpolation")
                .with_code("E_PARSE")
                .with_span(start_span),
        );
        return (
            Expr {
                kind: ExprKind::Error,
                span: start_span,
            },
            parser.diagnostics,
        );
    }
    let expr = parser.parse_expression();
    if !parser.at_eof() {
        parser.error_here("expected end of string interpolation expression");
    }
    (expr, parser.diagnostics)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
    allow_record_literals: bool,
}

impl Parser {
    fn new(tokens: Vec<Token>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics,
            allow_record_literals: true,
        }
    }
}

pub(crate) mod cst;
mod expressions;
mod items;
mod lists;
mod recovery;
mod statements;
mod types;

#[cfg(test)]
mod tests;
