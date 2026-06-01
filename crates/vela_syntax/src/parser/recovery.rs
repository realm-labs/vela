use super::*;

impl Parser {
    pub(super) fn skip_block_tokens(&mut self) {
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected block");
            return;
        }
        self.skip_balanced_until(Symbol::RBrace);
    }

    pub(super) fn skip_balanced_until(&mut self, close: Symbol) {
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

    pub(super) fn bump_depth(&self, depth: &mut u32) {
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

    pub(super) fn recover_until(&mut self, symbols: &[Symbol]) {
        while !self.at_eof() && !symbols.iter().any(|symbol| self.check_symbol(*symbol)) {
            self.advance();
        }
    }

    pub(super) fn recover_to_next_item(&mut self) {
        while !self.at_eof() {
            if self.check_keyword(Keyword::Pub)
                || self.check_keyword(Keyword::Use)
                || self.check_keyword(Keyword::Fn)
                || self.check_keyword(Keyword::Struct)
                || self.check_keyword(Keyword::Enum)
                || self.check_keyword(Keyword::Trait)
                || self.check_keyword(Keyword::Impl)
            {
                return;
            }
            self.advance();
        }
    }

    pub(super) fn is_statement_boundary(&self) -> bool {
        self.check_symbol(Symbol::Semicolon)
            || self.check_symbol(Symbol::RBrace)
            || self.check_symbol(Symbol::Comma)
            || self.at_eof()
    }

    pub(super) fn eat_assign_op(&mut self) -> Option<AssignOp> {
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

    pub(super) fn expect_ident(&mut self, message: &str) -> Option<String> {
        let ident = self.eat_ident();
        if ident.is_none() {
            self.error_here(message);
        }
        ident
    }

    pub(super) fn expect_ident_with_span(&mut self, message: &str) -> Option<(String, Span)> {
        let ident = self.eat_ident_with_span();
        if ident.is_none() {
            self.error_here(message);
        }
        ident
    }

    pub(super) fn eat_ident(&mut self) -> Option<String> {
        self.eat_ident_with_span().map(|(ident, _)| ident)
    }

    pub(super) fn eat_ident_with_span(&mut self) -> Option<(String, Span)> {
        let ident = match &self.current().kind {
            TokenKind::Ident(ident) => Some(ident.clone()),
            _ => None,
        }?;
        let span = self.advance().span;
        Some((ident, span))
    }

    pub(super) fn check_ident(&self) -> bool {
        matches!(self.current().kind, TokenKind::Ident(_))
    }

    pub(super) fn eat_keyword(&mut self, keyword: Keyword) -> Option<Token> {
        if self.check_keyword(keyword) {
            Some(self.advance())
        } else {
            None
        }
    }

    pub(super) fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(current) if current == keyword)
    }

    pub(super) fn eat_symbol(&mut self, symbol: Symbol) -> Option<Token> {
        if self.check_symbol(symbol) {
            Some(self.advance())
        } else {
            None
        }
    }

    pub(super) fn check_symbol(&self, symbol: Symbol) -> bool {
        matches!(self.current().kind, TokenKind::Symbol(current) if current == symbol)
    }

    pub(super) fn check_next_symbol(&self, symbol: Symbol) -> bool {
        matches!(
            self.tokens.get(self.pos.saturating_add(1)).map(|token| &token.kind),
            Some(TokenKind::Symbol(current)) if *current == symbol
        )
    }

    pub(super) fn current_symbol(&self) -> Option<Symbol> {
        match self.current().kind {
            TokenKind::Symbol(symbol) => Some(symbol),
            _ => None,
        }
    }

    pub(super) fn current(&self) -> &Token {
        let index = self.pos.min(self.tokens.len().saturating_sub(1));
        &self.tokens[index]
    }

    pub(super) fn previous_span(&self) -> Span {
        self.pos
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map_or_else(|| self.current().span, |token| token.span)
    }

    pub(super) fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if !self.at_eof() {
            self.pos = self.pos.saturating_add(1);
        }
        token
    }

    pub(super) fn at_eof(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    pub(super) fn error_here(&mut self, message: impl Into<String>) {
        self.diagnostics.push(
            Diagnostic::error(message)
                .with_code("E_PARSE")
                .with_span(self.current().span),
        );
    }

    pub(super) fn join_span(&self, start: Span, end: Span) -> Span {
        Span::new(start.source, start.start, end.end)
    }
}
