use vela_common::{Diagnostic, SourceId, Span};

use crate::ast::{FloatLiteral, FloatSuffix, IntRadix, IntegerLiteral, IntegerSuffix};
use crate::token::{InterpolatedStringTokenPart, Keyword, Symbol, Token, TokenKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn lex(source: SourceId, text: &str) -> Lexed {
    Lexer::new(source, text).lex()
}

#[must_use]
pub(crate) fn lex_at(source: SourceId, text: &str, base_offset: u32) -> Lexed {
    Lexer::new_at(source, text, base_offset).lex()
}

struct Lexer<'src> {
    source: SourceId,
    text: &'src str,
    base_offset: u32,
    offset: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'src> Lexer<'src> {
    fn new(source: SourceId, text: &'src str) -> Self {
        Self {
            source,
            text,
            base_offset: 0,
            offset: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn new_at(source: SourceId, text: &'src str, base_offset: u32) -> Self {
        Self {
            source,
            text,
            base_offset,
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
                '"' if self.starts_with_at_current("\"\"\"") => self.lex_multiline_string(),
                '"' => self.lex_string(),
                'f' if self.peek_next_char() == Some('"') => self.lex_interpolated_string(),
                'b' if self.peek_next_char() == Some('"') => self.lex_byte_string(),
                '\'' => self.lex_char(),
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

    fn bump_chars(&mut self, count: usize) {
        for _ in 0..count {
            self.bump_char();
        }
    }

    fn starts_with_at_current(&self, value: &str) -> bool {
        self.text
            .get(self.offset..)
            .is_some_and(|rest| rest.starts_with(value))
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
                    let escape_start = self.offset;
                    self.bump_char();
                    if let Some(escaped) = self.peek_char() {
                        if escaped == 'u' && self.peek_next_char() == Some('{') {
                            if let Some(decoded) = self.consume_unicode_escape() {
                                value.push(decoded);
                            }
                            continue;
                        }
                        self.bump_char();
                        let decoded = match escaped {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '0' => '\0',
                            '"' => '"',
                            '\\' => '\\',
                            '/' => '/',
                            other => {
                                self.push_string_escape_diagnostic(escape_start);
                                other
                            }
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

    fn lex_char(&mut self) {
        let start = self.offset;
        self.bump_char();

        let Some(value) = self.consume_char_literal_value(start) else {
            self.finish_invalid_char_literal(start);
            return;
        };

        match self.peek_char() {
            Some('\'') => {
                self.bump_char();
                self.push_token(TokenKind::Char(value), start, self.offset);
            }
            _ => self.finish_invalid_char_literal(start),
        }
    }

    fn consume_char_literal_value(&mut self, start: usize) -> Option<char> {
        let ch = self.peek_char()?;
        match ch {
            '\'' | '\n' => None,
            '\\' => {
                self.bump_char();
                let escape_start = self.offset.saturating_sub(1);
                let escaped = self.peek_char()?;
                if escaped == 'u' && self.peek_next_char() == Some('{') {
                    return self.consume_unicode_escape();
                }
                self.bump_char();
                Some(match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '0' => '\0',
                    '\'' => '\'',
                    '"' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    other => {
                        self.push_char_escape_diagnostic(escape_start);
                        other
                    }
                })
            }
            other => {
                self.bump_char();
                if other == '\0' {
                    self.push_char_literal_diagnostic(start);
                }
                Some(other)
            }
        }
    }

    fn finish_invalid_char_literal(&mut self, start: usize) {
        while let Some(ch) = self.peek_char() {
            self.bump_char();
            if ch == '\'' || ch == '\n' {
                break;
            }
        }
        self.push_char_literal_diagnostic(start);
    }

    fn lex_multiline_string(&mut self) {
        let start = self.offset;
        self.bump_chars(3);
        let content_start = self.offset;

        while self.peek_char().is_some() {
            if self.starts_with_at_current("\"\"\"") {
                let value = self.slice(content_start, self.offset).to_owned();
                self.bump_chars(3);
                self.push_token(TokenKind::String(value), start, self.offset);
                return;
            }
            self.bump_char();
        }

        self.diagnostics.push(
            Diagnostic::error("unterminated multiline string literal")
                .with_code("E_LEX_MULTILINE_STRING")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn lex_interpolated_string(&mut self) {
        let start = self.offset;
        self.bump_char();
        let multiline = self.starts_with_at_current("\"\"\"");
        if multiline {
            self.bump_chars(3);
        } else {
            self.bump_char();
        }

        let mut parts = Vec::new();
        let mut text = String::new();

        while let Some(ch) = self.peek_char() {
            if multiline && self.starts_with_at_current("\"\"\"") {
                self.bump_chars(3);
                self.flush_interpolated_text(&mut parts, &mut text);
                self.push_token(TokenKind::InterpolatedString(parts), start, self.offset);
                return;
            }
            if !multiline && ch == '"' {
                self.bump_char();
                self.flush_interpolated_text(&mut parts, &mut text);
                self.push_token(TokenKind::InterpolatedString(parts), start, self.offset);
                return;
            }
            if !multiline && ch == '\n' {
                break;
            }
            match ch {
                '\\' if !multiline => self.lex_interpolated_escape(&mut text),
                '{' if self.peek_next_char() == Some('{') => {
                    self.bump_char();
                    self.bump_char();
                    text.push('{');
                }
                '}' if self.peek_next_char() == Some('}') => {
                    self.bump_char();
                    self.bump_char();
                    text.push('}');
                }
                '{' => {
                    self.flush_interpolated_text(&mut parts, &mut text);
                    if let Some(part) = self.lex_interpolation_expr() {
                        parts.push(part);
                    }
                }
                '}' => {
                    let span_start = self.offset;
                    self.bump_char();
                    self.diagnostics.push(
                        Diagnostic::error("unmatched `}` in interpolated string")
                            .with_code("E_LEX_STRING_INTERPOLATION")
                            .with_span(self.span(span_start, self.offset)),
                    );
                    text.push('}');
                }
                other => {
                    self.bump_char();
                    text.push(other);
                }
            }
        }

        let code = if multiline {
            "E_LEX_MULTILINE_STRING"
        } else {
            "E_LEX_STRING"
        };
        self.diagnostics.push(
            Diagnostic::error("unterminated interpolated string literal")
                .with_code(code)
                .with_span(self.span(start, self.offset)),
        );
    }

    fn lex_interpolated_escape(&mut self, text: &mut String) {
        let escape_start = self.offset;
        self.bump_char();
        let Some(escaped) = self.peek_char() else {
            self.push_string_escape_diagnostic(escape_start);
            return;
        };
        if escaped == 'u' && self.peek_next_char() == Some('{') {
            if let Some(decoded) = self.consume_unicode_escape() {
                text.push(decoded);
            }
            return;
        }
        self.bump_char();
        let decoded = match escaped {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '0' => '\0',
            '"' => '"',
            '\\' => '\\',
            '/' => '/',
            '{' => '{',
            '}' => '}',
            other => {
                self.push_string_escape_diagnostic(escape_start);
                other
            }
        };
        text.push(decoded);
    }

    fn lex_interpolation_expr(&mut self) -> Option<InterpolatedStringTokenPart> {
        let open_start = self.offset;
        self.bump_char();
        let expr_start = self.offset;
        let mut depth = 0_u32;

        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => self.skip_quoted_source_string(),
                '\'' => self.skip_quoted_source_char(),
                'b' | 'f' if self.peek_next_char() == Some('"') => {
                    self.bump_char();
                    self.skip_quoted_source_string();
                }
                '/' if self.peek_next_char() == Some('/') => self.skip_line_comment(),
                '/' if self.peek_next_char() == Some('*') => self.skip_block_comment(),
                '{' => {
                    depth = depth.saturating_add(1);
                    self.bump_char();
                }
                '}' if depth == 0 => {
                    let expr_end = self.offset;
                    let source = self.slice(expr_start, expr_end).to_owned();
                    let span = self.span(expr_start, expr_end);
                    self.bump_char();
                    return Some(InterpolatedStringTokenPart::Expr { source, span });
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    self.bump_char();
                }
                _ => {
                    self.bump_char();
                }
            }
        }

        self.diagnostics.push(
            Diagnostic::error("unterminated string interpolation")
                .with_code("E_LEX_STRING_INTERPOLATION")
                .with_span(self.span(open_start, self.offset)),
        );
        None
    }

    fn skip_quoted_source_string(&mut self) {
        if self.starts_with_at_current("\"\"\"") {
            self.bump_chars(3);
            while self.peek_char().is_some() {
                if self.starts_with_at_current("\"\"\"") {
                    self.bump_chars(3);
                    return;
                }
                self.bump_char();
            }
            return;
        }

        self.bump_char();
        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.bump_char();
                    return;
                }
                '\\' => {
                    self.bump_char();
                    self.bump_char();
                }
                _ => {
                    self.bump_char();
                }
            }
        }
    }

    fn skip_quoted_source_char(&mut self) {
        self.bump_char();
        while let Some(ch) = self.peek_char() {
            match ch {
                '\'' => {
                    self.bump_char();
                    return;
                }
                '\\' => {
                    self.bump_char();
                    self.bump_char();
                }
                _ => {
                    self.bump_char();
                }
            }
        }
    }

    fn flush_interpolated_text(
        &mut self,
        parts: &mut Vec<InterpolatedStringTokenPart>,
        text: &mut String,
    ) {
        if !text.is_empty() {
            parts.push(InterpolatedStringTokenPart::Text(std::mem::take(text)));
        }
    }

    fn lex_byte_string(&mut self) {
        let start = self.offset;
        self.bump_char();
        self.bump_char();
        let mut value = Vec::new();

        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.bump_char();
                    self.push_token(TokenKind::Bytes(value), start, self.offset);
                    return;
                }
                '\\' => self.lex_byte_escape(&mut value),
                '\n' => {
                    break;
                }
                other if other.is_ascii() => {
                    self.bump_char();
                    value.push(other as u8);
                }
                _ => {
                    let char_start = self.offset;
                    self.bump_char();
                    self.push_byte_character_diagnostic(char_start);
                }
            }
        }

        self.diagnostics.push(
            Diagnostic::error("unterminated byte string literal")
                .with_code("E_LEX_BYTE_STRING")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn lex_byte_escape(&mut self, value: &mut Vec<u8>) {
        let escape_start = self.offset;
        self.bump_char();
        let Some(escaped) = self.peek_char() else {
            self.push_byte_escape_diagnostic(escape_start);
            return;
        };

        if escaped == 'u' && self.peek_next_char() == Some('{') {
            self.skip_invalid_unicode_escape();
            self.push_byte_escape_diagnostic(escape_start);
            return;
        }

        self.bump_char();
        match escaped {
            'n' => value.push(b'\n'),
            'r' => value.push(b'\r'),
            't' => value.push(b'\t'),
            '0' => value.push(b'\0'),
            '"' => value.push(b'"'),
            '\\' => value.push(b'\\'),
            'x' => {
                if let Some(byte) = self.consume_byte_hex_escape(escape_start) {
                    value.push(byte);
                }
            }
            other => {
                self.push_byte_escape_diagnostic(escape_start);
                if other.is_ascii() {
                    value.push(other as u8);
                }
            }
        }
    }

    fn consume_byte_hex_escape(&mut self, escape_start: usize) -> Option<u8> {
        let mut byte = 0_u8;
        for _ in 0..2 {
            let Some(ch) = self.peek_char() else {
                self.push_byte_escape_diagnostic(escape_start);
                return None;
            };
            if ch == '"' || ch == '\n' {
                self.push_byte_escape_diagnostic(escape_start);
                return None;
            }
            let Some(digit) = ch.to_digit(16) else {
                self.bump_char();
                self.push_byte_escape_diagnostic(escape_start);
                return None;
            };
            self.bump_char();
            byte = (byte << 4) | digit as u8;
        }
        Some(byte)
    }

    fn consume_unicode_escape(&mut self) -> Option<char> {
        let start = self.offset;
        self.bump_char();
        self.bump_char();
        let mut digits = String::new();

        while let Some(ch) = self.peek_char() {
            match ch {
                '}' => {
                    self.bump_char();
                    return self.decode_unicode_escape(start, &digits);
                }
                hex if hex.is_ascii_hexdigit() => {
                    digits.push(hex);
                    self.bump_char();
                }
                _ => {
                    self.skip_invalid_unicode_escape();
                    self.push_unicode_escape_diagnostic(start);
                    return None;
                }
            }
        }

        self.push_unicode_escape_diagnostic(start);
        None
    }

    fn decode_unicode_escape(&mut self, start: usize, digits: &str) -> Option<char> {
        if digits.is_empty() {
            self.push_unicode_escape_diagnostic(start);
            return None;
        }
        let Ok(value) = u32::from_str_radix(digits, 16) else {
            self.push_unicode_escape_diagnostic(start);
            return None;
        };
        let Some(decoded) = char::from_u32(value) else {
            self.push_unicode_escape_diagnostic(start);
            return None;
        };
        Some(decoded)
    }

    fn skip_invalid_unicode_escape(&mut self) {
        while let Some(ch) = self.peek_char() {
            self.bump_char();
            if ch == '}' || ch == '"' || ch == '\n' {
                break;
            }
        }
    }

    fn push_unicode_escape_diagnostic(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error("invalid unicode escape")
                .with_code("E_LEX_UNICODE_ESCAPE")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn push_string_escape_diagnostic(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error("invalid string escape")
                .with_code("E_LEX_STRING_ESCAPE")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn push_char_escape_diagnostic(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error("invalid char escape")
                .with_code("E_LEX_CHAR_ESCAPE")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn push_char_literal_diagnostic(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error("invalid char literal")
                .with_code("E_LEX_CHAR_LITERAL")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn push_byte_escape_diagnostic(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error("invalid byte string escape")
                .with_code("E_LEX_BYTE_ESCAPE")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn push_byte_character_diagnostic(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error("invalid byte string character")
                .with_code("E_LEX_BYTE_CHAR")
                .with_span(self.span(start, self.offset)),
        );
    }

    fn lex_number(&mut self) {
        let start = self.offset;
        if self.peek_char() == Some('0') && matches!(self.peek_next_char(), Some('x' | 'X')) {
            self.lex_radix_int(start, |ch| ch.is_ascii_hexdigit());
            return;
        }
        if self.peek_char() == Some('0') && matches!(self.peek_next_char(), Some('b' | 'B')) {
            self.lex_radix_int(start, |ch| matches!(ch, '0' | '1'));
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

        let number_end = self.offset;
        let suffix = self.consume_numeric_suffix();
        let token_end = self.offset;
        if is_float {
            let suffix = self.validate_float_suffix(suffix);
            self.push_token(
                TokenKind::Float(FloatLiteral {
                    text: self.slice(start, number_end).to_owned(),
                    suffix,
                }),
                start,
                token_end,
            );
        } else {
            let suffix = self.validate_integer_suffix(suffix);
            self.push_token(
                TokenKind::Int(IntegerLiteral {
                    text: self.slice(start, number_end).to_owned(),
                    radix: IntRadix::Decimal,
                    suffix,
                }),
                start,
                token_end,
            );
        }
    }

    fn lex_radix_int(&mut self, start: usize, is_digit: impl Fn(char) -> bool) {
        self.bump_char();
        let prefix = self.bump_char();
        let has_digit = self.consume_digits_or_underscores_with(is_digit);
        let number_end = self.offset;
        if !has_digit || matches!(prefix, Some('X' | 'B')) {
            self.push_int_literal_diagnostic(start);
        }
        let suffix = self.consume_numeric_suffix();
        let token_end = self.offset;
        let suffix = self.validate_integer_suffix(suffix);
        self.push_token(
            TokenKind::Int(IntegerLiteral {
                text: self.slice(start, number_end).to_owned(),
                radix: IntRadix::from_literal_text(self.slice(start, number_end)),
                suffix,
            }),
            start,
            token_end,
        );
    }

    fn consume_digits_or_underscores(&mut self) {
        self.consume_digits_or_underscores_with(|ch| ch.is_ascii_digit());
    }

    fn consume_digits_or_underscores_with(&mut self, is_digit: impl Fn(char) -> bool) -> bool {
        let mut has_digit = false;
        while let Some(ch) = self.peek_char() {
            if ch == '_' {
                self.bump_char();
            } else if is_digit(ch) {
                has_digit = true;
                self.bump_char();
            } else {
                break;
            }
        }
        has_digit
    }

    fn push_int_literal_diagnostic(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error("invalid integer literal")
                .with_code("E_LEX_INT")
                .with_span(self.span(start, self.offset)),
        );
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

    fn consume_numeric_suffix(&mut self) -> Option<(usize, String)> {
        let start = self.offset;
        if !self
            .peek_char()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        {
            return None;
        }
        while self
            .peek_char()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        {
            self.bump_char();
        }
        Some((start, self.slice(start, self.offset).to_owned()))
    }

    fn validate_integer_suffix(
        &mut self,
        suffix: Option<(usize, String)>,
    ) -> Option<IntegerSuffix> {
        let (start, suffix) = suffix?;
        let parsed = match suffix.as_str() {
            "i8" => Some(IntegerSuffix::I8),
            "i16" => Some(IntegerSuffix::I16),
            "i32" => Some(IntegerSuffix::I32),
            "i64" => Some(IntegerSuffix::I64),
            "u8" => Some(IntegerSuffix::U8),
            "u16" => Some(IntegerSuffix::U16),
            "u32" => Some(IntegerSuffix::U32),
            "u64" => Some(IntegerSuffix::U64),
            _ => None,
        };
        if parsed.is_none() {
            self.push_numeric_suffix_diagnostic(start);
        }
        parsed
    }

    fn validate_float_suffix(&mut self, suffix: Option<(usize, String)>) -> Option<FloatSuffix> {
        let (start, suffix) = suffix?;
        let parsed = match suffix.as_str() {
            "f32" => Some(FloatSuffix::F32),
            "f64" => Some(FloatSuffix::F64),
            _ => None,
        };
        if parsed.is_none() {
            self.push_numeric_suffix_diagnostic(start);
        }
        parsed
    }

    fn push_numeric_suffix_diagnostic(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error("invalid numeric literal suffix")
                .with_code("E_LEX_NUMERIC_SUFFIX")
                .with_span(self.span(start, self.offset)),
        );
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
            ':' if self.peek_char() == Some(':') => {
                self.bump_char();
                Some(Symbol::ColonColon)
            }
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
        Span::new(
            self.source,
            self.base_offset.saturating_add(to_u32(start)),
            self.base_offset.saturating_add(to_u32(end)),
        )
    }

    fn slice(&self, start: usize, end: usize) -> &str {
        self.text.get(start..end).unwrap_or_default()
    }
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
