use vela_common::{Diagnostic, SourceId, Span};

use crate::ast::{
    Argument, AssignOp, Attribute, BinaryOp, Block, ConstItem, ElseBranch, EnumVariant,
    EnumVariantFields, Expr, ExprKind, FunctionItem, GlobalItem, IfExpr, ImplItem, ImplMethod,
    InterpolatedStringPart, Item, ItemKind, Literal, MapEntry, MatchArm, MatchExpr, Param, Pattern,
    RecordField, RecordPatternField, SourceFile, Stmt, StmtKind, StructField, StructItem,
    TraitItem, TraitMethod, TypeHint, UnaryOp, UseItem, Visibility,
};
use crate::attribute::normalize_attribute_value;
use crate::lexer::{lex, lex_at};
use crate::token::{InterpolatedStringTokenPart, Keyword, Symbol, Token, TokenKind};

#[must_use]
pub fn parse_source(source: SourceId, text: &str) -> SourceFile {
    let lexed = lex(source, text);
    Parser::new(lexed.tokens, lexed.diagnostics).parse()
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

    fn parse(mut self) -> SourceFile {
        let mut items = Vec::new();
        while !self.at_eof() {
            if let Some(item) = self.parse_item() {
                items.push(item);
            } else {
                self.recover_to_next_item();
            }
        }

        SourceFile {
            items,
            diagnostics: self.diagnostics,
        }
    }
}

mod expressions;
mod items;
mod lists;
mod recovery;
mod statements;
mod types;

#[cfg(test)]
mod tests;
