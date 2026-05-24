use vela_common::{Diagnostic, SourceId, Span};

use crate::token::{Keyword, Symbol, Token, TokenKind};

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
        self.skip_shebang();

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

    fn skip_shebang(&mut self) {
        if self.offset != 0
            || !(self.peek_char() == Some('#') && self.peek_next_char() == Some('!'))
        {
            return;
        }
        while let Some(ch) = self.bump_char() {
            if ch == '\n' {
                break;
            }
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
        if self.peek_char() == Some('0') && matches!(self.peek_next_char(), Some('x' | 'X')) {
            self.bump_char();
            self.bump_char();
            self.consume_digits_or_underscores_with(|ch| ch.is_ascii_hexdigit());
            self.push_token(
                TokenKind::Int(self.slice(start, self.offset).to_owned()),
                start,
                self.offset,
            );
            return;
        }
        if self.peek_char() == Some('0') && matches!(self.peek_next_char(), Some('b' | 'B')) {
            self.bump_char();
            self.bump_char();
            self.consume_digits_or_underscores_with(|ch| matches!(ch, '0' | '1'));
            self.push_token(
                TokenKind::Int(self.slice(start, self.offset).to_owned()),
                start,
                self.offset,
            );
            return;
        }

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

        if is_float && self.has_valid_exponent() {
            self.consume_exponent();
        }

        let text = self.slice(start, self.offset).to_owned();
        if is_float {
            self.push_token(TokenKind::Float(text), start, self.offset);
        } else {
            self.push_token(TokenKind::Int(text), start, self.offset);
        }
    }

    fn consume_digits_or_underscores(&mut self) {
        self.consume_digits_or_underscores_with(|ch| ch.is_ascii_digit());
    }

    fn consume_digits_or_underscores_with(&mut self, is_digit: impl Fn(char) -> bool) {
        while self.peek_char().is_some_and(|ch| ch == '_' || is_digit(ch)) {
            self.bump_char();
        }
    }

    fn has_valid_exponent(&self) -> bool {
        let mut chars = self.text.get(self.offset..).unwrap_or_default().chars();
        if !matches!(chars.next(), Some('e' | 'E')) {
            return false;
        }
        match chars.next() {
            Some('+' | '-') => chars.next().is_some_and(|ch| ch.is_ascii_digit()),
            Some(ch) => ch.is_ascii_digit(),
            None => false,
        }
    }

    fn consume_exponent(&mut self) {
        self.bump_char();
        if matches!(self.peek_char(), Some('+' | '-')) {
            self.bump_char();
        }
        self.consume_digits_or_underscores();
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
            '.' if self.peek_char() == Some('.') => {
                self.bump_char();
                if self.peek_char() == Some('=') {
                    self.bump_char();
                    Some(Symbol::DotDotEqual)
                } else {
                    Some(Symbol::DotDot)
                }
            }
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

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
