use super::*;

impl Parser {
    pub(super) fn parse_expression(&mut self) -> Expr {
        self.parse_assignment()
    }

    pub(super) fn parse_expression_before_block(&mut self) -> Expr {
        let previous = self.allow_record_literals;
        self.allow_record_literals = false;
        let expr = self.parse_expression();
        self.allow_record_literals = previous;
        expr
    }

    pub(super) fn parse_assignment(&mut self) -> Expr {
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

    pub(super) fn parse_logical_or(&mut self) -> Expr {
        self.parse_binary_left_assoc(Self::parse_logical_and, &[(Symbol::OrOr, BinaryOp::Or)])
    }

    pub(super) fn parse_logical_and(&mut self) -> Expr {
        self.parse_binary_left_assoc(Self::parse_equality, &[(Symbol::AndAnd, BinaryOp::And)])
    }

    pub(super) fn parse_equality(&mut self) -> Expr {
        self.parse_binary_left_assoc(
            Self::parse_comparison,
            &[
                (Symbol::EqualEqual, BinaryOp::Equal),
                (Symbol::BangEqual, BinaryOp::NotEqual),
            ],
        )
    }

    pub(super) fn parse_comparison(&mut self) -> Expr {
        self.parse_binary_left_assoc(
            Self::parse_range,
            &[
                (Symbol::Less, BinaryOp::Less),
                (Symbol::LessEqual, BinaryOp::LessEqual),
                (Symbol::Greater, BinaryOp::Greater),
                (Symbol::GreaterEqual, BinaryOp::GreaterEqual),
            ],
        )
    }

    pub(super) fn parse_range(&mut self) -> Expr {
        let left = self.parse_additive();
        let op = if self.eat_symbol(Symbol::DotDotEqual).is_some() {
            BinaryOp::RangeInclusive
        } else if self.eat_symbol(Symbol::DotDot).is_some() {
            BinaryOp::Range
        } else {
            return left;
        };
        let right = self.parse_additive();
        let span = self.join_span(left.span, right.span);
        Expr {
            kind: ExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        }
    }

    pub(super) fn parse_additive(&mut self) -> Expr {
        self.parse_binary_left_assoc(
            Self::parse_multiplicative,
            &[
                (Symbol::Plus, BinaryOp::Add),
                (Symbol::Minus, BinaryOp::Sub),
            ],
        )
    }

    pub(super) fn parse_multiplicative(&mut self) -> Expr {
        self.parse_binary_left_assoc(
            Self::parse_unary,
            &[
                (Symbol::Star, BinaryOp::Mul),
                (Symbol::Slash, BinaryOp::Div),
                (Symbol::Percent, BinaryOp::Rem),
            ],
        )
    }

    pub(super) fn parse_binary_left_assoc(
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

    pub(super) fn parse_unary(&mut self) -> Expr {
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

    pub(super) fn parse_postfix(&mut self) -> Expr {
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

    pub(super) fn parse_primary(&mut self) -> Expr {
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
                self.literal_expr(Literal::Integer(value), span)
            }
            TokenKind::Float(value) => {
                self.advance();
                self.literal_expr(Literal::Float(value), span)
            }
            TokenKind::String(value) => {
                self.advance();
                self.literal_expr(Literal::String(value), span)
            }
            TokenKind::Bytes(value) => {
                self.advance();
                self.literal_expr(Literal::Bytes(value), span)
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
            TokenKind::Symbol(Symbol::Pipe | Symbol::OrOr) => self.parse_lambda_expression(),
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

    pub(super) fn literal_expr(&self, literal: Literal, span: Span) -> Expr {
        Expr {
            kind: ExprKind::Literal(literal),
            span,
        }
    }

    pub(super) fn parse_grouped_expression(&mut self) -> Expr {
        self.eat_symbol(Symbol::LParen);
        let expr = self.parse_expression();
        if self.eat_symbol(Symbol::RParen).is_none() {
            self.error_here("expected `)`");
        }
        expr
    }

    pub(super) fn parse_array_expression(&mut self) -> Expr {
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

    pub(super) fn parse_map_expression(&mut self) -> Expr {
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

    pub(super) fn parse_map_key(&mut self) -> Expr {
        match self.current().kind.clone() {
            TokenKind::Ident(_) => self.parse_path_or_record(),
            TokenKind::String(value) => {
                let span = self.advance().span;
                self.literal_expr(Literal::String(value), span)
            }
            TokenKind::Bytes(value) => {
                let span = self.advance().span;
                self.literal_expr(Literal::Bytes(value), span)
            }
            TokenKind::Int(value) => {
                let span = self.advance().span;
                self.literal_expr(Literal::Integer(value), span)
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

    pub(super) fn parse_path_or_record(&mut self) -> Expr {
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

    pub(super) fn parse_record_fields(&mut self) -> Vec<RecordField> {
        self.eat_symbol(Symbol::LBrace);
        let mut fields = Vec::new();
        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            let span = self.current().span;
            let name = self
                .expect_ident("expected record field")
                .unwrap_or_default();
            let value = if self.eat_symbol(Symbol::Colon).is_some() {
                Some(self.parse_expression())
            } else {
                None
            };
            fields.push(RecordField { name, span, value });
            if self.eat_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        if self.eat_symbol(Symbol::RBrace).is_none() {
            self.error_here("expected `}`");
        }
        fields
    }

    pub(super) fn parse_lambda_expression(&mut self) -> Expr {
        if let Some(start) = self.eat_symbol(Symbol::OrOr).map(|token| token.span) {
            let body = self.parse_lambda_body();
            return Expr {
                span: self.join_span(start, body.span),
                kind: ExprKind::Lambda {
                    params: Vec::new(),
                    body: Box::new(body),
                },
            };
        }

        let start = self.eat_symbol(Symbol::Pipe).expect("checked").span;
        let mut params = Vec::new();
        while !self.at_eof() && !self.check_symbol(Symbol::Pipe) {
            if let Some((param, name_span)) = self.eat_parameter_name_with_span() {
                let type_hint = self.parse_type_annotation();
                let end = type_hint
                    .as_ref()
                    .map_or(name_span.end, |hint| hint.span.end);
                params.push(Param {
                    name: param,
                    span: Span::new(name_span.source, name_span.start, end),
                    type_hint,
                    default_value: None,
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
        let body = self.parse_lambda_body();
        Expr {
            span: self.join_span(start, body.span),
            kind: ExprKind::Lambda {
                params,
                body: Box::new(body),
            },
        }
    }

    fn parse_lambda_body(&mut self) -> Expr {
        if self.check_symbol(Symbol::LBrace) {
            let block = self.parse_block().unwrap_or(Block {
                statements: Vec::new(),
                span: self.current().span,
            });
            return Expr {
                span: block.span,
                kind: ExprKind::Block(block),
            };
        }
        self.parse_expression()
    }

    pub(super) fn parse_if_expression(&mut self) -> Expr {
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

    pub(super) fn parse_match_expression(&mut self) -> Expr {
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

    pub(super) fn parse_pattern(&mut self) -> Pattern {
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
            TokenKind::Bytes(value) => {
                self.advance();
                Pattern::Literal(Literal::Bytes(value))
            }
            TokenKind::Int(value) => {
                self.advance();
                Pattern::Literal(Literal::Integer(value))
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

    pub(super) fn parse_path_pattern(&mut self) -> Pattern {
        let path = self.parse_static_path();
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
}
