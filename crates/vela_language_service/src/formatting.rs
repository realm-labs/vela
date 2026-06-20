use crate::{DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextEdit};
use vela_common::SourceId;
use vela_syntax::ast::{
    AstNode, SyntaxEnumItem, SyntaxImplItem, SyntaxItem, SyntaxSourceFile, SyntaxStructItem,
    SyntaxTraitItem,
};
use vela_syntax::formatting::{
    FormatElementKind, TriviaKind, extract_format_elements, format_source,
};
use vela_syntax::token::{Symbol, TokenKind};
use vela_syntax::{
    Parse as SyntaxParse, SyntaxKind, SyntaxNode, TextRange as SyntaxTextRange, TextSize,
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

        if let Some(parsed) = self.parse_db().syntax_parse(document_id)
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
            && let Some(parsed) = self.parse_db().syntax_parse(document_id)
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
    parsed: &SyntaxParse<SyntaxSourceFile>,
    range: DiagnosticRange,
) -> Option<TextEdit> {
    let line_index = LineIndex::new(source);
    let start = line_index.offset(range.start());
    let end = line_index.offset(range.end());
    let selection = selected_item_offsets(source, parsed, start, end)?;
    let selected = source.get(selection.start..selection.end)?;
    let mut formatted = if selection.members.len() > 1 {
        format_selected_members(source_id, source, &selection)
    } else {
        format_selected_range(source_id, source, selected, selection.start, selection.end)
    };
    trim_nested_member_newline_before_inline_gap(source, &selection, &mut formatted);
    (formatted != selected).then(|| {
        TextEdit::new(
            DiagnosticRange::new(
                line_index.position(selection.start),
                line_index.position(selection.end),
            ),
            formatted,
        )
    })
}

fn trim_nested_member_newline_before_inline_gap(
    source: &str,
    selection: &SelectedFormatRange,
    formatted: &mut String,
) {
    if !matches!(
        selection.members.as_slice(),
        [SelectableFormatRange {
            kind: SelectableFormatRangeKind::NestedMember,
            ..
        }]
    ) {
        return;
    }
    if source
        .get(selection.start..selection.end)
        .is_some_and(|selected| !selected.ends_with('\n'))
        && source
            .get(selection.end..)
            .is_some_and(|suffix| suffix.starts_with(' ') || suffix.starts_with('\t'))
        && formatted.ends_with('\n')
    {
        formatted.pop();
    }
}

