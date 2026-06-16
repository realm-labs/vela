use crate::{DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextEdit};
use vela_common::SourceId;
use vela_syntax::ast::{EnumVariantFields, ItemKind, SourceFile};
use vela_syntax::formatting::{
    FormatElementKind, TriviaKind, extract_format_elements, format_source,
};
use vela_syntax::token::{Symbol, TokenKind};

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

        if let Some(parsed) = self.parse_db().parsed_source(document_id)
            && let Some(edit) =
                selected_item_formatting_edit(source.source_id(), source.text(), parsed, range)
        {
            return vec![edit];
        }

        trailing_whitespace_edits(source.text(), range)
    }

    #[must_use]
    pub fn on_type_formatting(
        &self,
        document_id: &DocumentId,
        position: Position,
        trigger: &str,
    ) -> Vec<TextEdit> {
        if !matches!(trigger, "}" | "\n") {
            return Vec::new();
        }
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let line_index = LineIndex::new(source.text());
        if trigger == "}"
            && let Some(parsed) = self.parse_db().parsed_source(document_id)
            && let Some(edit) = completed_item_formatting_edit(
                source.source_id(),
                source.text(),
                parsed,
                &line_index,
                position,
            )
        {
            return vec![edit];
        }
        let range = current_construct_range(
            source.source_id(),
            source.text(),
            &line_index,
            position,
            trigger,
        )
        .unwrap_or_else(|| current_line_range(source.text(), &line_index, position));

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

fn selected_item_formatting_edit(
    source_id: SourceId,
    source: &str,
    parsed: &SourceFile,
    range: DiagnosticRange,
) -> Option<TextEdit> {
    let line_index = LineIndex::new(source);
    let start = line_index.offset(range.start());
    let end = line_index.offset(range.end());
    let (format_start, format_end) = selected_item_offsets(source, parsed, start, end)?;
    let selected = source.get(format_start..format_end)?;
    let formatted = format_selected_range(source_id, source, selected, format_start, format_end);
    (formatted != selected).then(|| {
        TextEdit::new(
            DiagnosticRange::new(
                line_index.position(format_start),
                line_index.position(format_end),
            ),
            formatted,
        )
    })
}

fn format_selected_range(
    source_id: SourceId,
    source: &str,
    selected: &str,
    format_start: usize,
    format_end: usize,
) -> String {
    let mut formatted = format_document(source_id, selected);
    let Some(indent) = line_indent_before(source, format_start) else {
        return formatted;
    };
    if indent.is_empty() {
        return formatted;
    }

    if source
        .get(format_end..)
        .is_some_and(|suffix| suffix.starts_with('\n') || suffix.starts_with("\r\n"))
        && formatted.ends_with('\n')
    {
        formatted.pop();
    }

    indent_continuation_lines(&formatted, indent)
}

fn line_indent_before(source: &str, offset: usize) -> Option<&str> {
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let indent = source.get(line_start..offset)?;
    indent
        .chars()
        .all(|ch| matches!(ch, ' ' | '\t'))
        .then_some(indent)
}

fn indent_continuation_lines(formatted: &str, indent: &str) -> String {
    let mut lines = formatted.split('\n');
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut indented = first.to_owned();
    for line in lines {
        indented.push('\n');
        if !line.is_empty() {
            indented.push_str(indent);
        }
        indented.push_str(line);
    }
    indented
}

fn selected_item_offsets(
    source: &str,
    parsed: &SourceFile,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    if start >= end {
        return None;
    }

    let ranges = selectable_format_ranges(parsed);
    let range = ranges.iter().find(|range| {
        range.start >= start
            && range.start < end
            && source
                .get(start..range.start)
                .is_some_and(|prefix| prefix.chars().all(char::is_whitespace))
    })?;
    if range.end > end || !source.get(range.end..end)?.chars().all(char::is_whitespace) {
        return None;
    }

    Some((range.start, end))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct SelectableFormatRange {
    start: usize,
    end: usize,
}

fn selectable_format_ranges(parsed: &SourceFile) -> Vec<SelectableFormatRange> {
    let mut ranges = Vec::new();
    for item in &parsed.items {
        ranges.push(SelectableFormatRange {
            start: item.span.start as usize,
            end: item.span.end as usize,
        });
        match &item.kind {
            ItemKind::Trait(trait_item) => {
                ranges.extend(
                    trait_item
                        .methods
                        .iter()
                        .map(|method| SelectableFormatRange {
                            start: method.span.start as usize,
                            end: method.span.end as usize,
                        }),
                );
            }
            ItemKind::Impl(impl_item) => {
                ranges.extend(
                    impl_item
                        .methods
                        .iter()
                        .map(|method| SelectableFormatRange {
                            start: method.span.start as usize,
                            end: method.span.end as usize,
                        }),
                );
            }
            ItemKind::Struct(struct_item) => {
                ranges.extend(
                    struct_item
                        .fields
                        .iter()
                        .map(|field| SelectableFormatRange {
                            start: field.span.start as usize,
                            end: field.span.end as usize,
                        }),
                );
            }
            ItemKind::Enum(enum_item) => {
                for variant in &enum_item.variants {
                    ranges.push(SelectableFormatRange {
                        start: variant.span.start as usize,
                        end: variant.span.end as usize,
                    });
                    if let EnumVariantFields::Record(fields) = &variant.fields {
                        ranges.extend(fields.iter().map(|field| SelectableFormatRange {
                            start: field.span.start as usize,
                            end: field.span.end as usize,
                        }));
                    }
                }
            }
            ItemKind::Use(_) | ItemKind::Const(_) | ItemKind::Global(_) | ItemKind::Function(_) => {
            }
        }
    }
    ranges
}

fn completed_item_formatting_edit(
    source_id: SourceId,
    source: &str,
    parsed: &SourceFile,
    line_index: &LineIndex,
    position: Position,
) -> Option<TextEdit> {
    let offset = line_index.offset(position).min(source.len());
    let item = parsed.items.iter().find(|item| {
        let start = item.span.start as usize;
        let end = item.span.end as usize;
        start < offset && offset <= end
    })?;
    let start = item.span.start as usize;
    let item_end = item.span.end as usize;
    if line_index.position(start).line != line_index.position(item_end).line {
        return None;
    }
    let end = include_single_trailing_newline(source, item_end);
    let range = DiagnosticRange::new(line_index.position(start), line_index.position(end));
    selected_item_formatting_edit(source_id, source, parsed, range)
}

fn include_single_trailing_newline(source: &str, offset: usize) -> usize {
    match source.get(offset..) {
        Some(suffix) if suffix.starts_with("\r\n") => offset + 2,
        Some(suffix) if suffix.starts_with('\n') => offset + 1,
        _ => offset,
    }
}

fn current_construct_range(
    source_id: SourceId,
    source: &str,
    line_index: &LineIndex,
    position: Position,
    trigger: &str,
) -> Option<DiagnosticRange> {
    let offset = line_index.offset(position).min(source.len());
    let stream = extract_format_elements(source_id, source);
    let tokens = stream
        .elements()
        .iter()
        .filter_map(|element| match element.kind() {
            FormatElementKind::Token(TokenKind::Symbol(symbol)) => Some((*symbol, element.span())),
            _ => None,
        })
        .collect::<Vec<_>>();

    if trigger == "}"
        && let Some(index) = tokens
            .iter()
            .rposition(|(symbol, span)| *symbol == Symbol::RBrace && (span.end as usize) <= offset)
    {
        return matching_brace_range(&tokens, index, line_index);
    }

    let mut stack = Vec::new();
    for (index, (symbol, span)) in tokens.iter().enumerate() {
        if (span.start as usize) > offset {
            break;
        }
        match symbol {
            Symbol::LBrace => stack.push(index),
            Symbol::RBrace => {
                stack.pop();
            }
            _ => {}
        }
    }

    let open_index = stack.pop()?;
    let end = matching_close_offset(&tokens, open_index).unwrap_or(offset);
    Some(DiagnosticRange::new(
        line_index.position(tokens[open_index].1.start as usize),
        line_index.position(end),
    ))
}

fn matching_brace_range(
    tokens: &[(Symbol, vela_common::Span)],
    close_index: usize,
    line_index: &LineIndex,
) -> Option<DiagnosticRange> {
    let mut depth = 0_usize;
    for index in (0..=close_index).rev() {
        match tokens[index].0 {
            Symbol::RBrace => depth = depth.saturating_add(1),
            Symbol::LBrace => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(DiagnosticRange::new(
                        line_index.position(tokens[index].1.start as usize),
                        line_index.position(tokens[close_index].1.end as usize),
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

fn matching_close_offset(
    tokens: &[(Symbol, vela_common::Span)],
    open_index: usize,
) -> Option<usize> {
    let mut depth = 0_usize;
    for (symbol, span) in tokens.iter().skip(open_index) {
        match symbol {
            Symbol::LBrace => depth = depth.saturating_add(1),
            Symbol::RBrace => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(span.end as usize);
                }
            }
            _ => {}
        }
    }
    None
}

fn current_line_range(source: &str, line_index: &LineIndex, position: Position) -> DiagnosticRange {
    let offset = line_index.offset(position).min(source.len());
    let start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let end = source[offset..]
        .find('\n')
        .map_or(source.len(), |index| offset + index + 1);
    DiagnosticRange::new(line_index.position(start), line_index.position(end))
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

    fn on_type_format_source(source: &str, position: Position, trigger: &str) -> Vec<TextEdit> {
        let (databases, document_id) = project_databases(source);
        databases.on_type_formatting(&document_id, position, trigger)
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
    fn formatting_formats_item_declarations() {
        let source = "pub struct Player{level:i64 name:String}impl Player{fn heal(amount:i64)->i64{return amount}}";
        let edits = format_source(source);
        let formatted = apply_edits(source, &edits);

        assert_eq!(
            formatted,
            "\
pub struct Player {
    level: i64
    name: String
}
impl Player {
    fn heal(amount: i64) -> i64 {
        return amount
    }
}
"
        );
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
    fn range_formatting_formats_selected_item() {
        let source = "pub fn main(){return 1}\n\npub fn other(){return 2}\n";
        let edits = range_format_source(
            source,
            DiagnosticRange::new(Position::new(0, 0), Position::new(1, 0)),
        );
        let formatted = apply_range_edits(source, &edits);

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range().start(), Position::new(0, 0));
        assert_eq!(edits[0].range().end(), Position::new(1, 0));
        assert_eq!(
            formatted,
            "\
pub fn main() {
    return 1
}

pub fn other(){return 2}
"
        );
    }

    #[test]
    fn range_formatting_formats_item_with_leading_blank_selection() {
        let source = "\n\npub fn main(){return 1}\n\npub fn other(){return 2}\n";
        let edits = range_format_source(
            source,
            DiagnosticRange::new(Position::new(0, 0), Position::new(3, 0)),
        );
        let formatted = apply_range_edits(source, &edits);

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range().start(), Position::new(2, 0));
        assert_eq!(edits[0].range().end(), Position::new(3, 0));
        assert_eq!(
            formatted,
            "\n\npub fn main() {\n    return 1\n}\n\npub fn other(){return 2}\n"
        );
    }

    #[test]
    fn range_formatting_formats_selected_impl_method() {
        let source = "impl Player{fn heal(amount:i64)->i64{return amount}fn hurt(amount:i64)->i64{return amount}}\n";
        let edits = range_format_source(
            source,
            DiagnosticRange::new(Position::new(0, 12), Position::new(0, 51)),
        );
        let formatted = apply_range_edits(source, &edits);

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range().start(), Position::new(0, 12));
        assert_eq!(edits[0].range().end(), Position::new(0, 51));
        assert_eq!(
            formatted,
            "impl Player{fn heal(amount: i64) -> i64 {\n    return amount\n}\nfn hurt(amount:i64)->i64{return amount}}\n"
        );
    }

    #[test]
    fn range_formatting_preserves_nested_method_indent() {
        let source = "\
impl Player {
    fn heal(amount:i64)->i64{return amount}
    fn hurt(amount:i64)->i64{return amount}
}
";
        let edits = range_format_source(
            source,
            DiagnosticRange::new(Position::new(1, 4), Position::new(1, 43)),
        );
        let formatted = apply_range_edits(source, &edits);

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range().start(), Position::new(1, 4));
        assert_eq!(edits[0].range().end(), Position::new(1, 43));
        assert_eq!(
            formatted,
            "impl Player {\n    fn heal(amount: i64) -> i64 {\n        return amount\n    }\n    fn hurt(amount:i64)->i64{return amount}\n}\n"
        );
    }

    #[test]
    fn range_formatting_preserves_struct_field_indent() {
        let source = "\
pub struct Player {
    level:i64
    name:String
}
";
        let edits = range_format_source(
            source,
            DiagnosticRange::new(Position::new(1, 4), Position::new(1, 13)),
        );
        let formatted = apply_range_edits(source, &edits);

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range().start(), Position::new(1, 4));
        assert_eq!(edits[0].range().end(), Position::new(1, 13));
        assert_eq!(
            formatted,
            "\
pub struct Player {
    level: i64
    name:String
}
"
        );
    }

    #[test]
    fn on_type_formatting_only_edits_current_construct() {
        let source = "\
pub fn main() {   
    return 1   
}

