use super::*;

impl Parser {
    pub(super) fn skip_attributes(&mut self) -> Option<u32> {
        let mut first_start = None;
        while self.check_symbol(Symbol::Hash) && self.check_next_symbol(Symbol::LBracket) {
            let start = self.advance().span.start;
            first_start.get_or_insert(start);
            self.advance();
            self.parse_static_path();
            self.skip_attribute_value();
            self.skip_balanced_until(Symbol::RBracket);
        }
        first_start
    }

    pub(super) fn skip_attribute_value(&mut self) -> Option<()> {
        self.eat_symbol(Symbol::LParen)?;
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
                }
                TokenKind::Symbol(Symbol::RParen | Symbol::RBracket | Symbol::RBrace) => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        Some(())
    }
}
