use crate::line_index::LineIndex;
use vela_language_service::SelectionRange;

pub(crate) fn selection_ranges(
    ranges: &[SelectionRange],
    text: &str,
) -> Result<Vec<lsp_types::SelectionRange>, String> {
    let index = LineIndex::new(text);
    ranges
        .iter()
        .map(|range| selection_range(range, &index))
        .collect()
}

fn selection_range(
    selection: &SelectionRange,
    index: &LineIndex<'_>,
) -> Result<lsp_types::SelectionRange, String> {
    let range = selection.range();
    let start = index.lsp_position(range.start())?;
    let end = index.lsp_position(range.end())?;
    if start > end {
        return Err("selection range start must not follow its end".to_owned());
    }
    Ok(lsp_types::SelectionRange {
        range: lsp_types::Range::new(start, end),
        parent: selection
            .parent()
            .map(|parent| selection_range(parent, index))
            .transpose()?
            .map(Box::new),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_language_service::{DiagnosticRange, Position};

    #[test]
    fn selection_projection_uses_utf16_preserves_points_and_rejects_invalid_parent_bounds() {
        let range = |start, end| DiagnosticRange::new(start, end);
        let child = SelectionRange::new(
            range(Position::new(0, 8), Position::new(0, 12)),
            Some(SelectionRange::new(
                range(Position::new(0, 0), Position::new(1, 12)),
                None,
            )),
        );
        let point = SelectionRange::new(range(Position::new(0, 7), Position::new(0, 7)), None);
        for text in ["中😀 name\n界😀 tail", "中😀 name\r\n界😀 tail"] {
            assert_eq!(
                selection_ranges(&[child.clone(), point.clone()], text)
                    .expect("whole chain and point"),
                vec![
                    lsp_types::SelectionRange {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(0, 4),
                            lsp_types::Position::new(0, 8)
                        ),
                        parent: Some(Box::new(lsp_types::SelectionRange {
                            range: lsp_types::Range::new(
                                lsp_types::Position::new(0, 0),
                                lsp_types::Position::new(1, 8)
                            ),
                            parent: None,
                        })),
                    },
                    lsp_types::SelectionRange {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(0, 3),
                            lsp_types::Position::new(0, 3)
                        ),
                        parent: None,
                    }
                ]
            );
            for (start, end) in [
                (Position::new(0, 1), Position::new(0, 12)),
                (Position::new(0, 4), Position::new(0, 12)),
                (Position::new(0, 8), Position::new(0, 30)),
                (Position::new(2, 0), Position::new(2, 0)),
                (Position::new(1, 12), Position::new(0, 8)),
            ] {
                assert!(
                    selection_ranges(&[SelectionRange::new(range(start, end), None)], text)
                        .is_err()
                );
                let parent = SelectionRange::new(range(start, end), None);
                let valid_child = SelectionRange::new(
                    range(Position::new(0, 8), Position::new(0, 12)),
                    Some(parent),
                );
                assert!(
                    selection_ranges(&[valid_child], text).is_err(),
                    "every parent is strictly projected"
                );
            }
        }
        assert!(
            selection_ranges(&[], "")
                .expect("empty position vector")
                .is_empty()
        );
        assert_eq!(
            selection_ranges(
                &[SelectionRange::new(
                    range(Position::new(0, 0), Position::new(0, 0)),
                    None
                )],
                ""
            )
            .expect("missing-source zero point")[0]
                .range,
            lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 0)
            )
        );
    }
}
