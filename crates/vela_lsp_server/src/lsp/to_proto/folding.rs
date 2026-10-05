use crate::line_index::LineIndex;
use vela_language_service::{FoldingRange, FoldingRangeKind};

pub(crate) fn folding_ranges(
    ranges: &[FoldingRange],
    text: &str,
) -> Result<Vec<lsp_types::FoldingRange>, String> {
    let index = LineIndex::new(text);
    ranges
        .iter()
        .map(|range| {
            let start = index.lsp_position(range.start())?;
            let end = index.lsp_position(range.end())?;
            if start >= end {
                return Err("folding range start must precede its end".to_owned());
            }
            Ok(lsp_types::FoldingRange {
                start_line: start.line,
                start_character: Some(start.character),
                end_line: end.line,
                end_character: Some(end.character),
                kind: Some(match range.kind() {
                    FoldingRangeKind::Imports => lsp_types::FoldingRangeKind::Imports,
                    FoldingRangeKind::Region => lsp_types::FoldingRangeKind::Region,
                }),
                collapsed_text: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_language_service::Position;
    #[test]
    fn folding_projection_uses_utf16_and_rejects_split_outside_or_reversed_byte_positions() {
        let text = "中😀 start\n界😀 end";
        let range = |start, end| FoldingRange::new(FoldingRangeKind::Imports, start, end);
        let actual = folding_ranges(&[range(Position::new(0, 7), Position::new(1, 11))], text)
            .expect("whole Unicode points");
        assert_eq!(
            actual,
            vec![lsp_types::FoldingRange {
                start_line: 0,
                start_character: Some(3),
                end_line: 1,
                end_character: Some(7),
                kind: Some(lsp_types::FoldingRangeKind::Imports),
                collapsed_text: None,
            }]
        );
        for (start, end) in [
            (Position::new(0, 1), Position::new(1, 11)),
            (Position::new(0, 4), Position::new(1, 11)),
            (Position::new(0, 30), Position::new(1, 11)),
            (Position::new(2, 0), Position::new(2, 1)),
            (Position::new(1, 11), Position::new(0, 7)),
            (Position::new(0, 7), Position::new(0, 7)),
        ] {
            assert!(folding_ranges(&[range(start, end)], text).is_err());
        }
        assert!(
            folding_ranges(&[], "")
                .expect("missing source has empty folds")
                .is_empty()
        );
    }
}
