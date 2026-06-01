use std::collections::BTreeMap;

use crate::{Diagnostic, SourceId, Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticSource {
    pub id: SourceId,
    pub name: String,
    pub text: String,
}

impl DiagnosticSource {
    #[must_use]
    pub fn new(id: SourceId, name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DiagnosticRenderer {
    sources: BTreeMap<SourceId, DiagnosticSource>,
}

impl DiagnosticRenderer {
    #[must_use]
    pub fn new(sources: impl IntoIterator<Item = DiagnosticSource>) -> Self {
        Self {
            sources: sources
                .into_iter()
                .map(|source| (source.id, source))
                .collect(),
        }
    }

    #[must_use]
    pub fn render(&self, diagnostic: &Diagnostic) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(render_header(diagnostic));

        if let Some(span) = diagnostic.span {
            self.push_span_lines(&mut lines, span, diagnostic.message.as_str());
        }

        for label in &diagnostic.labels {
            self.push_span_lines(&mut lines, label.span, label.message.as_str());
        }

        lines
    }

    fn push_span_lines(&self, lines: &mut Vec<String>, span: Span, message: &str) {
        let Some(source) = self.sources.get(&span.source) else {
            lines.push(format!(
                "  --> <source {}>:{}..{}",
                span.source.get(),
                span.start,
                span.end
            ));
            if !message.is_empty() {
                lines.push(format!("   = {message}"));
            }
            return;
        };

        let Some(line) = line_for_span(&source.text, span) else {
            lines.push(format!(
                "  --> {}:{}..{}",
                source.name, span.start, span.end
            ));
            if !message.is_empty() {
                lines.push(format!("   = {message}"));
            }
            return;
        };

        lines.push(format!(
            "  --> {}:{}:{}",
            source.name, line.number, line.column
        ));
        lines.push("   |".to_owned());
        lines.push(format!("{:>3} | {}", line.number, line.text));
        lines.push(format!(
            "   | {}{}{}",
            " ".repeat(line.caret_padding),
            "^".repeat(line.caret_width),
            if message.is_empty() {
                String::new()
            } else {
                format!(" {message}")
            }
        ));
    }
}

#[must_use]
pub fn render_diagnostic(
    diagnostic: &Diagnostic,
    sources: impl IntoIterator<Item = DiagnosticSource>,
) -> Vec<String> {
    DiagnosticRenderer::new(sources).render(diagnostic)
}

fn render_header(diagnostic: &Diagnostic) -> String {
    match diagnostic.code.as_deref() {
        Some(code) => format!("{}[{code}]: {}", diagnostic.severity, diagnostic.message),
        None => format!("{}: {}", diagnostic.severity, diagnostic.message),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RenderLine {
    number: usize,
    column: usize,
    text: String,
    caret_padding: usize,
    caret_width: usize,
}

fn line_for_span(source: &str, span: Span) -> Option<RenderLine> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    if start > source.len()
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return None;
    }

    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |index| start + index);
    let line_text = &source[line_start..line_end];
    let line_number = source[..line_start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let prefix = &source[line_start..start];
    let highlighted = &source[start..end.max(start + 1).min(line_end)];

    Some(RenderLine {
        number: line_number,
        column: prefix.chars().count() + 1,
        text: line_text.to_owned(),
        caret_padding: prefix.chars().count(),
        caret_width: highlighted.chars().count().max(1),
    })
}

#[cfg(test)]
mod tests {
    use crate::{Diagnostic, SourceId, Span};

    use super::{DiagnosticSource, render_diagnostic};

    #[test]
    fn renders_primary_span_and_related_labels() {
        let source = DiagnosticSource::new(
            SourceId::new(1),
            "combat.vela",
            "fn main() {\n    return player.levle;\n}\n",
        );
        let diagnostic = Diagnostic::error("unknown field `levle`")
            .with_code("hir::unknown_field")
            .with_span(Span::new(SourceId::new(1), 30, 35))
            .with_label(Span::new(SourceId::new(1), 23, 29), "receiver is `Player`");

        let lines = render_diagnostic(&diagnostic, [source]);

        assert_eq!(
            lines.join("\n"),
            "\
error[hir::unknown_field]: unknown field `levle`
  --> combat.vela:2:19
   |
  2 |     return player.levle;
   |                   ^^^^^ unknown field `levle`
  --> combat.vela:2:12
   |
  2 |     return player.levle;
   |            ^^^^^^ receiver is `Player`"
        );
    }

    #[test]
    fn renders_missing_sources_as_stable_offsets() {
        let diagnostic = Diagnostic::warning("unmapped source")
            .with_code("test::missing_source")
            .with_span(Span::new(SourceId::new(9), 4, 8));

        let lines = render_diagnostic(&diagnostic, []);

        assert_eq!(
            lines,
            [
                "warning[test::missing_source]: unmapped source",
                "  --> <source 9>:4..8",
                "   = unmapped source",
            ]
        );
    }
}
