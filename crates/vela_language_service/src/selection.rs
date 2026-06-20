use vela_syntax::{SyntaxNode, SyntaxToken, TextRange as SyntaxTextRange, TextSize, TokenAtOffset};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, SourceRecord,
    TextRange,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SelectionRange {
    range: DiagnosticRange,
    parent: Option<Box<SelectionRange>>,
}

impl SelectionRange {
    #[must_use]
    pub fn new(range: DiagnosticRange, parent: Option<SelectionRange>) -> Self {
        Self {
            range,
            parent: parent.map(Box::new),
        }
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn parent(&self) -> Option<&SelectionRange> {
        self.parent.as_deref()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct SelectionRangeKey {
    start: usize,
    end: usize,
}

impl SelectionRangeKey {
    const fn new(range: TextRange) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn selection_ranges(
        &self,
        document_id: &DocumentId,
        positions: &[Position],
    ) -> Vec<SelectionRange> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return positions.iter().copied().map(point_selection).collect();
        };
        let Some(parsed) = self.parse_db().syntax_parse(document_id) else {
            return positions.iter().copied().map(point_selection).collect();
        };
        let line_index = LineIndex::new(source.text());
        let syntax = parsed.syntax_node();

        positions
            .iter()
            .copied()
            .map(|position| {
                let offset = line_index.offset(position);
                let mut ranges = Vec::new();
                if let Some(token) = significant_token_at(&syntax, offset) {
                    collect_syntax_ancestor_ranges(&token, &mut ranges);
                }
                build_selection_chain(source, &line_index, position, ranges)
            })
            .collect()
    }
}

fn significant_token_at(root: &SyntaxNode, offset: usize) -> Option<SyntaxToken> {
    let offset = syntax_offset(offset)?;
    match root.token_at_offset(offset) {
        TokenAtOffset::None => None,
        TokenAtOffset::Single(token) => non_trivia_token(token),
        TokenAtOffset::Between(left, right) => {
            non_trivia_token(right).or_else(|| non_trivia_token(left))
        }
    }
}

fn non_trivia_token(token: SyntaxToken) -> Option<SyntaxToken> {
    (!token.kind().is_trivia()).then_some(token)
}

fn collect_syntax_ancestor_ranges(token: &SyntaxToken, ranges: &mut Vec<SelectionRangeKey>) {
    push_syntax_range(token.text_range(), ranges);
    if let Some(parent) = token.parent() {
        for node in parent.ancestors() {
            push_syntax_range(node.text_range(), ranges);
        }
    }
}

fn push_syntax_range(range: SyntaxTextRange, ranges: &mut Vec<SelectionRangeKey>) {
    let start = text_size_to_usize(range.start());
    let end = text_size_to_usize(range.end());
    if start < end {
        push_range(TextRange::new(start, end), ranges);
    }
}

fn push_range(range: TextRange, ranges: &mut Vec<SelectionRangeKey>) {
    let key = SelectionRangeKey::new(range);
    if !ranges.contains(&key) {
        ranges.push(key);
    }
}

fn build_selection_chain(
    source: &SourceRecord,
    line_index: &LineIndex,
    position: Position,
    mut ranges: Vec<SelectionRangeKey>,
) -> SelectionRange {
    if ranges.is_empty() {
        return point_selection(position);
    }
    ranges.sort_by_key(|range| (range.len(), std::cmp::Reverse(range.start), range.end));
    ranges.dedup();

    let mut selection = None;
    for range in ranges.into_iter().rev() {
        let diagnostic_range = diagnostic_range(source.text(), line_index, range);
        selection = Some(SelectionRange::new(diagnostic_range, selection));
    }
    selection.unwrap_or_else(|| point_selection(position))
}

fn point_selection(position: Position) -> SelectionRange {
    SelectionRange::new(DiagnosticRange::new(position, position), None)
}

fn diagnostic_range(
    text: &str,
    line_index: &LineIndex,
    range: SelectionRangeKey,
) -> DiagnosticRange {
    let end = range.end.min(text.len());
    DiagnosticRange::new(line_index.position(range.start), line_index.position(end))
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}

fn text_size_to_usize(size: TextSize) -> usize {
    u32::from(size) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn selection_ranges_walk_syntax_ancestors() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main(player: Player) -> i64 {
    let next = player.level + 1
    if next > 1 {
        return next
    }
    return 0
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let position = Position::new(
            1,
            text.lines()
                .nth(1)
                .expect("line should exist")
                .find("level")
                .expect("token should exist"),
        );

        let ranges = databases.selection_ranges(&document, &[position]);

        assert_eq!(ranges.len(), 1);
        let chain = flatten(&ranges[0]);
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 22
                && range.end().line == 1
                && range.end().character == 27),
            "{chain:?}"
        );
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 15
                && range.end().line == 1
                && range.end().character == 27),
            "{chain:?}"
        );
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 15
                && range.end().line == 1
                && range.end().character == 31),
            "{chain:?}"
        );
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 4
                && range.end().line == 1
                && range.end().character == 31),
            "{chain:?}"
        );
        assert!(
            chain
                .iter()
                .any(|range| range.start().line == 0 && range.end().line == 6),
            "{chain:?}"
        );
    }

    #[test]
    fn selection_ranges_preserve_token_and_ancestors_under_parser_recovery() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main(player: Player) -> i64 {
    let next = player.level + 1
    if next > 1 {
        return next
";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let position = Position::new(
            1,
            text.lines()
                .nth(1)
                .expect("line should exist")
                .find("level")
                .expect("token should exist"),
        );

        let ranges = databases.selection_ranges(&document, &[position]);

        assert_eq!(ranges.len(), 1);
        let chain = flatten(&ranges[0]);
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 22
                && range.end().line == 1
                && range.end().character == 27),
            "{chain:?}"
        );
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 15
                && range.end().line == 1
                && range.end().character == 27),
            "{chain:?}"
        );
        assert!(
            chain
                .iter()
                .any(|range| range.start().line == 0 && range.end().line == 4),
            "{chain:?}"
        );
    }

    fn flatten(range: &SelectionRange) -> Vec<DiagnosticRange> {
        let mut ranges = Vec::new();
        let mut current = Some(range);
        while let Some(range) = current {
            ranges.push(range.range());
            current = range.parent();
        }
        ranges
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
