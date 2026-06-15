use vela_common::{Diagnostic, SourceId, Span};

use crate::lexer::lex;
use crate::token::TokenKind;

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

    fn reconstruct(stream: &FormatElementStream) -> String {
        stream
            .elements()
            .iter()
            .map(FormatElement::text)
            .collect::<String>()
    }
}