fn format_selected_members(
    source_id: SourceId,
    source: &str,
    selection: &SelectedFormatRange,
) -> String {
    let mut formatted = String::new();
    let mut cursor = selection.start;
    for member in &selection.members {
        if let Some(gap) = source.get(cursor..member.start) {
            formatted.push_str(gap);
        }
        if let Some(selected) = source.get(member.start..member.end) {
            formatted.push_str(&format_selected_range(
                source_id,
                source,
                selected,
                member.start,
                member.end,
            ));
        }
        cursor = member.end;
    }
    if let Some(suffix) = source.get(cursor..selection.end) {
        formatted.push_str(suffix);
    }
    formatted
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
    parsed: &SyntaxParse<SyntaxSourceFile>,
    start: usize,
    end: usize,
) -> Option<SelectedFormatRange> {
    if start >= end {
        return None;
    }

    let mut ranges = selectable_format_ranges(parsed);
    ranges.sort_by_key(|range| (range.start, range.end));
    let (range_index, range) = ranges.iter().enumerate().find(|(_, range)| {
        range.start >= start
            && range.start < end
            && source
                .get(start..range.start)
                .is_some_and(|prefix| prefix.chars().all(char::is_whitespace))
    })?;
    if range.end > end {
        return None;
    }

    if range.kind == SelectableFormatRangeKind::EnumRecordVariant
        && source.get(range.end..end)?.chars().all(char::is_whitespace)
    {
        let variant_group = SelectableFormatGroup {
            start: range.start,
            end: range.end,
        };
        let members = ranges
            .iter()
            .copied()
            .filter(|member| member.group == Some(variant_group))
            .collect::<Vec<_>>();
        if !members.is_empty() {
            return Some(SelectedFormatRange {
                start: range.start,
                end,
                members,
            });
        }
    }

    let mut members = vec![*range];
    let mut cursor = range.end;
    while !source.get(cursor..end)?.chars().all(char::is_whitespace) {
        let group = range.group?;
        let next = ranges[range_index + 1..].iter().find(|next| {
            next.group == Some(group)
                && next.start >= cursor
                && next.start < end
                && source
                    .get(cursor..next.start)
                    .is_some_and(|gap| gap.chars().all(char::is_whitespace))
        })?;
        if next.end > end {
            return None;
        }
        members.push(*next);
        cursor = next.end;
    }

    Some(SelectedFormatRange {
        start: range.start,
        end,
        members,
    })
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct SelectableFormatRange {
    start: usize,
    end: usize,
    group: Option<SelectableFormatGroup>,
    kind: SelectableFormatRangeKind,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SelectedFormatRange {
    start: usize,
    end: usize,
    members: Vec<SelectableFormatRange>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct SelectableFormatGroup {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SelectableFormatRangeKind {
    Item,
    NestedMember,
    EnumRecordVariant,
}

fn selectable_format_ranges(parsed: &SyntaxParse<SyntaxSourceFile>) -> Vec<SelectableFormatRange> {
    let mut ranges = Vec::new();
    let source = parsed.tree();
    for item in source.items() {
        let item_span = format_group(item.syntax().text_range());
        ranges.push(SelectableFormatRange {
            start: item_span.start,
            end: item_span.end,
            group: None,
            kind: SelectableFormatRangeKind::Item,
        });
        match item.syntax().kind() {
            SyntaxKind::TraitItem => collect_trait_format_ranges(&item, item_span, &mut ranges),
            SyntaxKind::ImplItem => collect_impl_format_ranges(&item, item_span, &mut ranges),
            SyntaxKind::StructItem => collect_struct_format_ranges(&item, item_span, &mut ranges),
            SyntaxKind::EnumItem => collect_enum_format_ranges(&item, item_span, &mut ranges),
            SyntaxKind::UseItem
            | SyntaxKind::ConstItem
            | SyntaxKind::GlobalItem
            | SyntaxKind::FunctionItem => {}
            kind => unreachable!("non-item syntax kind: {kind:?}"),
        }
    }
    ranges
}

fn collect_trait_format_ranges(
    item: &SyntaxItem,
    item_span: SelectableFormatGroup,
    ranges: &mut Vec<SelectableFormatRange>,
) {
    let Some(trait_item) = SyntaxTraitItem::cast(item.syntax().clone()) else {
        return;
    };
    for method in trait_item.methods() {
        collect_method_format_ranges(method.syntax(), item_span, ranges);
    }
}

fn collect_impl_format_ranges(
    item: &SyntaxItem,
    item_span: SelectableFormatGroup,
    ranges: &mut Vec<SelectableFormatRange>,
) {
    let Some(impl_item) = SyntaxImplItem::cast(item.syntax().clone()) else {
        return;
    };
    for method in impl_item.methods() {
        collect_method_format_ranges(method.syntax(), item_span, ranges);
    }
}

fn collect_struct_format_ranges(
    item: &SyntaxItem,
    item_span: SelectableFormatGroup,
    ranges: &mut Vec<SelectableFormatRange>,
) {
    let Some(struct_item) = SyntaxStructItem::cast(item.syntax().clone()) else {
        return;
    };
    let Some(field_list) = struct_item.field_list() else {
        return;
    };
    ranges.extend(field_list.fields().map(|field| {
        let span = format_group(field.syntax().text_range());
        SelectableFormatRange {
            start: span.start,
            end: span.end,
            group: Some(item_span),
            kind: SelectableFormatRangeKind::NestedMember,
        }
    }));
}

fn collect_enum_format_ranges(
    item: &SyntaxItem,
    item_span: SelectableFormatGroup,
    ranges: &mut Vec<SelectableFormatRange>,
) {
    let Some(enum_item) = SyntaxEnumItem::cast(item.syntax().clone()) else {
        return;
    };
    let Some(variant_list) = enum_item.variant_list() else {
        return;
    };
    for variant in variant_list.variants() {
        let variant_span = format_group(variant.syntax().text_range());
        let record_field_list = variant.record_field_list();
        ranges.push(SelectableFormatRange {
            start: variant_span.start,
            end: variant_span.end,
            group: Some(item_span),
            kind: if record_field_list.is_some() {
                SelectableFormatRangeKind::EnumRecordVariant
            } else {
                SelectableFormatRangeKind::NestedMember
            },
        });
        if let Some(fields) = record_field_list {
            ranges.extend(fields.fields().map(|field| {
                let span = format_group(field.syntax().text_range());
                SelectableFormatRange {
                    start: span.start,
                    end: span.end,
                    group: Some(variant_span),
                    kind: SelectableFormatRangeKind::NestedMember,
                }
            }));
        }
    }
}

fn collect_method_format_ranges(
    method: &SyntaxNode,
    item_span: SelectableFormatGroup,
    ranges: &mut Vec<SelectableFormatRange>,
) {
    let tokens = method
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect::<Vec<_>>();
    let fn_token_indexes = tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| (token.kind() == SyntaxKind::FnKw).then_some(index))
        .collect::<Vec<_>>();
    if fn_token_indexes.len() <= 1 {
        let span = format_group(method.text_range());
        ranges.push(SelectableFormatRange {
            start: span.start,
            end: span.end,
            group: Some(item_span),
            kind: SelectableFormatRangeKind::NestedMember,
        });
        return;
    }

    for (position, start_index) in fn_token_indexes.iter().copied().enumerate() {
        let next_fn_index = fn_token_indexes.get(position + 1).copied();
        let end_index = next_fn_index
            .map(|index| index.saturating_sub(1))
            .unwrap_or_else(|| tokens.len().saturating_sub(1));
        let start = text_size_to_usize(tokens[start_index].text_range().start());
        let end = text_size_to_usize(tokens[end_index].text_range().end());
        ranges.push(SelectableFormatRange {
            start,
            end,
            group: Some(item_span),
            kind: SelectableFormatRangeKind::NestedMember,
        });
    }
}

fn format_group(range: SyntaxTextRange) -> SelectableFormatGroup {
    SelectableFormatGroup {
        start: text_size_to_usize(range.start()),
        end: text_size_to_usize(range.end()),
    }
}

fn text_size_to_usize(size: TextSize) -> usize {
    u32::from(size) as usize
}

fn completed_item_formatting_edit(
    source_id: SourceId,
    source: &str,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    line_index: &LineIndex,
    position: Position,
) -> Option<TextEdit> {
    let offset = line_index.offset(position).min(source.len());
    let selected = selectable_format_ranges(parsed)
        .into_iter()
        .filter(|range| range.start < offset && offset <= range.end)
        .min_by_key(|range| range.end.saturating_sub(range.start))?;
    let end = include_single_trailing_newline(source, selected.end);
    let range = DiagnosticRange::new(
        line_index.position(selected.start),
        line_index.position(end),
    );
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
mod tests;
