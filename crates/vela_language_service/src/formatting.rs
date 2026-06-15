use crate::{DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextEdit};
use vela_common::SourceId;
use vela_syntax::formatting::{
    FormatElementKind, TriviaKind, extract_format_elements, format_source,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FormattingIr {
    document_id: DocumentId,
    segments: Vec<FormattingSegment>,
}

impl FormattingIr {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub fn segments(&self) -> &[FormattingSegment] {
        &self.segments
    }

    #[must_use]
    pub fn reconstruct_source(&self) -> String {
        self.segments.iter().map(FormattingSegment::text).collect()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FormattingSegment {
    kind: FormattingSegmentKind,
    range: DiagnosticRange,
    text: String,
}

impl FormattingSegment {
    #[must_use]
    pub const fn kind(&self) -> FormattingSegmentKind {
        self.kind
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FormattingSegmentKind {
    Token,
    Whitespace,
    LineComment,
    BlockComment,
    Shebang,
    UnknownTrivia,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn formatting_ir(&self, document_id: &DocumentId) -> Option<FormattingIr> {
        let source = self.source_db().records().get(document_id)?;
        let line_index = LineIndex::new(source.text());
        let extracted = extract_format_elements(source.source_id(), source.text());
        let segments = extracted
            .elements()
            .iter()
            .map(|element| FormattingSegment {
                kind: service_segment_kind(element.kind()),
                range: DiagnosticRange::new(
                    line_index.position(element.span().start as usize),
                    line_index.position(element.span().end as usize),
                ),
                text: element.text().to_owned(),
            })
            .collect();

        Some(FormattingIr {
            document_id: document_id.clone(),
            segments,
        })
    }

    #[must_use]
    pub fn document_formatting(&self, document_id: &DocumentId) -> Vec<TextEdit> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let formatted = format_document(source.source_id(), source.text());
        if formatted == source.text() {
            return Vec::new();
        }

        vec![TextEdit::new(
            DiagnosticRange::new(
                Position::new(0, 0),
                LineIndex::new(source.text()).position(source.text().len()),
            ),
            formatted,
        )]
    }

    #[must_use]
    pub fn range_formatting(
        &self,
        document_id: &DocumentId,
        range: DiagnosticRange,
    ) -> Vec<TextEdit> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };

        trailing_whitespace_edits(source.text(), range)
    }
}

fn service_segment_kind(kind: &FormatElementKind) -> FormattingSegmentKind {
    match kind {
        FormatElementKind::Token(_) => FormattingSegmentKind::Token,
        FormatElementKind::Trivia(TriviaKind::Whitespace) => FormattingSegmentKind::Whitespace,
        FormatElementKind::Trivia(TriviaKind::LineComment) => FormattingSegmentKind::LineComment,
        FormatElementKind::Trivia(TriviaKind::BlockComment) => FormattingSegmentKind::BlockComment,
        FormatElementKind::Trivia(TriviaKind::Shebang) => FormattingSegmentKind::Shebang,
        FormatElementKind::Trivia(TriviaKind::Unknown) => FormattingSegmentKind::UnknownTrivia,
    }
}

fn format_document(source_id: SourceId, source: &str) -> String {
    format_source(source_id, source).text().to_owned()
}

fn trailing_whitespace_edits(source: &str, range: DiagnosticRange) -> Vec<TextEdit> {
    let line_index = LineIndex::new(source);
    let start = line_index.offset(range.start());
    let end = line_index.offset(range.end());
    if start >= end {
        return Vec::new();
    }

    let mut edits = Vec::new();
    let mut line_start = 0;
    while line_start < source.len() {
        let line_end = source[line_start..]
            .find('\n')
            .map_or(source.len(), |offset| line_start + offset + 1);
        let newline_start = if source.as_bytes()[line_end.saturating_sub(1)] == b'\n' {
            line_end.saturating_sub(1)
        } else {
            line_end
        };
        let body_end =
            if newline_start > line_start && source.as_bytes()[newline_start - 1] == b'\r' {
                newline_start - 1
            } else {
                newline_start
            };
        let trim_start = trimmed_ascii_whitespace_end(source, line_start, body_end);
        let edit_start = trim_start.max(start);
        let edit_end = body_end.min(end);
        if edit_start < edit_end {
            edits.push(TextEdit::new(
                DiagnosticRange::new(
                    line_index.position(edit_start),
                    line_index.position(edit_end),
                ),
                "",
            ));
        }

        if line_end == source.len() {
            break;
        }
        line_start = line_end;
    }

    edits
}

fn trimmed_ascii_whitespace_end(source: &str, start: usize, end: usize) -> usize {
    let bytes = source.as_bytes();
    let mut trimmed = end;
    while trimmed > start && matches!(bytes[trimmed - 1], b' ' | b'\t') {
        trimmed -= 1;
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    fn project_databases(source: &str) -> (LanguageServiceDatabases, DocumentId) {
        let document_id = DocumentId::from("file:///workspace/scripts/main.vela");
        let config = WorkspaceConfig::workspace([WorkspaceRoot::new("/workspace/scripts")]);
        let files = vec![SourceFileSnapshot::new(document_id.clone(), source)];
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        (databases, document_id)
    }

    fn format_source(source: &str) -> Vec<TextEdit> {
        let (databases, document_id) = project_databases(source);
        databases.document_formatting(&document_id)
    }

    fn range_format_source(source: &str, range: DiagnosticRange) -> Vec<TextEdit> {
        let (databases, document_id) = project_databases(source);
        databases.range_formatting(&document_id, range)
    }

    fn formatting_ir(source: &str) -> FormattingIr {
        let (databases, document_id) = project_databases(source);
        databases
            .formatting_ir(&document_id)
            .expect("formatting IR should be available")
    }

    fn apply_edits(source: &str, edits: &[TextEdit]) -> String {
        if edits.is_empty() {
            return source.to_owned();
        }
        assert_eq!(edits.len(), 1);
        edits[0].new_text().to_owned()
    }

    fn apply_range_edits(source: &str, edits: &[TextEdit]) -> String {
        let line_index = LineIndex::new(source);
        let mut formatted = source.to_owned();
        let mut offset_edits = edits
            .iter()
            .map(|edit| {
                (
                    line_index.offset(edit.range().start()),
                    line_index.offset(edit.range().end()),
                    edit.new_text().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        offset_edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
        for (start, end, replacement) in offset_edits {
            formatted.replace_range(start..end, &replacement);
        }
        formatted
    }

    #[test]
    fn formatting_preserves_comments() {
        let source = "// keep this comment   \npub fn main() { // inline\t\n    return 1   \n}";
        let edits = format_source(source);
        let formatted = apply_edits(source, &edits);

        assert_eq!(
            formatted,
            "// keep this comment\npub fn main() {\n    // inline\n    return 1\n}\n"
        );
    }

    #[test]
    fn formatting_is_idempotent() {
        let source = "pub fn main() {\n    return 1\n}\n";
        let edits = format_source(source);

        assert!(edits.is_empty());
    }

    #[test]
    fn formatting_handles_malformed_source_without_panic() {
        let source = "pub fn main( {   ";
        let edits = format_source(source);
        let formatted = apply_edits(source, &edits);

        assert_eq!(formatted, "pub fn main( {\n");
    }

    #[test]
    fn range_formatting_limits_edits_to_range() {
        let source = "pub fn main() {   \n    return 1   \n}\n";
        let edits = range_format_source(
            source,
            DiagnosticRange::new(Position::new(1, 0), Position::new(2, 0)),
        );
        let formatted = apply_range_edits(source, &edits);

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range().start(), Position::new(1, 12));
        assert_eq!(edits[0].range().end(), Position::new(1, 15));
        assert_eq!(formatted, "pub fn main() {   \n    return 1\n}\n");
    }

    #[test]
    fn formatting_ir_preserves_comments_and_blank_line_groups() {
        let source = "#!/usr/bin/env vela\n\npub fn main() {\n    /* keep\n\n       grouped */\n    // tail\n    return 1\n}\n";
        let ir = formatting_ir(source);
        let comment_texts = ir
            .segments()
            .iter()
            .filter(|segment| {
                matches!(
                    segment.kind(),
                    FormattingSegmentKind::LineComment | FormattingSegmentKind::BlockComment
                )
            })
            .map(FormattingSegment::text)
            .collect::<Vec<_>>();
        let preserves_blank_line_group = ir.segments().iter().any(|segment| {
            segment.kind() == FormattingSegmentKind::Whitespace
                && segment.text().matches('\n').count() >= 2
        });

        assert_eq!(
            ir.document_id().as_str(),
            "file:///workspace/scripts/main.vela"
        );
        assert_eq!(ir.reconstruct_source(), source);
        assert_eq!(
            comment_texts,
            vec!["/* keep\n\n       grouped */", "// tail"]
        );
        assert!(preserves_blank_line_group);
        assert_eq!(ir.segments()[0].kind(), FormattingSegmentKind::Shebang);
        assert_eq!(ir.segments()[0].range().start(), Position::new(0, 0));
    }
}
