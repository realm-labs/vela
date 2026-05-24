//! Lexer and early parser for Vela source files.

use vela_common::{Diagnostic, SourceId, Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Int(String),
    Float(String),
    String(String),
    Keyword(Keyword),
    Symbol(Symbol),
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Keyword {
    Use,
    Pub,
    Const,
    Let,
    Fn,
    Struct,
    Enum,
    Trait,
    Impl,
    For,
    If,
    Else,
    Match,
    Return,
    Break,
    Continue,
    True,
    False,
    Null,
    SelfValue,
    In,
    As,
}

impl Keyword {
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "use" => Some(Self::Use),
            "pub" => Some(Self::Pub),
            "const" => Some(Self::Const),
            "let" => Some(Self::Let),
            "fn" => Some(Self::Fn),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "trait" => Some(Self::Trait),
            "impl" => Some(Self::Impl),
            "for" => Some(Self::For),
            "if" => Some(Self::If),
            "else" => Some(Self::Else),
            "match" => Some(Self::Match),
            "return" => Some(Self::Return),
            "break" => Some(Self::Break),
            "continue" => Some(Self::Continue),
            "true" => Some(Self::True),
            "false" => Some(Self::False),
            "null" => Some(Self::Null),
            "self" => Some(Self::SelfValue),
            "in" => Some(Self::In),
            "as" => Some(Self::As),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Symbol {
    Hash,
    LBracket,
    RBracket,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Arrow,
    FatArrow,
    Equal,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    BangEqual,
    EqualEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Pipe,
    Question,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn lex(source: SourceId, text: &str) -> Lexed {
    Lexer::new(source, text).lex()
}

struct Lexer<'src> {
    source: SourceId,
    text: &'src str,
    offset: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'src> Lexer<'src> {
    fn new(source: SourceId, text: &'src str) -> Self {
        Self {
            source,
            text,
            offset: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn lex(mut self) -> Lexed {
        while let Some(ch) = self.peek_char() {
            match ch {
                ' ' | '\t' | '\r' | '\n' => {
                    self.bump_char();
                }
                '/' if self.peek_next_char() == Some('/') => self.skip_line_comment(),
                '/' if self.peek_next_char() == Some('*') => self.skip_block_comment(),
                '"' => self.lex_string(),
                '0'..='9' => self.lex_number(),
                '_' | 'a'..='z' | 'A'..='Z' => self.lex_ident_or_keyword(),
                _ => self.lex_symbol_or_error(),
            }
        }

        self.push_token(TokenKind::Eof, self.offset, self.offset);
        Lexed {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.text.get(self.offset..)?.chars().next()
    }

    fn peek_next_char(&self) -> Option<char> {
        let mut chars = self.text.get(self.offset..)?.chars();
        chars.next()?;
        chars.next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.offset = self.offset.saturating_add(ch.len_utf8());
        Some(ch)
    }

    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            self.bump_char();
            if ch == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) {
        let start = self.offset;
        self.bump_char();
        self.bump_char();
        let mut depth = 1_u32;

        while let Some(ch) = self.peek_char() {
            if ch == '/' && self.peek_next_char() == Some('*') {
                self.bump_char();
                self.bump_char();
                depth = depth.saturating_add(1);
            } else if ch == '*' && self.peek_next_char() == Some('/') {
                self.bump_char();
                self.bump_char();
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return;
                }
            } else {
                self.bump_char();
            }
        }

        self.diagnostics.push(
            Diagnostic::error("unterminated block comment")
                .with_code("E_LEX_BLOCK_COMMENT")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn lex_string(&mut self) {
        let start = self.offset;
        self.bump_char();
        let mut value = String::new();

        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.bump_char();
                    self.push_token(TokenKind::String(value), start, self.offset);
                    return;
                }
                '\\' => {
                    self.bump_char();
                    if let Some(escaped) = self.peek_char() {
                        self.bump_char();
                        let decoded = match escaped {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '0' => '\0',
                            '"' => '"',
                            '\\' => '\\',
                            '/' => '/',
                            other => other,
                        };
                        value.push(decoded);
                    }
                }
                '\n' => {
                    break;
                }
                other => {
                    self.bump_char();
                    value.push(other);
                }
            }
        }

        self.diagnostics.push(
            Diagnostic::error("unterminated string literal")
                .with_code("E_LEX_STRING")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn lex_number(&mut self) {
        let start = self.offset;
        self.consume_digits_or_underscores();
        let mut is_float = false;

        if self.peek_char() == Some('.')
            && self
                .peek_next_char()
                .is_some_and(|next| next.is_ascii_digit())
        {
            is_float = true;
            self.bump_char();
            self.consume_digits_or_underscores();
        }

        let text = self.slice(start, self.offset).to_owned();
        if is_float {
            self.push_token(TokenKind::Float(text), start, self.offset);
        } else {
            self.push_token(TokenKind::Int(text), start, self.offset);
        }
    }

    fn consume_digits_or_underscores(&mut self) {
        while self
            .peek_char()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_digit())
        {
            self.bump_char();
        }
    }

    fn lex_ident_or_keyword(&mut self) {
        let start = self.offset;
        while self
            .peek_char()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        {
            self.bump_char();
        }

        let text = self.slice(start, self.offset);
        if let Some(keyword) = Keyword::from_text(text) {
            self.push_token(TokenKind::Keyword(keyword), start, self.offset);
        } else {
            self.push_token(TokenKind::Ident(text.to_owned()), start, self.offset);
        }
    }

    fn lex_symbol_or_error(&mut self) {
        let start = self.offset;
        let Some(ch) = self.bump_char() else {
            return;
        };

        let symbol = match ch {
            '#' => Some(Symbol::Hash),
            '[' => Some(Symbol::LBracket),
            ']' => Some(Symbol::RBracket),
            '(' => Some(Symbol::LParen),
            ')' => Some(Symbol::RParen),
            '{' => Some(Symbol::LBrace),
            '}' => Some(Symbol::RBrace),
            ',' => Some(Symbol::Comma),
            '.' => Some(Symbol::Dot),
            ':' => Some(Symbol::Colon),
            ';' => Some(Symbol::Semicolon),
            '?' => Some(Symbol::Question),
            '|' if self.peek_char() == Some('|') => {
                self.bump_char();
                Some(Symbol::OrOr)
            }
            '|' => Some(Symbol::Pipe),
            '&' if self.peek_char() == Some('&') => {
                self.bump_char();
                Some(Symbol::AndAnd)
            }
            '+' if self.peek_char() == Some('=') => {
                self.bump_char();
                Some(Symbol::PlusEqual)
            }
            '+' => Some(Symbol::Plus),
            '-' if self.peek_char() == Some('>') => {
                self.bump_char();
                Some(Symbol::Arrow)
            }
            '-' if self.peek_char() == Some('=') => {
                self.bump_char();
                Some(Symbol::MinusEqual)
            }
            '-' => Some(Symbol::Minus),
            '*' if self.peek_char() == Some('=') => {
                self.bump_char();
                Some(Symbol::StarEqual)
            }
            '*' => Some(Symbol::Star),
            '/' if self.peek_char() == Some('=') => {
                self.bump_char();
                Some(Symbol::SlashEqual)
            }
            '/' => Some(Symbol::Slash),
            '%' if self.peek_char() == Some('=') => {
                self.bump_char();
                Some(Symbol::PercentEqual)
            }
            '%' => Some(Symbol::Percent),
            '!' if self.peek_char() == Some('=') => {
                self.bump_char();
                Some(Symbol::BangEqual)
            }
            '!' => Some(Symbol::Bang),
            '=' if self.peek_char() == Some('>') => {
                self.bump_char();
                Some(Symbol::FatArrow)
            }
            '=' if self.peek_char() == Some('=') => {
                self.bump_char();
                Some(Symbol::EqualEqual)
            }
            '=' => Some(Symbol::Equal),
            '<' if self.peek_char() == Some('=') => {
                self.bump_char();
                Some(Symbol::LessEqual)
            }
            '<' => Some(Symbol::Less),
            '>' if self.peek_char() == Some('=') => {
                self.bump_char();
                Some(Symbol::GreaterEqual)
            }
            '>' => Some(Symbol::Greater),
            _ => None,
        };

        if let Some(symbol) = symbol {
            self.push_token(TokenKind::Symbol(symbol), start, self.offset);
        } else {
            self.diagnostics.push(
                Diagnostic::error(format!("unexpected character `{ch}`"))
                    .with_code("E_LEX_CHAR")
                    .with_span(self.span(start, self.offset)),
            );
        }
    }

    fn push_token(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: self.span(start, end),
        });
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span::new(self.source, to_u32(start), to_u32(end))
    }

    fn slice(&self, start: usize, end: usize) -> &str {
        self.text.get(start..end).unwrap_or_default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    pub items: Vec<Item>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub attrs: Vec<Attribute>,
    pub visibility: Visibility,
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Visibility {
    Private,
    Public,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Use(UseItem),
    Function(FunctionItem),
    Struct(StructItem),
    Enum(EnumItem),
    Trait(TraitItem),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseItem {
    pub path: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionItem {
    pub name: String,
    pub params: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructItem {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumItem {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitItem {
    pub name: String,
    pub methods: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub path: Vec<String>,
    pub span: Span,
}

#[must_use]
pub fn parse_source(source: SourceId, text: &str) -> SourceFile {
    let lexed = lex(source, text);
    Parser::new(lexed.tokens, lexed.diagnostics).parse()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    fn new(tokens: Vec<Token>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics,
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
        self.skip_optional_return_type();
        self.expect_balanced_block();
        Some(FunctionItem { name, params })
    }

    fn parse_struct_item(&mut self) -> Option<StructItem> {
        let name = self.expect_ident("expected struct name")?;
        let fields = self.parse_named_members_in_braces();
        Some(StructItem { name, fields })
    }

    fn parse_enum_item(&mut self) -> Option<EnumItem> {
        let name = self.expect_ident("expected enum name")?;
        let variants = self.parse_named_members_in_braces();
        Some(EnumItem { name, variants })
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
                    methods.push(method);
                }
                self.parse_parameter_list();
                self.skip_optional_return_type();
                if self.check_symbol(Symbol::LBrace) {
                    self.expect_balanced_block();
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

    fn parse_parameter_list(&mut self) -> Vec<String> {
        let mut params = Vec::new();
        if self.eat_symbol(Symbol::LParen).is_none() {
            self.error_here("expected parameter list");
            return params;
        }

        while !self.at_eof() && !self.check_symbol(Symbol::RParen) {
            if let Some(param) = self.eat_ident() {
                params.push(param);
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

    fn skip_parameter_tail(&mut self) {
        let mut depth = 0_u32;
        while !self.at_eof() {
            if depth == 0 && (self.check_symbol(Symbol::Comma) || self.check_symbol(Symbol::RParen))
            {
                break;
            }
            self.adjust_depth();
            self.advance();
            if self.previous_is_closing() && depth > 0 {
                depth = depth.saturating_sub(1);
            } else if self.previous_is_opening() {
                depth = depth.saturating_add(1);
            }
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
            self.adjust_depth();
            self.advance();
            if self.previous_is_closing() && depth > 0 {
                depth = depth.saturating_sub(1);
            } else if self.previous_is_opening() {
                depth = depth.saturating_add(1);
            }
        }
    }

    fn skip_optional_return_type(&mut self) {
        if self.eat_symbol(Symbol::Arrow).is_some() {
            while !self.at_eof() && !self.check_symbol(Symbol::LBrace) {
                self.advance();
            }
        }
    }

    fn expect_balanced_block(&mut self) {
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

            match self.current_symbol() {
                Some(Symbol::LBrace | Symbol::LBracket | Symbol::LParen) => {
                    depth = depth.saturating_add(1);
                }
                Some(Symbol::RBrace | Symbol::RBracket | Symbol::RParen) if depth > 0 => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
            self.advance();
        }
        self.error_here("expected closing delimiter");
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

    fn adjust_depth(&self) {}

    fn previous_is_opening(&self) -> bool {
        matches!(
            self.previous_kind(),
            Some(TokenKind::Symbol(
                Symbol::LBrace | Symbol::LBracket | Symbol::LParen
            ))
        )
    }

    fn previous_is_closing(&self) -> bool {
        matches!(
            self.previous_kind(),
            Some(TokenKind::Symbol(
                Symbol::RBrace | Symbol::RBracket | Symbol::RParen
            ))
        )
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

    fn previous_kind(&self) -> Option<&TokenKind> {
        self.pos
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(|token| &token.kind)
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
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_id() -> SourceId {
        SourceId::new(1)
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
        assert_eq!(function.params, ["ctx", "player", "monster"]);
        assert_eq!(parsed.items[1].attrs[0].path, ["event"]);

        let ItemKind::Struct(record) = &parsed.items[2].kind else {
            panic!("expected struct item");
        };
        assert_eq!(record.fields, ["item_id", "count"]);

        let ItemKind::Enum(enumeration) = &parsed.items[3].kind else {
            panic!("expected enum item");
        };
        assert_eq!(enumeration.variants, ["None", "Active"]);

        let ItemKind::Trait(trait_item) = &parsed.items[4].kind else {
            panic!("expected trait item");
        };
        assert_eq!(trait_item.methods, ["damage"]);
    }

    #[test]
    fn parser_recovers_after_bad_item() {
        let parsed = parse_source(source_id(), "bogus @@@\nfn next() {}");

        assert!(!parsed.diagnostics.is_empty());
        assert_eq!(parsed.items.len(), 1);
        assert!(matches!(parsed.items[0].kind, ItemKind::Function(_)));
    }
}
