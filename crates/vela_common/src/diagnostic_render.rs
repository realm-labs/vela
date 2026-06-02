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

        let Some(span_lines) = lines_for_span(&source.text, span) else {
            lines.push(format!(
                "  --> {}:{}..{}",
                source.name, span.start, span.end
            ));
            if !message.is_empty() {
                lines.push(format!("   = {message}"));
            }
            return;
        };
        let Some(first_line) = span_lines.first() else {
            return;
        };

        lines.push(format!(
            "  --> {}:{}:{}",
            source.name, first_line.number, first_line.column
        ));
        lines.push("   |".to_owned());
        for (index, line) in span_lines.iter().enumerate() {
            lines.push(format!("{:>3} | {}", line.number, line.text));
            lines.push(format!(
                "   | {}{}{}",
                " ".repeat(line.caret_padding),
                "^".repeat(line.caret_width),
                if index == 0 && !message.is_empty() {
                    format!(" {message}")
                } else {
                    String::new()
                }
            ));
        }
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

fn lines_for_span(source: &str, span: Span) -> Option<Vec<RenderLine>> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    if start > source.len()
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return None;
    }

    let mut lines = Vec::new();
    let mut line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let mut line_number = source[..line_start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let highlight_end = end.max(start + 1).min(source.len());

    loop {
        let line_end = source[line_start..]
            .find('\n')
            .map_or(source.len(), |index| line_start + index);
        let display_end = source[line_start..line_end]
            .strip_suffix('\r')
            .map_or(line_end, |text| line_start + text.len());
        let caret_start = start.max(line_start).min(display_end);
        let caret_end = highlight_end.min(display_end);
        if caret_start <= caret_end && (caret_start < caret_end || lines.is_empty()) {
            let prefix = &source[line_start..caret_start];
            let highlighted = &source[caret_start..caret_end];
            lines.push(RenderLine {
                number: line_number,
                column: prefix.chars().count() + 1,
                text: source[line_start..display_end].to_owned(),
                caret_padding: prefix.chars().count(),
                caret_width: highlighted.chars().count().max(1),
            });
        }

        if line_end >= highlight_end || line_end == source.len() {
            break;
        }
        line_start = line_end + 1;
        line_number += 1;
    }

    Some(lines)
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

    #[test]
    fn renders_multi_line_spans() {
        let source = DiagnosticSource::new(
            SourceId::new(1),
            "combat.vela",
            "fn main() {\n    player.level += 1\n    player.exp = 0\n}\n",
        );
        let diagnostic = Diagnostic::error("top-level host mutation")
            .with_code("hir::top_level_effect")
            .with_span(Span::new(SourceId::new(1), 12, 52));

        let lines = render_diagnostic(&diagnostic, [source]);

        assert_eq!(
            lines.join("\n"),
            "\
error[hir::top_level_effect]: top-level host mutation
  --> combat.vela:2:1
   |
  2 |     player.level += 1
   | ^^^^^^^^^^^^^^^^^^^^^ top-level host mutation
  3 |     player.exp = 0
   | ^^^^^^^^^^^^^^^^^^"
        );
    }
}