pub fn other() {   
    return 2   
}
";
        let edits = on_type_format_source(source, Position::new(2, 1), "}");
        let formatted = apply_range_edits(source, &edits);

        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].range().start(), Position::new(0, 15));
        assert_eq!(edits[0].range().end(), Position::new(0, 18));
        assert_eq!(edits[1].range().start(), Position::new(1, 12));
        assert_eq!(edits[1].range().end(), Position::new(1, 15));
        assert_eq!(
            formatted,
            "\
pub fn main() {
    return 1
}

pub fn other() {   
    return 2   
}
"
        );
    }

    #[test]
    fn on_type_formatting_reflows_completed_item() {
        let source = "pub fn main(){return 1}\n\npub fn other(){return 2}\n";
        let edits = on_type_format_source(source, Position::new(0, 23), "}");
        let formatted = apply_range_edits(source, &edits);

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range().start(), Position::new(0, 0));
        assert_eq!(edits[0].range().end(), Position::new(1, 0));
        assert_eq!(
            formatted,
            "\
pub fn main() {
    return 1
}

pub fn other(){return 2}
"
        );
    }

    #[test]
    fn on_type_formatting_ignores_unsupported_trigger() {
        let source = "pub fn main() {   \n    return 1   \n}\n";
        let edits = on_type_format_source(source, Position::new(0, 16), "(");

        assert!(edits.is_empty());
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
