use super::*;

impl Parser {
    pub(super) fn parse_path(&mut self) -> Vec<String> {
        let mut parts = Vec::new();
        let Some(first) = self.eat_ident() else {
            return parts;
        };
        parts.push(first);

        while self.eat_symbol(Symbol::ColonColon).is_some() {
            if let Some(part) = self.eat_ident() {
                parts.push(part);
            } else {
                self.error_here("expected path segment");
                break;
            }
        }
        parts
    }

    pub(super) fn parse_static_path(&mut self) -> Vec<String> {
        let parts = self.parse_path();
        if self.check_symbol(Symbol::Dot) {
            self.error_here("use `::` for module/type paths; `.` is value access");
        }
        parts
    }

    pub(super) fn looks_like_map_literal(&self) -> bool {
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

    pub(super) fn skip_member_tail(&mut self) {
        let mut depth = 0_u32;
        while !self.at_eof() {
            if depth == 0
                && (self.check_symbol(Symbol::Comma)
                    || self.check_symbol(Symbol::Semicolon)
                    || self.check_symbol(Symbol::RBrace)
                    || self.check_member_start())
            {
                break;
            }
            self.bump_depth(&mut depth);
            self.advance();
        }
    }

    pub(super) fn check_member_start(&self) -> bool {
        matches!(self.current().kind, TokenKind::Ident(_))
            || (self.check_symbol(Symbol::Hash) && self.check_next_symbol(Symbol::LBracket))
    }

    pub(super) fn parse_type_annotation(&mut self) -> Option<TypeHint> {
        self.eat_symbol(Symbol::Colon)?;
        self.parse_type_hint()
    }

    pub(super) fn parse_optional_return_type(&mut self) -> Option<TypeHint> {
        if self.eat_symbol(Symbol::Arrow).is_some() {
            return self.parse_type_hint();
        }
        None
    }

    pub(super) fn parse_type_hint(&mut self) -> Option<TypeHint> {
        let start = self.current().span;
        let Some(first) = self.eat_type_hint_segment() else {
            self.error_here("expected type hint");
            return None;
        };
        let mut path = vec![first];

        while self.eat_symbol(Symbol::ColonColon).is_some() {
            if let Some(segment) = self.eat_type_hint_segment() {
                path.push(segment);
            } else {
                self.error_here("expected type path segment");
                break;
            }
        }
        if self.check_symbol(Symbol::Dot) {
            self.error_here("use `::` for module/type paths; `.` is value access");
        }

        let args = if self.check_symbol(Symbol::Less) {
            if type_argument_contract(&path).is_some() {
                self.parse_allowed_type_arguments(&path)
            } else {
                let generic_span = self.current().span;
                self.diagnostics.push(
                    Diagnostic::error(
                        "only builtin container, Option, and Result type hints support type arguments",
                    )
                    .with_code("syntax::generic_type_hint")
                    .with_span(generic_span)
                    .with_label(
                        generic_span,
                        "use a builtin parameterized type hint or remove these type arguments",
                    ),
                );
                self.skip_generic_type_arguments();
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Some(TypeHint {
            path,
            args,
            span: self.join_span(start, self.previous_span()),
        })
    }

    fn parse_allowed_type_arguments(&mut self, path: &[String]) -> Vec<TypeHint> {
        let Some(open) = self.eat_symbol(Symbol::Less) else {
            return Vec::new();
        };
        let open_span = open.span;
        let Some(contract) = type_argument_contract(path) else {
            return Vec::new();
        };
        let expected = contract.arity();
        let mut args = Vec::new();
        if self.check_symbol(Symbol::Greater) {
            self.error_here("expected type argument");
        }
        while !self.at_eof() && !self.check_symbol(Symbol::Greater) {
            if let Some(arg) = self.parse_type_hint() {
                args.push(arg);
            } else {
                self.skip_member_tail();
                break;
            }
            if self.eat_symbol(Symbol::Comma).is_none() {
                break;
            }
            if self.check_symbol(Symbol::Greater) {
                self.error_here("expected type argument after `,`");
                break;
            }
        }
        if self.eat_symbol(Symbol::Greater).is_none() {
            self.diagnostics.push(
                Diagnostic::error("unterminated type argument list")
                    .with_code("syntax::type_arguments")
                    .with_span(open_span)
                    .with_label(open_span, "type arguments start here"),
            );
        }
        if args.len() != expected {
            let span = self.join_span(open_span, self.previous_span());
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "`{}` expects {expected} type argument{}",
                    path.join("::"),
                    if expected == 1 { "" } else { "s" }
                ))
                .with_code("syntax::type_argument_arity")
                .with_span(span)
                .with_label(span, "wrong number of type arguments"),
            );
        } else if matches!(contract, TypeArgumentContract::StringKeyedMap)
            && !is_string_type_hint(&args[0])
        {
            let span = args[0].span;
            self.diagnostics.push(
                Diagnostic::error("`Map` type hints currently require `String` keys")
                    .with_code("syntax::map_key_type_argument")
                    .with_span(span)
                    .with_label(span, "use `String` as the first `Map` type argument"),
            );
        } else if matches!(contract, TypeArgumentContract::KeyedSet)
            && !is_set_key_type_hint(&args[0])
        {
            let span = args[0].span;
            self.diagnostics.push(
                Diagnostic::error("`Set` type hints require a set-keyable element type")
                    .with_code("syntax::set_element_type_argument")
                    .with_span(span)
                    .with_label(
                        span,
                        "use bool, i64, f64, String, or an unparameterized Set",
                    ),
            );
        }
        args
    }

    pub(super) fn eat_type_hint_segment(&mut self) -> Option<String> {
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

    pub(super) fn skip_generic_type_arguments(&mut self) {
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

    pub(super) fn is_type_hint_boundary(&self) -> bool {
        self.check_symbol(Symbol::Equal)
            || self.check_symbol(Symbol::Comma)
            || self.check_symbol(Symbol::RParen)
            || self.check_symbol(Symbol::RBrace)
            || self.check_symbol(Symbol::LBrace)
            || self.check_symbol(Symbol::Pipe)
            || self.check_symbol(Symbol::Semicolon)
            || self.at_eof()
    }
}

#[derive(Clone, Copy)]
enum TypeArgumentContract {
    FixedArity(usize),
    StringKeyedMap,
    KeyedSet,
}

impl TypeArgumentContract {
    fn arity(self) -> usize {
        match self {
            Self::FixedArity(arity) => arity,
            Self::StringKeyedMap => 2,
            Self::KeyedSet => 1,
        }
    }
}

fn type_argument_contract(path: &[String]) -> Option<TypeArgumentContract> {
    match path {
        [name] if name == "Array" => Some(TypeArgumentContract::FixedArity(1)),
        [name] if name == "Set" => Some(TypeArgumentContract::KeyedSet),
        [name] if name == "Map" => Some(TypeArgumentContract::StringKeyedMap),
        [name] if name == "Iterator" => Some(TypeArgumentContract::FixedArity(1)),
        [name] if name == "Option" => Some(TypeArgumentContract::FixedArity(1)),
        [name] if name == "Result" => Some(TypeArgumentContract::FixedArity(2)),
        _ => None,
    }
}

fn is_string_type_hint(hint: &TypeHint) -> bool {
    hint.path == ["String"] && hint.args.is_empty()
}

fn is_set_key_type_hint(hint: &TypeHint) -> bool {
    matches!(hint.path.as_slice(), [name] if matches!(
        name.as_str(),
        "null" | "bool" | "i64" | "f64" | "String"
    )) && hint.args.is_empty()
}
