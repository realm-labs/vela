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

    pub(super) fn parse_parameter_list(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        if self.eat_symbol(Symbol::LParen).is_none() {
            self.error_here("expected parameter list");
            return params;
        }

        while !self.at_eof() && !self.check_symbol(Symbol::RParen) {
            if let Some((param, name_span)) = self.eat_parameter_name_with_span() {
                let type_hint = self.parse_type_annotation();
                let default_value = if self.eat_symbol(Symbol::Equal).is_some() {
                    Some(self.parse_expression())
                } else {
                    None
                };
                let end = default_value.as_ref().map_or_else(
                    || {
                        type_hint
                            .as_ref()
                            .map_or(name_span.end, |hint| hint.span.end)
                    },
                    |value| value.span.end,
                );
                params.push(Param {
                    name: param,
                    span: Span::new(name_span.source, name_span.start, end),
                    type_hint,
                    default_value,
                });
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

    pub(super) fn parse_struct_fields_in_braces(&mut self) -> Vec<StructField> {
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected `{`");
            return Vec::new();
        }
        self.parse_struct_fields_until_rbrace()
    }

    pub(super) fn parse_struct_fields_until_rbrace(&mut self) -> Vec<StructField> {
        let mut fields = Vec::new();
        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            let attrs = self.parse_attributes();
            if let Some((name, name_span)) = self.eat_ident_with_span() {
                let type_hint = self.parse_type_annotation();
                let default_value = if self.eat_symbol(Symbol::Equal).is_some() {
                    Some(self.parse_expression())
                } else {
                    None
                };
                let span_start = attrs
                    .first()
                    .map_or(name_span.start, |attr| attr.span.start);
                let end = default_value.as_ref().map_or_else(
                    || {
                        type_hint
                            .as_ref()
                            .map_or(name_span.end, |hint| hint.span.end)
                    },
                    |value| value.span.end,
                );
                fields.push(StructField {
                    attrs,
                    name,
                    span: Span::new(name_span.source, span_start, end),
                    type_hint,
                    default_value,
                });
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

    pub(super) fn parse_enum_variants_in_braces(&mut self) -> Vec<EnumVariant> {
        let mut variants = Vec::new();
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected `{`");
            return variants;
        }

        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            let attrs = self.parse_attributes();
            if let Some((name, name_span)) = self.eat_ident_with_span() {
                let fields = if self.check_symbol(Symbol::LParen) {
                    EnumVariantFields::Tuple(self.parse_parameter_list())
                } else if self.eat_symbol(Symbol::LBrace).is_some() {
                    EnumVariantFields::Record(self.parse_struct_fields_until_rbrace())
                } else {
                    EnumVariantFields::Unit
                };
                let span_start = attrs
                    .first()
                    .map_or(name_span.start, |attr| attr.span.start);
                variants.push(EnumVariant {
                    attrs,
                    name,
                    span: Span::new(name_span.source, span_start, self.previous_span().end),
                    fields,
                });
                self.skip_member_tail();
            } else {
                self.advance();
            }
            self.eat_symbol(Symbol::Comma);
            self.eat_symbol(Symbol::Semicolon);
        }

        self.eat_symbol(Symbol::RBrace);
        variants
    }
}
