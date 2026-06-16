use super::*;
use crate::ast::ImplKind;

impl Parser {
    pub(super) fn parse_item(&mut self) -> Option<Item> {
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
        } else if self.eat_keyword(Keyword::Const).is_some() {
            self.parse_const_item().map(ItemKind::Const)
        } else if self.eat_keyword(Keyword::Global).is_some() {
            self.parse_global_item().map(ItemKind::Global)
        } else if self.eat_keyword(Keyword::Fn).is_some() {
            self.parse_function_item().map(ItemKind::Function)
        } else if self.eat_keyword(Keyword::Struct).is_some() {
            self.parse_struct_item().map(ItemKind::Struct)
        } else if self.eat_keyword(Keyword::Enum).is_some() {
            self.parse_enum_item().map(ItemKind::Enum)
        } else if self.eat_keyword(Keyword::Trait).is_some() {
            self.parse_trait_item().map(ItemKind::Trait)
        } else if self.eat_keyword(Keyword::Impl).is_some() {
            self.parse_impl_item().map(ItemKind::Impl)
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

    pub(super) fn parse_use_item(&mut self) -> Option<UseItem> {
        let path = self.parse_static_path();
        if path.is_empty() {
            self.error_here("expected use path");
            return None;
        }
        let alias = if self.eat_keyword(Keyword::As).is_some() {
            self.expect_ident("expected import alias")
        } else {
            None
        };
        self.eat_symbol(Symbol::Semicolon);
        Some(UseItem { path, alias })
    }

    pub(super) fn parse_const_item(&mut self) -> Option<ConstItem> {
        let name = self.expect_ident("expected const name")?;
        let type_hint = self.parse_type_annotation();
        if self.eat_symbol(Symbol::Equal).is_none() {
            self.error_here("expected `=` in const declaration");
        }
        let value = self.parse_expression();
        self.eat_symbol(Symbol::Semicolon);
        Some(ConstItem {
            name,
            type_hint,
            value,
        })
    }

    pub(super) fn parse_global_item(&mut self) -> Option<GlobalItem> {
        let name = self.expect_ident("expected global name")?;
        let type_hint = match self.parse_type_annotation() {
            Some(type_hint) => type_hint,
            None => {
                self.error_here("expected global type annotation");
                TypeHint {
                    path: Vec::new(),
                    args: Vec::new(),
                    span: self.previous_span(),
                }
            }
        };
        if self.eat_symbol(Symbol::Equal).is_some() {
            self.error_here(
                "global declarations are bound by the host and cannot initialize values",
            );
            let _ = self.parse_expression();
        }
        self.eat_symbol(Symbol::Semicolon);
        Some(GlobalItem { name, type_hint })
    }

    pub(super) fn parse_function_item(&mut self) -> Option<FunctionItem> {
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

    pub(super) fn parse_struct_item(&mut self) -> Option<StructItem> {
        let name = self.expect_ident("expected struct name")?;
        let fields = self.parse_struct_fields_in_braces();
        Some(StructItem { name, fields })
    }

    pub(super) fn parse_enum_item(&mut self) -> Option<crate::ast::EnumItem> {
        let name = self.expect_ident("expected enum name")?;
        let variants = self.parse_enum_variants_in_braces();
        Some(crate::ast::EnumItem { name, variants })
    }

    pub(super) fn parse_trait_item(&mut self) -> Option<TraitItem> {
        let name = self.expect_ident("expected trait name")?;
        let mut methods = Vec::new();
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected trait body");
            return Some(TraitItem { name, methods });
        }

        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            let attrs = self.parse_attributes();
            if let Some(fn_token) = self.eat_keyword(Keyword::Fn) {
                if let Some((method, name_span)) =
                    self.expect_ident_with_span("expected trait method name")
                {
                    let params = self.parse_parameter_list();
                    let return_type = self.parse_optional_return_type();
                    let default_body = if self.check_symbol(Symbol::LBrace) {
                        self.parse_block()
                    } else {
                        self.eat_symbol(Symbol::Semicolon);
                        None
                    };
                    let span_start = attrs
                        .first()
                        .map_or(fn_token.span.start, |attr| attr.span.start);
                    methods.push(TraitMethod {
                        attrs,
                        name: method,
                        span: Span::new(name_span.source, span_start, self.previous_span().end),
                        params,
                        return_type,
                        has_default: default_body.is_some(),
                        default_body,
                    });
                } else {
                    self.parse_parameter_list();
                    self.parse_optional_return_type();
                    if self.check_symbol(Symbol::LBrace) {
                        self.skip_block_tokens();
                    } else {
                        self.eat_symbol(Symbol::Semicolon);
                    }
                }
            } else {
                self.error_here("expected trait item");
                self.advance();
            }
        }

        self.eat_symbol(Symbol::RBrace);
        Some(TraitItem { name, methods })
    }

    pub(super) fn parse_impl_item(&mut self) -> Option<ImplItem> {
        let first_path = self.parse_static_path();
        if first_path.is_empty() {
            self.error_here("expected impl path");
        }
        let (kind, target_path) = if self.eat_keyword(Keyword::For).is_some() {
            let target_path = self.parse_static_path();
            (
                ImplKind::Trait {
                    trait_path: first_path,
                },
                target_path,
            )
        } else {
            (ImplKind::Inherent, first_path)
        };
        if target_path.is_empty() {
            self.error_here("expected impl target path");
        }

        let mut methods = Vec::new();
        if self.eat_symbol(Symbol::LBrace).is_none() {
            self.error_here("expected impl body");
            return Some(ImplItem {
                kind,
                target_path,
                methods,
            });
        }

        while !self.at_eof() && !self.check_symbol(Symbol::RBrace) {
            let attrs = self.parse_attributes();
            if let Some(fn_token) = self.eat_keyword(Keyword::Fn) {
                if let Some(function) = self.parse_function_item() {
                    let span_start = attrs
                        .first()
                        .map_or(fn_token.span.start, |attr| attr.span.start);
                    let span = Span::new(fn_token.span.source, span_start, function.body.span.end);
                    methods.push(ImplMethod {
                        attrs,
                        function,
                        span,
                    });
                }
            } else {
                self.error_here("expected impl method");
                self.advance();
            }
        }

        self.eat_symbol(Symbol::RBrace);
        Some(ImplItem {
            kind,
            target_path,
            methods,
        })
    }
}
