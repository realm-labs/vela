use vela_common::{Diagnostic, SourceId, Span};

use crate::ast::{
    Argument, AssignOp, Attribute, BinaryOp, Block, ConstItem, ElseBranch, EnumVariant,
    EnumVariantFields, Expr, ExprKind, FunctionItem, IfExpr, ImplItem, ImplMethod, Item, ItemKind,
    Literal, MapEntry, MatchArm, MatchExpr, Param, Pattern, RecordField, RecordPatternField,
    SourceFile, Stmt, StmtKind, StructField, StructItem, TraitItem, TraitMethod, TypeHint, UnaryOp,
    UseItem, Visibility,
};
use crate::attribute::normalize_attribute_value;
use crate::lexer::lex;
use crate::token::{Keyword, Symbol, Token, TokenKind};

#[must_use]
pub fn parse_source(source: SourceId, text: &str) -> SourceFile {
    let lexed = lex(source, text);
    Parser::new(lexed.tokens, lexed.diagnostics).parse()
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
