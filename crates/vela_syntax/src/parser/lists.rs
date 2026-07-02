use super::*;

impl Parser {
    pub(super) fn parse_argument_list(&mut self) -> Vec<Argument> {
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

    pub(super) fn eat_parameter_name_with_span(&mut self) -> Option<(String, Span)> {
        match self.current().kind.clone() {
            TokenKind::Ident(name) => {
                let span = self.advance().span;
                Some((name, span))
            }
            TokenKind::Keyword(Keyword::SelfValue) => {
                let span = self.advance().span;
                Some(("self".to_owned(), span))
            }
            _ => None,
        }
    }
}
