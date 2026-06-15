use vela_common::{Diagnostic, SourceId, Span};

use crate::lexer::lex;
use crate::token::{Keyword, Symbol, TokenKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatElementStream {
    elements: Vec<FormatElement>,
    diagnostics: Vec<Diagnostic>,
}

impl FormatElementStream {
    #[must_use]
    pub fn elements(&self) -> &[FormatElement] {
        &self.elements
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatElement {
    kind: FormatElementKind,
    span: Span,
    text: String,
}

impl FormatElement {
    #[must_use]
    pub fn kind(&self) -> &FormatElementKind {
        &self.kind
    }

    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormatElementKind {
    Token(TokenKind),
    Trivia(TriviaKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriviaKind {
    Whitespace,
    LineComment,
    BlockComment,
    Shebang,
    Unknown,
}

impl TriviaKind {
    #[must_use]
    pub const fn is_comment(self) -> bool {
        matches!(self, Self::LineComment | Self::BlockComment)
    }
}

#[must_use]
pub fn extract_format_elements(source: SourceId, text: &str) -> FormatElementStream {
    let lexed = lex(source, text);
    let mut elements = Vec::new();
    let mut cursor = 0_usize;

    for token in &lexed.tokens {
        let token_start = token.span.start as usize;
        let token_end = token.span.end as usize;
        push_trivia_segments(source, text, cursor, token_start, &mut elements);
        elements.push(FormatElement {
            kind: FormatElementKind::Token(token.kind.clone()),
            span: token.span,
            text: slice(text, token_start, token_end).to_owned(),
        });
        cursor = token_end;
    }

    FormatElementStream {
        elements,
        diagnostics: lexed.diagnostics,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormattedSource {
    text: String,
    diagnostics: Vec<Diagnostic>,
}

impl FormattedSource {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

#[must_use]
pub fn format_source(source: SourceId, text: &str) -> FormattedSource {
    let stream = extract_format_elements(source, text);
    let mut formatter = Formatter::new();
    formatter.format(stream.elements());
    FormattedSource {
        text: formatter.finish(),
        diagnostics: stream.diagnostics,
    }
}

#[derive(Debug, Default)]
struct Formatter {
    output: String,
    indent: usize,
    line_start: bool,
    pending_blank_lines: usize,
    previous_token: Option<TokenKind>,
    delimiter_stack: Vec<Symbol>,
}

impl Formatter {
    fn new() -> Self {
        Self {
            line_start: true,
            ..Self::default()
        }
    }

    fn format(&mut self, elements: &[FormatElement]) {
        for element in elements {
            match element.kind() {
                FormatElementKind::Token(TokenKind::Eof) => {}
                FormatElementKind::Token(token) => self.write_token(token, element.text()),
                FormatElementKind::Trivia(kind) => self.write_trivia(*kind, element.text()),
            }
        }
    }

    fn finish(mut self) -> String {
        self.trim_trailing_horizontal_space();
        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.output
    }

    fn write_trivia(&mut self, kind: TriviaKind, text: &str) {
        match kind {
            TriviaKind::Whitespace => {
                self.pending_blank_lines = self
                    .pending_blank_lines
                    .max(text.matches('\n').count().saturating_sub(1));
            }
            TriviaKind::LineComment | TriviaKind::Shebang => self.write_line_comment(text),
            TriviaKind::BlockComment => self.write_block_comment(text),
            TriviaKind::Unknown => self.write_unknown_trivia(text),
        }
    }

    fn write_line_comment(&mut self, text: &str) {
        if !self.line_start {
            self.output.push(' ');
        }
        self.write_indent_if_needed();
        self.output.push_str(text.trim_end());
        self.newline();
    }

    fn write_block_comment(&mut self, text: &str) {
        if text.contains('\n') {
            self.ensure_line_start();
            self.write_indent_if_needed();
            self.output.push_str(text.trim_end());
            self.newline();
        } else {
            if !self.line_start {
                self.output.push(' ');
            }
            self.write_indent_if_needed();
            self.output.push_str(text.trim_end());
        }
    }

    fn write_unknown_trivia(&mut self, text: &str) {
        self.write_indent_if_needed();
        self.output.push_str(text);
    }

    fn write_token(&mut self, token: &TokenKind, text: &str) {
        match token {
            TokenKind::Symbol(symbol) => self.write_symbol(*symbol, text),
            _ => {
                self.write_space_before_word(token);
                self.write_indent_if_needed();
                self.output.push_str(text);
            }
        }
        self.previous_token = Some(token.clone());
    }

    fn write_symbol(&mut self, symbol: Symbol, text: &str) {
        match symbol {
            Symbol::LBrace => {
                self.write_space_before_open_brace();
                self.write_indent_if_needed();
                self.output.push_str(text);
                self.delimiter_stack.push(symbol);
                self.indent = self.indent.saturating_add(1);
                self.newline();
            }
            Symbol::RBrace => {
                self.indent = self.indent.saturating_sub(1);
                self.pop_delimiter(Symbol::LBrace);
                self.ensure_line_start();
                self.write_indent_if_needed();
                self.output.push_str(text);
            }
            Symbol::LParen | Symbol::LBracket => {
                self.write_indent_if_needed();
                self.output.push_str(text);
                self.delimiter_stack.push(symbol);
            }
            Symbol::RParen => {
                self.trim_trailing_horizontal_space();
                self.pop_delimiter(Symbol::LParen);
                self.output.push_str(text);
            }
            Symbol::RBracket => {
                self.trim_trailing_horizontal_space();
                self.pop_delimiter(Symbol::LBracket);
                self.output.push_str(text);
            }
            Symbol::Comma => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
                if self.in_brace_block() {
                    self.newline();
                } else {
                    self.output.push(' ');
                }
            }
            Symbol::Semicolon => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
                self.newline();
            }
            Symbol::Dot | Symbol::ColonColon | Symbol::Question => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
            }
            Symbol::Colon => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
                self.output.push(' ');
            }
            Symbol::Arrow | Symbol::FatArrow => self.write_spaced_symbol(text),
            symbol if is_assignment_or_binary_symbol(symbol) => self.write_spaced_symbol(text),
            Symbol::Pipe => {
                if matches!(
                    self.previous_token,
                    None | Some(TokenKind::Symbol(
                        Symbol::LParen | Symbol::Equal | Symbol::Comma
                    ))
                ) {
                    self.write_indent_if_needed();
                    self.output.push_str(text);
                } else {
                    self.write_spaced_symbol(text);
                }
            }
            _ => {
                self.write_indent_if_needed();
                self.output.push_str(text);
            }
        }
    }

    fn write_space_before_word(&mut self, token: &TokenKind) {
        if self.line_start || !needs_space_between(self.previous_token.as_ref(), token) {
            return;
        }
        self.trim_trailing_horizontal_space();
        self.output.push(' ');
    }

    fn write_space_before_open_brace(&mut self) {
        if self.line_start {
            return;
        }
        match self.previous_token {
            Some(TokenKind::Symbol(
                Symbol::LBrace | Symbol::LBracket | Symbol::ColonColon | Symbol::Dot,
            )) => {}
            _ => {
                self.trim_trailing_horizontal_space();
                self.output.push(' ');
            }
        }
    }

    fn write_spaced_symbol(&mut self, text: &str) {
        if !self.line_start {
            self.trim_trailing_horizontal_space();
            self.output.push(' ');
        }
        self.write_indent_if_needed();
        self.output.push_str(text);
        self.output.push(' ');
    }

    fn write_indent_if_needed(&mut self) {
        if !self.line_start {
            return;
        }
        self.flush_blank_lines();
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.line_start = false;
    }

    fn flush_blank_lines(&mut self) {
        if self.output.is_empty() {
            self.pending_blank_lines = 0;
            return;
        }
        for _ in 0..self.pending_blank_lines.min(2) {
            if !self.output.ends_with('\n') {
                self.output.push('\n');
            }
            self.output.push('\n');
        }
        self.pending_blank_lines = 0;
    }

    fn ensure_line_start(&mut self) {
        if !self.line_start {
            self.newline();
        }
    }

    fn newline(&mut self) {
        self.trim_trailing_horizontal_space();
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.line_start = true;
    }

    fn trim_trailing_horizontal_space(&mut self) {
        while self.output.ends_with([' ', '\t']) {
            self.output.pop();
        }
    }

    fn pop_delimiter(&mut self, expected: Symbol) {
        if let Some(position) = self
            .delimiter_stack
            .iter()
            .rposition(|symbol| *symbol == expected)
        {
            self.delimiter_stack.truncate(position);
        }
    }

    fn in_brace_block(&self) -> bool {
        self.delimiter_stack.last() == Some(&Symbol::LBrace)
    }
}

fn needs_space_between(previous: Option<&TokenKind>, current: &TokenKind) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if matches!(
        previous,
        TokenKind::Symbol(
            Symbol::LParen | Symbol::LBracket | Symbol::Dot | Symbol::ColonColon | Symbol::Bang
        )
    ) || matches!(
        current,
        TokenKind::Symbol(
            Symbol::RParen
                | Symbol::RBracket
                | Symbol::RBrace
                | Symbol::Comma
                | Symbol::Dot
                | Symbol::ColonColon
                | Symbol::Semicolon
                | Symbol::Question
        )
    ) {
        return false;
    }
    is_word_like(previous) && is_word_like(current)
}

fn is_word_like(token: &TokenKind) -> bool {
    matches!(
        token,
        TokenKind::Ident(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Char(_)
            | TokenKind::String(_)
            | TokenKind::InterpolatedString(_)
            | TokenKind::Bytes(_)
            | TokenKind::Keyword(
                Keyword::Use
                    | Keyword::Pub
                    | Keyword::Const
                    | Keyword::Global
                    | Keyword::Let
                    | Keyword::Fn
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Trait
                    | Keyword::Impl
                    | Keyword::For
                    | Keyword::If
                    | Keyword::Else
                    | Keyword::Match
                    | Keyword::Return
                    | Keyword::Break
                    | Keyword::Continue
                    | Keyword::True
                    | Keyword::False
                    | Keyword::Null
                    | Keyword::SelfValue
                    | Keyword::In
                    | Keyword::As
            )
    )
}

fn is_assignment_or_binary_symbol(symbol: Symbol) -> bool {
    matches!(
        symbol,
        Symbol::Equal
            | Symbol::PlusEqual
            | Symbol::MinusEqual
            | Symbol::StarEqual
            | Symbol::SlashEqual
            | Symbol::PercentEqual
            | Symbol::Plus
            | Symbol::Minus
            | Symbol::Star
            | Symbol::Slash
            | Symbol::Percent
            | Symbol::BangEqual
            | Symbol::BangEqualEqual
            | Symbol::EqualEqual
            | Symbol::EqualEqualEqual
            | Symbol::Less
            | Symbol::LessEqual
            | Symbol::Greater
            | Symbol::GreaterEqual
            | Symbol::AndAnd
            | Symbol::OrOr
            | Symbol::DotDot
            | Symbol::DotDotEqual
    )
}

fn push_trivia_segments(
    source: SourceId,
    text: &str,
    mut cursor: usize,
    end: usize,
    elements: &mut Vec<FormatElement>,
) {
    while cursor < end {
        let rest = slice(text, cursor, end);
        if cursor == 0 && rest.starts_with("#!") {
            let next = cursor + line_comment_len(rest);
            push_trivia(source, text, cursor, next, TriviaKind::Shebang, elements);
            cursor = next;
        } else if rest.starts_with("//") {
            let next = cursor + line_comment_len(rest);
            push_trivia(
                source,
                text,
                cursor,
                next,
                TriviaKind::LineComment,
                elements,
            );
            cursor = next;
        } else if rest.starts_with("/*") {
            let next = cursor + block_comment_len(rest);
            push_trivia(
                source,
                text,
                cursor,
                next,
                TriviaKind::BlockComment,
                elements,
            );
            cursor = next;
        } else if let Some(ch) = rest.chars().next() {
            let kind = if is_layout(ch) {
                TriviaKind::Whitespace
            } else {
                TriviaKind::Unknown
            };
            let next = cursor + trivia_run_len(rest, kind);
            push_trivia(source, text, cursor, next, kind, elements);
            cursor = next;
        } else {
            break;
        }
    }
}

fn push_trivia(
    source: SourceId,
    text: &str,
    start: usize,
    end: usize,
    kind: TriviaKind,
    elements: &mut Vec<FormatElement>,
) {
    elements.push(FormatElement {
        kind: FormatElementKind::Trivia(kind),
        span: span(source, start, end),
        text: slice(text, start, end).to_owned(),
    });
}

fn trivia_run_len(text: &str, kind: TriviaKind) -> usize {
    match kind {
        TriviaKind::Whitespace => text
            .char_indices()
            .find_map(|(offset, ch)| (!is_layout(ch)).then_some(offset))
            .unwrap_or(text.len()),
        TriviaKind::Unknown => text
            .char_indices()
            .skip(1)
            .find_map(|(offset, ch)| {
                (is_layout(ch)
                    || text[offset..].starts_with("//")
                    || text[offset..].starts_with("/*"))
                .then_some(offset)
            })
            .unwrap_or(text.len()),
        TriviaKind::LineComment | TriviaKind::BlockComment | TriviaKind::Shebang => text.len(),
    }
}

fn line_comment_len(text: &str) -> usize {
    text.find('\n').unwrap_or(text.len())
}

fn block_comment_len(text: &str) -> usize {
    let mut depth = 0_u32;
    let mut cursor = 0_usize;
    while cursor < text.len() {
        let rest = &text[cursor..];
        if rest.starts_with("/*") {
            depth = depth.saturating_add(1);
            cursor += 2;
        } else if rest.starts_with("*/") {
            cursor += 2;
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return cursor;
            }
        } else if let Some(ch) = rest.chars().next() {
            cursor += ch.len_utf8();
        } else {
            break;
        }
    }
    text.len()
}

fn is_layout(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\n')
}

fn span(source: SourceId, start: usize, end: usize) -> Span {
    Span::new(source, to_u32(start), to_u32(end))
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn slice(text: &str, start: usize, end: usize) -> &str {
    text.get(start..end).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::{Keyword, Symbol};

    fn source_id() -> SourceId {
        SourceId::new(1)
    }

    #[test]
    fn formatting_extracts_tokens_and_trivia_in_source_order() {
        let source = "pub fn main() {\n    // keep\n    return 1\n}\n";
        let stream = extract_format_elements(source_id(), source);

        assert!(stream.diagnostics().is_empty());
        assert_eq!(reconstruct(&stream), source);
        assert_eq!(
            stream
                .elements()
                .iter()
                .filter(|element| matches!(element.kind(), FormatElementKind::Token(_)))
                .count(),
            10
        );
        assert!(stream.elements().iter().any(|element| matches!(
            element.kind(),
            FormatElementKind::Token(TokenKind::Keyword(Keyword::Return))
        )));
    }

    #[test]
    fn formatting_extracts_comments_and_blank_line_groups() {
        let source = "fn main() {\n    /* one\n\n       two */\n\n    // tail\n}\n";
        let stream = extract_format_elements(source_id(), source);
        let comments = stream
            .elements()
            .iter()
            .filter_map(|element| match element.kind() {
                FormatElementKind::Trivia(kind) if kind.is_comment() => Some(element.text()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let blank_line_group = stream.elements().iter().any(|element| {
            matches!(
                element.kind(),
                FormatElementKind::Trivia(TriviaKind::Whitespace)
            ) && element.text().matches('\n').count() >= 2
        });

        assert_eq!(reconstruct(&stream), source);
        assert_eq!(comments, vec!["/* one\n\n       two */", "// tail"]);
        assert!(blank_line_group);
    }

    #[test]
    fn formatting_extracts_shebang_as_trivia() {
        let source = "#!/usr/bin/env vela\nfn main() { return 1 }\n";
        let stream = extract_format_elements(source_id(), source);

        assert!(matches!(
            stream.elements().first().map(FormatElement::kind),
            Some(FormatElementKind::Trivia(TriviaKind::Shebang))
        ));
        assert_eq!(stream.elements()[0].span(), Span::new(source_id(), 0, 19));
        assert!(stream.elements().iter().any(|element| {
            matches!(
                element.kind(),
                FormatElementKind::Token(TokenKind::Symbol(Symbol::LBrace))
            )
        }));
        assert_eq!(reconstruct(&stream), source);
    }

    #[test]
    fn formatting_formats_expressions_and_function_blocks() {
        let source = "pub fn main(){return 1+2*3}";
        let formatted = format_source(source_id(), source);

        assert!(formatted.diagnostics().is_empty());
        assert_eq!(
            formatted.text(),
            "pub fn main() {\n    return 1 + 2 * 3\n}\n"
        );
    }

    #[test]
    fn formatting_preserves_comments_while_formatting_blocks() {
        let source = "fn main(){// keep\nlet value=1\n/* block\n\ncomment */\nreturn value}";
        let formatted = format_source(source_id(), source);

        assert_eq!(
            formatted.text(),
            "fn main() {\n    // keep\n    let value = 1\n    /* block\n\ncomment */\n    return value\n}\n"
        );
    }

    fn reconstruct(stream: &FormatElementStream) -> String {
        stream
            .elements()
            .iter()
            .map(FormatElement::text)
            .collect::<String>()
    }
}
