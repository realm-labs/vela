use super::*;

impl Parser {
    pub(super) fn parse_attributes(&mut self) -> Vec<Attribute> {
        let mut attrs = Vec::new();
        while self.check_symbol(Symbol::Hash) && self.check_next_symbol(Symbol::LBracket) {
            let start = self.advance().span.start;
            self.advance();
            let path = self.parse_static_path();
            let value = self.parse_attribute_value();
            self.skip_balanced_until(Symbol::RBracket);
            let end = self.previous_span().end;
            attrs.push(Attribute {
                path,
                value,
                span: Span::new(self.current().span.source, start, end),
            });
        }
        attrs
    }

    pub(super) fn parse_attribute_value(&mut self) -> Option<String> {
        self.eat_symbol(Symbol::LParen)?;
        let mut tokens = Vec::new();
        let mut depth = 1_usize;
        while !self.at_eof() {
            if self.check_symbol(Symbol::RParen) && depth == 1 {
                self.advance();
                break;
            }

            let token = self.advance();
            match token.kind {
                TokenKind::Symbol(Symbol::LParen | Symbol::LBracket | Symbol::LBrace) => {
                    depth = depth.saturating_add(1);
                    tokens.push(token.kind);
                }
                TokenKind::Symbol(Symbol::RParen | Symbol::RBracket | Symbol::RBrace) => {
                    depth = depth.saturating_sub(1);
                    tokens.push(token.kind);
                    if depth == 0 {
                        break;
                    }
                }
                _ => tokens.push(token.kind),
            }
        }
        Some(normalize_attribute_value(&tokens))
    }
}
