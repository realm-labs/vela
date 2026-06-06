use super::*;

impl Parser {
    pub(super) fn parse_block(&mut self) -> Option<Block> {
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

    pub(super) fn parse_statement(&mut self) -> Option<Stmt> {
        let attrs = self.parse_attributes();
        let start = attrs
            .first()
            .map(|attr| attr.span.start)
            .unwrap_or(self.current().span.start);

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
            attrs,
            kind,
            span: Span::new(self.current().span.source, start, end),
        })
    }

    pub(super) fn parse_let_statement(&mut self) -> StmtKind {
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

    pub(super) fn parse_for_statement(&mut self) -> StmtKind {
        let first_pattern = self.parse_pattern();
        let (index_pattern, pattern) = if self.eat_symbol(Symbol::Comma).is_some() {
            (Some(first_pattern), self.parse_pattern())
        } else {
            (None, first_pattern)
        };
        if self.eat_keyword(Keyword::In).is_none() {
            self.error_here("expected `in`");
        }
        let iterable = self.parse_expression_before_block();
        let body = self.parse_block().unwrap_or_else(|| Block {
            statements: Vec::new(),
            span: self.current().span,
        });
        StmtKind::For {
            index_pattern,
            pattern,
            iterable,
            body,
        }
    }
}
