use vela_common::{Diagnostic, SourceId, Span};

use crate::ast::{
    Argument, AssignOp, Attribute, BinaryOp, Block, ElseBranch, Expr, ExprKind, FunctionItem,
    IfExpr, Item, ItemKind, Literal, MapEntry, MatchArm, MatchExpr, Param, Pattern, RecordField,
    RecordPatternField, SourceFile, Stmt, StmtKind, StructField, StructItem, TraitItem,
    TraitMethod, TypeHint, UnaryOp, UseItem, Visibility,
};
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

    fn parse_item(&mut self) -> Option<Item> {
        let attrs = self.parse_attributes();
        let start = attrs
            .first()
            .map_or_else(|| self.current().span.start, |attr| attr.span.start);
        let visibility = if self.eat_keyword(Keyword::Pub).is_some() {
            Visibility::Public
        } else {
            Visibility::Private
        };

        let kind = if self.eat_keyword(Keyword::Use).is_some() {
            self.parse_use_item().map(ItemKind::Use)
        } else if self.eat_keyword(Keyword::Fn).is_some() {
            self.parse_function_item().map(ItemKind::Function)
        } else if self.eat_keyword(Keyword::Struct).is_some() {
            self.parse_struct_item().map(ItemKind::Struct)
        } else if self.eat_keyword(Keyword::Enum).is_some() {
            self.parse_enum_item().map(ItemKind::Enum)
        } else if self.eat_keyword(Keyword::Trait).is_some() {
            self.parse_trait_item().map(ItemKind::Trait)
        } else {
            self.error_here("expected item");
            return None;
        }?;

        let end = self.previous_span().end;
        Some(Item {
            attrs,
            visibility,
            kind,
            span: Span::new(self.current().span.source, start, end),
        })
    }

    fn parse_attributes(&mut self) -> Vec<Attribute> {
        let mut attrs = Vec::new();
        while self.check_symbol(Symbol::Hash) && self.check_next_symbol(Symbol::LBracket) {
            let start = self.advance().span.start;
            self.advance();
            let path = self.parse_path();
            self.skip_balanced_until(Symbol::RBracket);
            let end = self.previous_span().end;
            attrs.push(Attribute {
                path,
                span: Span::new(self.current().span.source, start, end),
            });
        }
        attrs
    }

    fn parse_use_item(&mut self) -> Option<UseItem> {
        let path = self.parse_path();
        if path.is_empty() {
            self.error_here("expected use path");
            return None;
        }
        self.eat_symbol(Symbol::Semicolon);
        Some(UseItem { path })
    }

    fn parse_function_item(&mut self) -> Option<FunctionItem> {
        let name = self.expect_ident("expected function name")?;
        let params = self.parse_parameter_list();
        let return_type = self.parse_optional_return_type();
        let body = self.parse_block()?;
        Some(FunctionItem {
            name,
            params,
            return_type,
            body,
        })
    }

    fn parse_struct_item(&mut self) -> Option<StructItem> {
        let name = self.expect_ident("expected struct name")?;
        let fields = self.parse_struct_fields_in_braces();
        Some(StructItem { name, fields })
    }

    fn parse_enum_item(&mut self) -> Option<crate::ast::EnumItem> {
        let name = self.expect_ident("expected enum name")?;
        let variants = self.parse_named_members_in_braces();
        Some(crate::ast::EnumItem { name, variants })
    }

    fn parse_trait_item(&mut self) -> Option<TraitItem> {
        let name = self.expect_ident("expected trait name")?;
        let mut methods = Vec::new();
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected trait body");
            return Some(TraitItem { name, methods });
        }

        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            self.parse_attributes();
            if self.eat_keyword(Keyword::Fn).is_some() {
                if let Some(method) = self.expect_ident("expected trait method name") {
                    let params = self.parse_parameter_list();
                    let return_type = self.parse_optional_return_type();
                    methods.push(TraitMethod {
                        name: method,
                        params,
                        return_type,
                    });
                } else {
                    self.parse_parameter_list();
                    self.parse_optional_return_type();
                }
                if self.check_symbol(Symbol::LBrace) {
                    self.skip_block_tokens();
                } else {
                    self.eat_symbol(Symbol::Semicolon);
                }
            } else {
                self.error_here("expected trait item");
                self.advance();
            }
        }

        self.eat_symbol(Symbol::RBrace);
        Some(TraitItem { name, methods })
    }

    fn parse_block(&mut self) -> Option<Block> {
        let start = self.eat_symbol(Symbol::LBrace)?.span.start;
        let mut statements = Vec::new();
        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            } else {
                self.advance();
            }
        }
        if self.eat_symbol(Symbol::RBrace).is_none() {
            self.error_here("expected `}`");
        }
        let end = self.previous_span().end;
        Some(Block {
            statements,
            span: Span::new(self.current().span.source, start, end),
        })
    }

    fn parse_statement(&mut self) -> Option<Stmt> {
        self.parse_attributes();
        let start = self.current().span.start;

        let kind = if self.eat_keyword(Keyword::Let).is_some() {
            self.parse_let_statement()
        } else if self.eat_keyword(Keyword::Return).is_some() {
            let value = if self.is_statement_boundary() {
                None
            } else {
                Some(self.parse_expression())
            };
            StmtKind::Return(value)
        } else if self.eat_keyword(Keyword::Break).is_some() {
            StmtKind::Break
        } else if self.eat_keyword(Keyword::Continue).is_some() {
            StmtKind::Continue
        } else if self.eat_keyword(Keyword::For).is_some() {
            self.parse_for_statement()
        } else if self.check_symbol(Symbol::LBrace) {
            StmtKind::Block(self.parse_block()?)
        } else {
            StmtKind::Expr(self.parse_expression())
        };

        self.eat_symbol(Symbol::Semicolon);
        let end = self.previous_span().end;
        Some(Stmt {
            kind,
            span: Span::new(self.current().span.source, start, end),
        })
    }

    fn parse_let_statement(&mut self) -> StmtKind {
        let name = self
            .expect_ident("expected binding name")
            .unwrap_or_default();
        let type_hint = self.parse_type_annotation();
        let value = if self.eat_symbol(Symbol::Equal).is_some() {
            Some(self.parse_expression())
        } else {
            None
        };
        StmtKind::Let {
            name,
            type_hint,
            value,
        }
    }

    fn parse_for_statement(&mut self) -> StmtKind {
        let binding = self
            .expect_ident("expected loop binding")
            .unwrap_or_default();
        if self.eat_keyword(Keyword::In).is_none() {
            self.error_here("expected `in`");
        }
        let iterable = self.parse_expression_before_block();
        let body = self.parse_block().unwrap_or_else(|| Block {
            statements: Vec::new(),
            span: self.current().span,
        });
        StmtKind::For {
            binding,
            iterable,
            body,
        }
    }

    fn parse_expression(&mut self) -> Expr {
        self.parse_assignment()
    }

    fn parse_expression_before_block(&mut self) -> Expr {
        let previous = self.allow_record_literals;
        self.allow_record_literals = false;
        let expr = self.parse_expression();
        self.allow_record_literals = previous;
        expr
    }

    fn parse_assignment(&mut self) -> Expr {
        let left = self.parse_logical_or();
        let Some(op) = self.eat_assign_op() else {
            return left;
        };
        let value = self.parse_assignment();
        let span = self.join_span(left.span, value.span);
        Expr {
            kind: ExprKind::Assign {
                op,
                target: Box::new(left),
                value: Box::new(value),
            },
            span,
        }
    }

    fn parse_logical_or(&mut self) -> Expr {
        self.parse_binary_left_assoc(Self::parse_logical_and, &[(Symbol::OrOr, BinaryOp::Or)])
    }

    fn parse_logical_and(&mut self) -> Expr {
        self.parse_binary_left_assoc(Self::parse_equality, &[(Symbol::AndAnd, BinaryOp::And)])
    }

    fn parse_equality(&mut self) -> Expr {
        self.parse_binary_left_assoc(
            Self::parse_comparison,
            &[
                (Symbol::EqualEqual, BinaryOp::Equal),
                (Symbol::BangEqual, BinaryOp::NotEqual),
            ],
        )
    }

    fn parse_comparison(&mut self) -> Expr {
        self.parse_binary_left_assoc(
            Self::parse_additive,
            &[
                (Symbol::Less, BinaryOp::Less),
                (Symbol::LessEqual, BinaryOp::LessEqual),
                (Symbol::Greater, BinaryOp::Greater),
                (Symbol::GreaterEqual, BinaryOp::GreaterEqual),
            ],
        )
    }

    fn parse_additive(&mut self) -> Expr {
        self.parse_binary_left_assoc(
            Self::parse_multiplicative,
            &[
                (Symbol::Plus, BinaryOp::Add),
                (Symbol::Minus, BinaryOp::Sub),
            ],
        )
    }

    fn parse_multiplicative(&mut self) -> Expr {
        self.parse_binary_left_assoc(
            Self::parse_unary,
            &[
                (Symbol::Star, BinaryOp::Mul),
                (Symbol::Slash, BinaryOp::Div),
                (Symbol::Percent, BinaryOp::Rem),
            ],
        )
    }

    fn parse_binary_left_assoc(
        &mut self,
        parse_operand: fn(&mut Self) -> Expr,
        ops: &[(Symbol, BinaryOp)],
    ) -> Expr {
        let mut expr = parse_operand(self);
        while let Some((symbol, op)) = ops
            .iter()
            .find(|(symbol, _)| self.check_symbol(*symbol))
            .copied()
        {
            self.eat_symbol(symbol);
            let right = parse_operand(self);
            let span = self.join_span(expr.span, right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        expr
    }

    fn parse_unary(&mut self) -> Expr {
        let start = self.current().span;
        let op = if self.eat_symbol(Symbol::Bang).is_some() {
            Some(UnaryOp::Not)
        } else if self.eat_symbol(Symbol::Minus).is_some() {
            Some(UnaryOp::Negate)
        } else {
            None
        };

        if let Some(op) = op {
            let expr = self.parse_unary();
            return Expr {
                span: self.join_span(start, expr.span),
                kind: ExprKind::Unary {
                    op,
                    expr: Box::new(expr),
                },
            };
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        loop {
            if self.check_symbol(Symbol::LParen) {
                let args = self.parse_argument_list();
                let span = self.join_span(expr.span, self.previous_span());
                expr = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    span,
                };
            } else if self.eat_symbol(Symbol::Dot).is_some() {
                let name = self.expect_ident("expected field name").unwrap_or_default();
                let span = self.join_span(expr.span, self.previous_span());
                expr = Expr {
                    kind: ExprKind::Field {
                        base: Box::new(expr),
                        name,
                    },
                    span,
                };
            } else if self.eat_symbol(Symbol::LBracket).is_some() {
                let index = self.parse_expression();
                if self.eat_symbol(Symbol::RBracket).is_none() {
                    self.error_here("expected `]`");
                }
                let span = self.join_span(expr.span, self.previous_span());
                expr = Expr {
                    kind: ExprKind::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                    },
                    span,
                };
            } else if self.eat_symbol(Symbol::Question).is_some() {
                let span = self.join_span(expr.span, self.previous_span());
                expr = Expr {
                    kind: ExprKind::Try(Box::new(expr)),
                    span,
                };
            } else {
                break;
            }
        }
        expr
    }

    fn parse_primary(&mut self) -> Expr {
        let span = self.current().span;
        match self.current().kind.clone() {
            TokenKind::Keyword(Keyword::True) => {
                self.advance();
                self.literal_expr(Literal::Bool(true), span)
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance();
                self.literal_expr(Literal::Bool(false), span)
            }
            TokenKind::Keyword(Keyword::Null) => {
                self.advance();
                self.literal_expr(Literal::Null, span)
            }
            TokenKind::Keyword(Keyword::SelfValue) => {
                self.advance();
                Expr {
                    kind: ExprKind::SelfValue,
                    span,
                }
            }
            TokenKind::Keyword(Keyword::If) => self.parse_if_expression(),
            TokenKind::Keyword(Keyword::Match) => self.parse_match_expression(),
            TokenKind::Int(value) => {
                self.advance();
                self.literal_expr(Literal::Int(value), span)
            }
            TokenKind::Float(value) => {
                self.advance();
                self.literal_expr(Literal::Float(value), span)
            }
            TokenKind::String(value) => {
                self.advance();
                self.literal_expr(Literal::String(value), span)
            }
            TokenKind::Ident(_) => self.parse_path_or_record(),
            TokenKind::Symbol(Symbol::LParen) => self.parse_grouped_expression(),
            TokenKind::Symbol(Symbol::LBracket) => self.parse_array_expression(),
            TokenKind::Symbol(Symbol::LBrace) if self.looks_like_map_literal() => {
                self.parse_map_expression()
            }
            TokenKind::Symbol(Symbol::LBrace) => {
                let block = self.parse_block().unwrap_or(Block {
                    statements: Vec::new(),
                    span,
                });
                Expr {
                    span: block.span,
                    kind: ExprKind::Block(block),
                }
            }
            TokenKind::Symbol(Symbol::Pipe) => self.parse_lambda_expression(),
            _ => {
                self.error_here("expected expression");
                self.advance();
                Expr {
                    kind: ExprKind::Error,
                    span,
                }
            }
        }
    }

    fn literal_expr(&self, literal: Literal, span: Span) -> Expr {
        Expr {
            kind: ExprKind::Literal(literal),
            span,
        }
    }

    fn parse_grouped_expression(&mut self) -> Expr {
        self.eat_symbol(Symbol::LParen);
        let expr = self.parse_expression();
        if self.eat_symbol(Symbol::RParen).is_none() {
            self.error_here("expected `)`");
        }
        expr
    }

    fn parse_array_expression(&mut self) -> Expr {
        let start = self.eat_symbol(Symbol::LBracket).expect("checked").span;
        let mut items = Vec::new();
        while !self.at_eof() && !self.check_symbol(Symbol::RBracket) {
            items.push(self.parse_expression());
            if self.eat_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        if self.eat_symbol(Symbol::RBracket).is_none() {
            self.error_here("expected `]`");
        }
        Expr {
            kind: ExprKind::Array(items),
            span: self.join_span(start, self.previous_span()),
        }
    }

    fn parse_map_expression(&mut self) -> Expr {
        let start = self.eat_symbol(Symbol::LBrace).expect("checked").span;
        let mut entries = Vec::new();
        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            let key = self.parse_map_key();
            if self.eat_symbol(Symbol::Colon).is_none() {
                self.error_here("expected `:` in map literal");
            }
            let value = self.parse_expression();
            entries.push(MapEntry { key, value });
            if self.eat_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        if self.eat_symbol(Symbol::RBrace).is_none() {
            self.error_here("expected `}`");
        }
        Expr {
            kind: ExprKind::Map(entries),
            span: self.join_span(start, self.previous_span()),
        }
    }

    fn parse_map_key(&mut self) -> Expr {
        match self.current().kind.clone() {
            TokenKind::Ident(_) => self.parse_path_or_record(),
            TokenKind::String(value) => {
                let span = self.advance().span;
                self.literal_expr(Literal::String(value), span)
            }
            TokenKind::Int(value) => {
                let span = self.advance().span;
                self.literal_expr(Literal::Int(value), span)
            }
            TokenKind::Float(value) => {
                let span = self.advance().span;
                self.literal_expr(Literal::Float(value), span)
            }
            _ => {
                self.error_here("expected map key");
                let span = self.advance().span;
                Expr {
                    kind: ExprKind::Error,
                    span,
                }
            }
        }
    }

    fn parse_path_or_record(&mut self) -> Expr {
        let start = self.current().span;
        let path = self.parse_path();
        if self.allow_record_literals && self.check_symbol(Symbol::LBrace) {
            let fields = self.parse_record_fields();
            return Expr {
                kind: ExprKind::Record { path, fields },
                span: self.join_span(start, self.previous_span()),
            };
        }
        Expr {
            kind: ExprKind::Path(path),
            span: self.join_span(start, self.previous_span()),
        }
    }

    fn parse_record_fields(&mut self) -> Vec<RecordField> {
        self.eat_symbol(Symbol::LBrace);
        let mut fields = Vec::new();
        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            let name = self
                .expect_ident("expected record field")
                .unwrap_or_default();
            let value = if self.eat_symbol(Symbol::Colon).is_some() {
                Some(self.parse_expression())
            } else {
                None
            };
            fields.push(RecordField { name, value });
            if self.eat_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        if self.eat_symbol(Symbol::RBrace).is_none() {
            self.error_here("expected `}`");
        }
        fields
    }

    fn parse_lambda_expression(&mut self) -> Expr {
        let start = self.eat_symbol(Symbol::Pipe).expect("checked").span;
        let mut params = Vec::new();
        while !self.at_eof() && !self.check_symbol(Symbol::Pipe) {
            if let Some(param) = self.eat_ident() {
                let type_hint = self.parse_type_annotation();
                params.push(Param {
                    name: param,
                    type_hint,
                });
            } else {
                self.error_here("expected lambda parameter");
                self.advance();
            }
            if self.eat_symbol(Symbol::Comma).is_none() && !self.check_symbol(Symbol::Pipe) {
                self.error_here("expected `,` or `|` in lambda parameter list");
                break;
            }
        }
        if self.eat_symbol(Symbol::Pipe).is_none() {
            self.error_here("expected `|`");
        }
        let body = if self.check_symbol(Symbol::LBrace) {
            let block = self.parse_block().unwrap_or(Block {
                statements: Vec::new(),
                span: self.current().span,
            });
            Expr {
                span: block.span,
                kind: ExprKind::Block(block),
            }
        } else {
            self.parse_expression()
        };
        Expr {
            span: self.join_span(start, body.span),
            kind: ExprKind::Lambda {
                params,
                body: Box::new(body),
            },
        }
    }

    fn parse_if_expression(&mut self) -> Expr {
        let start = self.eat_keyword(Keyword::If).expect("checked").span;
        let condition = self.parse_expression_before_block();
        let then_branch = self.parse_block().unwrap_or(Block {
            statements: Vec::new(),
            span: self.current().span,
        });
        let else_branch = if self.eat_keyword(Keyword::Else).is_some() {
            if self.check_keyword(Keyword::If) {
                let else_if = self.parse_if_expression();
                match else_if.kind {
                    ExprKind::If(if_expr) => Some(ElseBranch::If(if_expr)),
                    _ => None,
                }
            } else {
                self.parse_block().map(ElseBranch::Block)
            }
        } else {
            None
        };
        let span = self.join_span(start, self.previous_span());
        Expr {
            kind: ExprKind::If(Box::new(IfExpr {
                condition,
                then_branch,
                else_branch,
            })),
            span,
        }
    }

    fn parse_match_expression(&mut self) -> Expr {
        let start = self.eat_keyword(Keyword::Match).expect("checked").span;
        let scrutinee = self.parse_expression_before_block();
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected match body");
        }
        let mut arms = Vec::new();
        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            let pattern = self.parse_pattern();
            let guard = if self.eat_keyword(Keyword::If).is_some() {
                Some(self.parse_expression())
            } else {
                None
            };
            if self.eat_symbol(Symbol::FatArrow).is_none() {
                self.error_here("expected `=>`");
            }
            let body = if self.check_symbol(Symbol::LBrace) {
                let block = self.parse_block().unwrap_or(Block {
                    statements: Vec::new(),
                    span: self.current().span,
                });
                Expr {
                    span: block.span,
                    kind: ExprKind::Block(block),
                }
            } else {
                self.parse_expression()
            };
            arms.push(MatchArm {
                pattern,
                guard,
                body,
            });
            self.eat_symbol(Symbol::Comma);
            self.eat_symbol(Symbol::Semicolon);
        }
        if self.eat_symbol(Symbol::RBrace).is_none() {
            self.error_here("expected `}`");
        }
        Expr {
            kind: ExprKind::Match(Box::new(MatchExpr { scrutinee, arms })),
            span: self.join_span(start, self.previous_span()),
        }
    }

    fn parse_pattern(&mut self) -> Pattern {
        match self.current().kind.clone() {
            TokenKind::Ident(name) if name == "_" => {
                self.advance();
                Pattern::Wildcard
            }
            TokenKind::Keyword(Keyword::True) => {
                self.advance();
                Pattern::Literal(Literal::Bool(true))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance();
                Pattern::Literal(Literal::Bool(false))
            }
            TokenKind::Keyword(Keyword::Null) => {
                self.advance();
                Pattern::Literal(Literal::Null)
            }
            TokenKind::String(value) => {
                self.advance();
                Pattern::Literal(Literal::String(value))
            }
            TokenKind::Int(value) => {
                self.advance();
                Pattern::Literal(Literal::Int(value))
            }
            TokenKind::Float(value) => {
                self.advance();
                Pattern::Literal(Literal::Float(value))
            }
            TokenKind::Ident(_) => self.parse_path_pattern(),
            _ => {
                self.error_here("expected pattern");
                self.advance();
                Pattern::Wildcard
            }
        }
    }

    fn parse_path_pattern(&mut self) -> Pattern {
        let path = self.parse_path();
        if self.eat_symbol(Symbol::LParen).is_some() {
            let mut fields = Vec::new();
            while !self.at_eof() && !self.check_symbol(Symbol::RParen) {
                fields.push(self.parse_pattern());
                if self.eat_symbol(Symbol::Comma).is_none() {
                    break;
                }
            }
            if self.eat_symbol(Symbol::RParen).is_none() {
                self.error_here("expected `)`");
            }
            Pattern::TupleVariant { path, fields }
        } else if self.eat_symbol(Symbol::LBrace).is_some() {
            let mut fields = Vec::new();
            while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
                let name = self
                    .expect_ident("expected pattern field")
                    .unwrap_or_default();
                let pattern = if self.eat_symbol(Symbol::Colon).is_some() {
                    Some(self.parse_pattern())
                } else {
                    None
                };
                fields.push(RecordPatternField { name, pattern });
                if self.eat_symbol(Symbol::Comma).is_none() {
                    break;
                }
            }
            if self.eat_symbol(Symbol::RBrace).is_none() {
                self.error_here("expected `}`");
            }
            Pattern::RecordVariant { path, fields }
        } else if path.len() == 1 {
            Pattern::Binding(path.into_iter().next().unwrap_or_default())
        } else {
            Pattern::Path(path)
        }
    }

    fn parse_argument_list(&mut self) -> Vec<Argument> {
        let mut args = Vec::new();
        self.eat_symbol(Symbol::LParen);
        while !self.at_eof() && !self.check_symbol(Symbol::RParen) {
            let name = if self.check_ident() && self.check_next_symbol(Symbol::Equal) {
                let name = self.eat_ident();
                self.eat_symbol(Symbol::Equal);
                name
            } else {
                None
            };
            let value = self.parse_expression();
            args.push(Argument { name, value });
            if self.eat_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        if self.eat_symbol(Symbol::RParen).is_none() {
            self.error_here("expected `)`");
        }
        args
    }

    fn parse_parameter_list(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        if self.eat_symbol(Symbol::LParen).is_none() {
            self.error_here("expected parameter list");
            return params;
        }

        while !self.at_eof() && !self.check_symbol(Symbol::RParen) {
            if let Some(param) = self.eat_ident() {
                let type_hint = self.parse_type_annotation();
                params.push(Param {
                    name: param,
                    type_hint,
                });
                self.skip_parameter_tail();
            } else {
                self.advance();
            }

            if self.eat_symbol(Symbol::Comma).is_none() && !self.check_symbol(Symbol::RParen) {
                self.error_here("expected `,` or `)` in parameter list");
                self.recover_until(&[Symbol::Comma, Symbol::RParen]);
                self.eat_symbol(Symbol::Comma);
            }
        }

        self.eat_symbol(Symbol::RParen);
        params
    }

    fn parse_struct_fields_in_braces(&mut self) -> Vec<StructField> {
        let mut fields = Vec::new();
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected `{`");
            return fields;
        }

        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            self.parse_attributes();
            if let Some(name) = self.eat_ident() {
                let type_hint = self.parse_type_annotation();
                fields.push(StructField { name, type_hint });
                self.skip_member_tail();
            } else {
                self.advance();
            }
            self.eat_symbol(Symbol::Comma);
            self.eat_symbol(Symbol::Semicolon);
        }

        self.eat_symbol(Symbol::RBrace);
        fields
    }

    fn parse_named_members_in_braces(&mut self) -> Vec<String> {
        let mut names = Vec::new();
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected `{`");
            return names;
        }

        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            self.parse_attributes();
            if let Some(name) = self.eat_ident() {
                names.push(name);
                self.skip_member_tail();
            } else {
                self.advance();
            }
            self.eat_symbol(Symbol::Comma);
            self.eat_symbol(Symbol::Semicolon);
        }

        self.eat_symbol(Symbol::RBrace);
        names
    }

    fn parse_path(&mut self) -> Vec<String> {
        let mut parts = Vec::new();
        let Some(first) = self.eat_ident() else {
            return parts;
        };
        parts.push(first);

        while self.eat_symbol(Symbol::Dot).is_some() {
            if let Some(part) = self.eat_ident() {
                parts.push(part);
            } else {
                self.error_here("expected path segment");
                break;
            }
        }
        parts
    }

    fn looks_like_map_literal(&self) -> bool {
        if !self.check_symbol(Symbol::LBrace) {
            return false;
        }
        let mut depth = 0_u32;
        let mut index = self.pos.saturating_add(1);
        while let Some(token) = self.tokens.get(index) {
            match token.kind {
                TokenKind::Symbol(Symbol::LBrace | Symbol::LBracket | Symbol::LParen) => {
                    depth = depth.saturating_add(1);
                }
                TokenKind::Symbol(Symbol::RBrace) if depth == 0 => return false,
                TokenKind::Symbol(Symbol::RBrace | Symbol::RBracket | Symbol::RParen) => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Symbol(Symbol::Colon) if depth == 0 => return true,
                TokenKind::Symbol(Symbol::Comma | Symbol::Semicolon) if depth == 0 => {
                    return false;
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            index = index.saturating_add(1);
        }
        false
    }

    fn skip_parameter_tail(&mut self) {
        let mut depth = 0_u32;
        while !self.at_eof() {
            if depth == 0 && (self.check_symbol(Symbol::Comma) || self.check_symbol(Symbol::RParen))
            {
                break;
            }
            self.bump_depth(&mut depth);
            self.advance();
        }
    }

    fn skip_member_tail(&mut self) {
        let mut depth = 0_u32;
        while !self.at_eof() {
            if depth == 0
                && (self.check_symbol(Symbol::Comma)
                    || self.check_symbol(Symbol::Semicolon)
                    || self.check_symbol(Symbol::RBrace))
            {
                break;
            }
            self.bump_depth(&mut depth);
            self.advance();
        }
    }

    fn parse_type_annotation(&mut self) -> Option<TypeHint> {
        self.eat_symbol(Symbol::Colon)?;
        self.parse_type_hint()
    }

    fn parse_optional_return_type(&mut self) -> Option<TypeHint> {
        if self.eat_symbol(Symbol::Arrow).is_some() {
            return self.parse_type_hint();
        }
        None
    }

    fn parse_type_hint(&mut self) -> Option<TypeHint> {
        let start = self.current().span;
        let Some(first) = self.eat_type_hint_segment() else {
            self.error_here("expected type hint");
            return None;
        };
        let mut path = vec![first];

        while self.eat_symbol(Symbol::Dot).is_some() {
            if let Some(segment) = self.eat_type_hint_segment() {
                path.push(segment);
            } else {
                self.error_here("expected type path segment");
                break;
            }
        }

        if self.check_symbol(Symbol::Less) {
            let generic_span = self.current().span;
            self.diagnostics.push(
                Diagnostic::error("script type hints do not support generics")
                    .with_code("syntax::generic_type_hint")
                    .with_span(generic_span)
                    .with_label(generic_span, "remove generic type arguments"),
            );
            self.skip_generic_type_arguments();
        }

        Some(TypeHint {
            path,
            span: self.join_span(start, self.previous_span()),
        })
    }

    fn eat_type_hint_segment(&mut self) -> Option<String> {
        match self.current().kind.clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Some(name)
            }
            TokenKind::Keyword(Keyword::Null) => {
                self.advance();
                Some("null".to_owned())
            }
            _ => None,
        }
    }

    fn skip_generic_type_arguments(&mut self) {
        let mut depth = 0_u32;
        while !(self.at_eof() || depth == 0 && self.is_type_hint_boundary()) {
            match self.current_symbol() {
                Some(Symbol::Less) => {
                    depth = depth.saturating_add(1);
                    self.advance();
                }
                Some(Symbol::Greater) if depth > 0 => {
                    depth = depth.saturating_sub(1);
                    self.advance();
                    if depth == 0 {
                        break;
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn is_type_hint_boundary(&self) -> bool {
        self.check_symbol(Symbol::Equal)
            || self.check_symbol(Symbol::Comma)
            || self.check_symbol(Symbol::RParen)
            || self.check_symbol(Symbol::RBrace)
            || self.check_symbol(Symbol::LBrace)
            || self.check_symbol(Symbol::Pipe)
            || self.check_symbol(Symbol::Semicolon)
            || self.at_eof()
    }

    fn skip_block_tokens(&mut self) {
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected block");
            return;
        }
        self.skip_balanced_until(Symbol::RBrace);
    }

    fn skip_balanced_until(&mut self, close: Symbol) {
        let mut depth = 0_u32;
        while !self.at_eof() {
            if depth == 0 && self.check_symbol(close) {
                self.advance();
                return;
            }

            self.bump_depth(&mut depth);
            self.advance();
        }
        self.error_here("expected closing delimiter");
    }

    fn bump_depth(&self, depth: &mut u32) {
        match self.current_symbol() {
            Some(Symbol::LBrace | Symbol::LBracket | Symbol::LParen) => {
                *depth = depth.saturating_add(1);
            }
            Some(Symbol::RBrace | Symbol::RBracket | Symbol::RParen) if *depth > 0 => {
                *depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn recover_until(&mut self, symbols: &[Symbol]) {
        while !self.at_eof() && !symbols.iter().any(|symbol| self.check_symbol(*symbol)) {
            self.advance();
        }
    }

    fn recover_to_next_item(&mut self) {
        while !self.at_eof() {
            if self.check_keyword(Keyword::Pub)
                || self.check_keyword(Keyword::Use)
                || self.check_keyword(Keyword::Fn)
                || self.check_keyword(Keyword::Struct)
                || self.check_keyword(Keyword::Enum)
                || self.check_keyword(Keyword::Trait)
            {
                return;
            }
            self.advance();
        }
    }

    fn is_statement_boundary(&self) -> bool {
        self.check_symbol(Symbol::Semicolon)
            || self.check_symbol(Symbol::RBrace)
            || self.check_symbol(Symbol::Comma)
            || self.at_eof()
    }

    fn eat_assign_op(&mut self) -> Option<AssignOp> {
        let op = if self.eat_symbol(Symbol::Equal).is_some() {
            AssignOp::Set
        } else if self.eat_symbol(Symbol::PlusEqual).is_some() {
            AssignOp::Add
        } else if self.eat_symbol(Symbol::MinusEqual).is_some() {
            AssignOp::Sub
        } else if self.eat_symbol(Symbol::StarEqual).is_some() {
            AssignOp::Mul
        } else if self.eat_symbol(Symbol::SlashEqual).is_some() {
            AssignOp::Div
        } else if self.eat_symbol(Symbol::PercentEqual).is_some() {
            AssignOp::Rem
        } else {
            return None;
        };
        Some(op)
    }

    fn expect_ident(&mut self, message: &str) -> Option<String> {
        let ident = self.eat_ident();
        if ident.is_none() {
            self.error_here(message);
        }
        ident
    }

    fn eat_ident(&mut self) -> Option<String> {
        let ident = match &self.current().kind {
            TokenKind::Ident(ident) => Some(ident.clone()),
            _ => None,
        }?;
        self.advance();
        Some(ident)
    }

    fn check_ident(&self) -> bool {
        matches!(self.current().kind, TokenKind::Ident(_))
    }

    fn eat_keyword(&mut self, keyword: Keyword) -> Option<Token> {
        if self.check_keyword(keyword) {
            Some(self.advance())
        } else {
            None
        }
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(current) if current == keyword)
    }

    fn eat_symbol(&mut self, symbol: Symbol) -> Option<Token> {
        if self.check_symbol(symbol) {
            Some(self.advance())
        } else {
            None
        }
    }

    fn check_symbol(&self, symbol: Symbol) -> bool {
        matches!(self.current().kind, TokenKind::Symbol(current) if current == symbol)
    }

    fn check_next_symbol(&self, symbol: Symbol) -> bool {
        matches!(
            self.tokens.get(self.pos.saturating_add(1)).map(|token| &token.kind),
            Some(TokenKind::Symbol(current)) if *current == symbol
        )
    }

    fn current_symbol(&self) -> Option<Symbol> {
        match self.current().kind {
            TokenKind::Symbol(symbol) => Some(symbol),
            _ => None,
        }
    }

    fn current(&self) -> &Token {
        let index = self.pos.min(self.tokens.len().saturating_sub(1));
        &self.tokens[index]
    }

    fn previous_span(&self) -> Span {
        self.pos
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map_or_else(|| self.current().span, |token| token.span)
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if !self.at_eof() {
            self.pos = self.pos.saturating_add(1);
        }
        token
    }

    fn at_eof(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn error_here(&mut self, message: impl Into<String>) {
        self.diagnostics.push(
            Diagnostic::error(message)
                .with_code("E_PARSE")
                .with_span(self.current().span),
        );
    }

    fn join_span(&self, start: Span, end: Span) -> Span {
        Span::new(start.source, start.start, end.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BinaryOp, ExprKind, Keyword, Literal, StmtKind, Symbol, TokenKind, lex};
    use std::fmt::Write as _;

    fn source_id() -> SourceId {
        SourceId::new(1)
    }

    fn param_names(params: &[Param]) -> Vec<String> {
        params.iter().map(|param| param.name.clone()).collect()
    }

    fn struct_field_names(fields: &[StructField]) -> Vec<String> {
        fields.iter().map(|field| field.name.clone()).collect()
    }

    fn trait_method_names(methods: &[TraitMethod]) -> Vec<String> {
        methods.iter().map(|method| method.name.clone()).collect()
    }

    #[test]
    fn lexes_keywords_identifiers_and_operators_with_spans() {
        let lexed = lex(source_id(), "pub fn level_up(player) { player.level += 1 }");

        assert!(lexed.diagnostics.is_empty());
        assert_eq!(lexed.tokens[0].kind, TokenKind::Keyword(Keyword::Pub));
        assert_eq!(lexed.tokens[0].span, Span::new(source_id(), 0, 3));
        assert_eq!(lexed.tokens[2].kind, TokenKind::Ident("level_up".into()));
        assert!(
            lexed
                .tokens
                .iter()
                .any(|token| token.kind == TokenKind::Symbol(Symbol::PlusEqual))
        );
    }

    #[test]
    fn parses_core_module_items() {
        let parsed = parse_source(
            source_id(),
            r#"
use game.player.Player;

#[event("monster.kill")]
pub fn on_kill(ctx, player, monster) {
    player.exp += monster.exp
}

struct KillReward {
    item_id,
    count,
}

enum QuestProgress {
    None,
    Active { quest_id, count },
}

trait Damageable {
    fn damage(self, amount);
}
"#,
        );

        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.items.len(), 5);
        assert!(matches!(parsed.items[0].kind, ItemKind::Use(_)));

        let ItemKind::Function(function) = &parsed.items[1].kind else {
            panic!("expected function item");
        };
        assert_eq!(parsed.items[1].visibility, Visibility::Public);
        assert_eq!(function.name, "on_kill");
        assert_eq!(param_names(&function.params), ["ctx", "player", "monster"]);
        assert_eq!(function.body.statements.len(), 1);
        assert_eq!(parsed.items[1].attrs[0].path, ["event"]);

        let ItemKind::Struct(record) = &parsed.items[2].kind else {
            panic!("expected struct item");
        };
        assert_eq!(struct_field_names(&record.fields), ["item_id", "count"]);

        let ItemKind::Enum(enumeration) = &parsed.items[3].kind else {
            panic!("expected enum item");
        };
        assert_eq!(enumeration.variants, ["None", "Active"]);

        let ItemKind::Trait(trait_item) = &parsed.items[4].kind else {
            panic!("expected trait item");
        };
        assert_eq!(trait_method_names(&trait_item.methods), ["damage"]);
    }

    #[test]
    fn parses_function_body_statements_and_expressions() {
        let parsed = parse_source(
            source_id(),
            r#"
fn on_kill(ctx, player, monster) {
    let rewards = [monster.exp, 2 + 3 * 4];
    player.exp += monster.exp;
    if player.exp >= ctx.config.exp_to_next_level(player.level) {
        player.level += 1;
        player.exp = 0;
    } else {
        return null;
    }
    for reward in rewards {
        player.inventory.add(reward.item_id, reward.count);
    }
}
"#,
        );

        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let ItemKind::Function(function) = &parsed.items[0].kind else {
            panic!("expected function item");
        };
        assert_eq!(function.body.statements.len(), 4);
        assert!(matches!(
            function.body.statements[0].kind,
            StmtKind::Let { .. }
        ));
        assert!(matches!(
            function.body.statements[2].kind,
            StmtKind::Expr(Expr {
                kind: ExprKind::If(_),
                ..
            })
        ));
        assert!(matches!(
            function.body.statements[3].kind,
            StmtKind::For { .. }
        ));

        let StmtKind::Let {
            value: Some(value), ..
        } = &function.body.statements[0].kind
        else {
            panic!("expected initialized let");
        };
        let ExprKind::Array(items) = &value.kind else {
            panic!("expected array literal");
        };
        assert_eq!(items.len(), 2);
        assert!(matches!(
            items[1].kind,
            ExprKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
    }

    #[test]
    fn parses_match_lambda_record_and_map_expressions() {
        let parsed = parse_source(
            source_id(),
            r#"
fn update(player) {
    let values = {"level": player.level, count: 1};
    let reward = KillReward { item_id: "gold", count };
    let mapped = values.map(|entry| entry.value + 1);
    match player.quest_progress {
        QuestProgress.Active { quest_id, count } => {
            player.quest_progress = QuestProgress.Active { quest_id, count: count + 1 };
        },
        _ => reward,
    }
}
"#,
        );

        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let ItemKind::Function(function) = &parsed.items[0].kind else {
            panic!("expected function item");
        };
        assert_eq!(function.body.statements.len(), 4);

        let StmtKind::Let {
            value: Some(map), ..
        } = &function.body.statements[0].kind
        else {
            panic!("expected map let");
        };
        assert!(matches!(map.kind, ExprKind::Map(_)));

        let StmtKind::Let {
            value: Some(record),
            ..
        } = &function.body.statements[1].kind
        else {
            panic!("expected record let");
        };
        assert!(matches!(record.kind, ExprKind::Record { .. }));

        let StmtKind::Expr(Expr {
            kind: ExprKind::Match(match_expr),
            ..
        }) = &function.body.statements[3].kind
        else {
            panic!("expected match expression statement");
        };
        assert_eq!(match_expr.arms.len(), 2);
        assert!(matches!(match_expr.arms[1].pattern, Pattern::Wildcard));
    }

    #[test]
    fn parser_recovers_after_bad_item() {
        let parsed = parse_source(source_id(), "bogus @@@\nfn next() {}");

        assert!(!parsed.diagnostics.is_empty());
        assert_eq!(parsed.items.len(), 1);
        assert!(matches!(parsed.items[0].kind, ItemKind::Function(_)));
    }

    #[test]
    fn parses_literal_return() {
        let parsed = parse_source(source_id(), "fn answer() { return 42; }");

        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let ItemKind::Function(function) = &parsed.items[0].kind else {
            panic!("expected function item");
        };
        let StmtKind::Return(Some(value)) = &function.body.statements[0].kind else {
            panic!("expected return value");
        };
        assert_eq!(value.kind, ExprKind::Literal(Literal::Int("42".into())));
    }

    #[test]
    fn parses_type_hint_metadata_and_rejects_generics() {
        let parsed = parse_source(
            source_id(),
            r#"
fn level_up(player: game.Player, amount: int) -> Result {
    let next: int = player.level + amount;
    let mapper = |reward: Reward| reward.count;
    return next;
}

struct Reward {
    item_id: string,
    count: int,
}
"#,
        );

        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let ItemKind::Function(function) = &parsed.items[0].kind else {
            panic!("expected function item");
        };
        assert_eq!(
            function.params[0]
                .type_hint
                .as_ref()
                .expect("player type hint")
                .path,
            ["game", "Player"]
        );
        assert_eq!(
            function.params[1]
                .type_hint
                .as_ref()
                .expect("amount type hint")
                .path,
            ["int"]
        );
        assert_eq!(
            function
                .return_type
                .as_ref()
                .expect("function return type hint")
                .path,
            ["Result"]
        );

        let StmtKind::Let {
            type_hint: Some(next_hint),
            ..
        } = &function.body.statements[0].kind
        else {
            panic!("expected typed let");
        };
        assert_eq!(next_hint.path, ["int"]);

        let StmtKind::Let {
            value: Some(lambda),
            ..
        } = &function.body.statements[1].kind
        else {
            panic!("expected lambda let");
        };
        let ExprKind::Lambda { params, .. } = &lambda.kind else {
            panic!("expected lambda");
        };
        assert_eq!(
            params[0]
                .type_hint
                .as_ref()
                .expect("lambda param type hint")
                .path,
            ["Reward"]
        );

        let ItemKind::Struct(record) = &parsed.items[1].kind else {
            panic!("expected struct item");
        };
        assert_eq!(
            record.fields[0]
                .type_hint
                .as_ref()
                .expect("item_id field type hint")
                .path,
            ["string"]
        );
        assert_eq!(
            record.fields[1]
                .type_hint
                .as_ref()
                .expect("count field type hint")
                .path,
            ["int"]
        );

        let generic = parse_source(source_id(), "fn bad(xs: Array<int>) { return xs; }");
        assert!(
            generic.diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_deref() == Some("syntax::generic_type_hint")
            })
        );
    }

    #[test]
    fn snapshots_core_m1_syntax_shape() {
        let parsed = parse_source(
            source_id(),
            r#"
use game.player.Player;

#[event("monster.kill")]
pub fn on_kill(ctx, player, monster) {
    let rewards = ctx.config.kill_rewards.filter(|r| r.monster_id == monster.id);
    player.exp += monster.exp;
    if player.exp >= ctx.config.exp_to_next_level(player.level) {
        player.level += 1;
    }
    for reward in rewards {
        player.inventory.add(reward.item_id, reward.count);
    }
    match player.quest_progress {
        QuestProgress.Active { quest_id, count } => {
            player.quest_progress = QuestProgress.Active { quest_id, count: count + 1 };
        },
        _ => {},
    }
}

struct KillReward { item_id, count }
enum QuestProgress { None, Active { quest_id, count } }
trait Damageable { fn damage(self, amount); }
"#,
        );

        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(
            snapshot_file(&parsed),
            r#"use game.player.Player
pub fn on_kill(ctx, player, monster)
  let rewards = call
  expr assign
  expr if
    expr assign
  for reward in path
    expr call
  expr match
    arm record_variant => block
    arm _ => block
struct KillReward(item_id, count)
enum QuestProgress(None, Active)
trait Damageable(damage)
"#
        );
    }

    #[test]
    fn malformed_body_diagnostics_keep_source_spans() {
        let parsed = parse_source(
            source_id(),
            r#"
fn bad(player) {
    let = ;
    if player.level > {
        return;
    }
}
fn next() {}
"#,
        );

        assert!(!parsed.diagnostics.is_empty());
        assert!(
            parsed
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.span.is_some())
        );
        assert_eq!(parsed.items.len(), 2);
        assert!(matches!(parsed.items[1].kind, ItemKind::Function(_)));
    }

    fn snapshot_file(file: &SourceFile) -> String {
        let mut out = String::new();
        for item in &file.items {
            match &item.kind {
                ItemKind::Use(use_item) => {
                    writeln!(out, "use {}", use_item.path.join("."))
                        .expect("write syntax snapshot");
                }
                ItemKind::Function(function) => {
                    let visibility = if item.visibility == Visibility::Public {
                        "pub "
                    } else {
                        ""
                    };
                    writeln!(
                        out,
                        "{visibility}fn {}({})",
                        function.name,
                        param_names(&function.params).join(", ")
                    )
                    .expect("write syntax snapshot");
                    snapshot_block(&mut out, &function.body, 1);
                }
                ItemKind::Struct(record) => {
                    writeln!(
                        out,
                        "struct {}({})",
                        record.name,
                        struct_field_names(&record.fields).join(", ")
                    )
                    .expect("write syntax snapshot");
                }
                ItemKind::Enum(enumeration) => {
                    writeln!(
                        out,
                        "enum {}({})",
                        enumeration.name,
                        enumeration.variants.join(", ")
                    )
                    .expect("write syntax snapshot");
                }
                ItemKind::Trait(trait_item) => {
                    writeln!(
                        out,
                        "trait {}({})",
                        trait_item.name,
                        trait_method_names(&trait_item.methods).join(", ")
                    )
                    .expect("write syntax snapshot");
                }
            }
        }
        out
    }

    fn snapshot_block(out: &mut String, block: &Block, indent: usize) {
        for stmt in &block.statements {
            snapshot_stmt(out, stmt, indent);
        }
    }

    fn snapshot_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
        let pad = "  ".repeat(indent);
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                let value = value.as_ref().map_or("<none>", expr_kind_name);
                writeln!(out, "{pad}let {name} = {value}").expect("write syntax snapshot");
            }
            StmtKind::Return(value) => {
                let value = value.as_ref().map_or("<none>", expr_kind_name);
                writeln!(out, "{pad}return {value}").expect("write syntax snapshot");
            }
            StmtKind::Break => writeln!(out, "{pad}break").expect("write syntax snapshot"),
            StmtKind::Continue => writeln!(out, "{pad}continue").expect("write syntax snapshot"),
            StmtKind::For {
                binding,
                iterable,
                body,
            } => {
                writeln!(out, "{pad}for {binding} in {}", expr_kind_name(iterable))
                    .expect("write syntax snapshot");
                snapshot_block(out, body, indent + 1);
            }
            StmtKind::Expr(expr) => snapshot_expr_stmt(out, expr, indent),
            StmtKind::Block(block) => {
                writeln!(out, "{pad}block").expect("write syntax snapshot");
                snapshot_block(out, block, indent + 1);
            }
        }
    }

    fn snapshot_expr_stmt(out: &mut String, expr: &Expr, indent: usize) {
        let pad = "  ".repeat(indent);
        writeln!(out, "{pad}expr {}", expr_kind_name(expr)).expect("write syntax snapshot");
        match &expr.kind {
            ExprKind::If(if_expr) => snapshot_block(out, &if_expr.then_branch, indent + 1),
            ExprKind::Match(match_expr) => {
                for arm in &match_expr.arms {
                    writeln!(
                        out,
                        "{pad}  arm {} => {}",
                        pattern_kind_name(&arm.pattern),
                        expr_kind_name(&arm.body)
                    )
                    .expect("write syntax snapshot");
                }
            }
            _ => {}
        }
    }

    fn expr_kind_name(expr: &Expr) -> &'static str {
        match expr.kind {
            ExprKind::Literal(_) => "literal",
            ExprKind::Path(_) => "path",
            ExprKind::SelfValue => "self",
            ExprKind::Unary { .. } => "unary",
            ExprKind::Binary { .. } => "binary",
            ExprKind::Assign { .. } => "assign",
            ExprKind::Field { .. } => "field",
            ExprKind::Call { .. } => "call",
            ExprKind::Index { .. } => "index",
            ExprKind::Try(_) => "try",
            ExprKind::Array(_) => "array",
            ExprKind::Map(_) => "map",
            ExprKind::Record { .. } => "record",
            ExprKind::Lambda { .. } => "lambda",
            ExprKind::If(_) => "if",
            ExprKind::Match(_) => "match",
            ExprKind::Block(_) => "block",
            ExprKind::Error => "error",
        }
    }

    fn pattern_kind_name(pattern: &Pattern) -> &'static str {
        match pattern {
            Pattern::Wildcard => "_",
            Pattern::Literal(_) => "literal",
            Pattern::Binding(_) => "binding",
            Pattern::Path(_) => "path",
            Pattern::TupleVariant { .. } => "tuple_variant",
            Pattern::RecordVariant { .. } => "record_variant",
        }
    }
}
