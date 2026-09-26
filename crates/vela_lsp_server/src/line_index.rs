use vela_language_service::{DiagnosticRange, LineIndex as ServiceLineIndex, Position};

use crate::protocol::{LspPosition, LspRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PositionEncoding {
    Utf16,
}

pub(crate) struct LineIndex<'a> {
    text: &'a str,
    encoding: PositionEncoding,
}

impl<'a> LineIndex<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self {
            text,
            encoding: PositionEncoding::Utf16,
        }
    }

    pub(crate) fn offset(&self, position: LspPosition) -> Result<usize, String> {
        match self.encoding {
            PositionEncoding::Utf16 => self.utf16_offset(position),
        }
    }

    pub(crate) fn service_position(&self, position: LspPosition) -> Result<Position, String> {
        let offset = self.offset(position)?;
        Ok(ServiceLineIndex::new(self.text).position(offset))
    }

    pub(crate) fn lsp_position(&self, position: Position) -> Result<lsp_types::Position, String> {
        let (start, end) = self.line_bounds(position.line)?;
        let prefix = self.text[start..end]
            .get(..position.character)
            .ok_or_else(|| {
                "service position is outside the line or splits a character".to_owned()
            })?;
        Ok(lsp_types::Position::new(
            u32::try_from(position.line).map_err(|_| "line is too large".to_owned())?,
            u32::try_from(prefix.encode_utf16().count())
                .map_err(|_| "character is too large".to_owned())?,
        ))
    }

    pub(crate) fn diagnostic_position(
        &self,
        mut position: Position,
    ) -> Result<lsp_types::Position, String> {
        let (start, end) = self.line_bounds(position.line)?;
        // Lexer diagnostics may end after CR but before LF. Normalize this
        // display boundary only; edit positions must remain exact.
        if position.character == end - start + 1
            && self.text.as_bytes().get(end..end + 2) == Some(b"\r\n")
        {
            position.character = end - start;
        }
        self.lsp_position(position)
    }

    pub(crate) fn service_range(&self, range: LspRange) -> Result<DiagnosticRange, String> {
        let start_offset = self.offset_clamped(range.start)?;
        let end_offset = self.offset_clamped(range.end)?;
        if start_offset > end_offset {
            return Err("LSP range start must not be after the end".to_owned());
        }
        let service_index = ServiceLineIndex::new(self.text);
        Ok(DiagnosticRange::new(
            service_index.position(start_offset),
            service_index.position(end_offset),
        ))
    }

    fn offset_clamped(&self, position: LspPosition) -> Result<usize, String> {
        match self.encoding {
            PositionEncoding::Utf16 => self.utf16_offset_clamped(position),
        }
    }

    fn utf16_offset(&self, position: LspPosition) -> Result<usize, String> {
        let line = usize::try_from(position.line)
            .map_err(|_| "LSP position line is too large".to_owned())?;
        let character = usize::try_from(position.character)
            .map_err(|_| "LSP position character is too large".to_owned())?;
        let (line_start, line_end) = self.line_bounds(line)?;
        utf16_character_offset(&self.text[line_start..line_end], character)
            .map(|offset| line_start + offset)
    }

    fn utf16_offset_clamped(&self, position: LspPosition) -> Result<usize, String> {
        let line = usize::try_from(position.line)
            .map_err(|_| "LSP position line is too large".to_owned())?;
        let character = usize::try_from(position.character)
            .map_err(|_| "LSP position character is too large".to_owned())?;
        let (line_start, line_end) = self
            .line_bounds(line)
            .unwrap_or((self.text.len(), self.text.len()));
        utf16_character_offset_clamped(&self.text[line_start..line_end], character)
            .map(|offset| line_start + offset)
    }

    fn line_bounds(&self, target_line: usize) -> Result<(usize, usize), String> {
        let mut line = 0usize;
        let mut line_start = 0usize;
        for (offset, byte) in self.text.bytes().enumerate() {
            if byte != b'\n' {
                continue;
            }
            if line == target_line {
                return Ok((
                    line_start,
                    trim_carriage_return(self.text, line_start, offset),
                ));
            }
            line = line.saturating_add(1);
            line_start = offset + 1;
        }
        if line == target_line {
            Ok((line_start, self.text.len()))
        } else {
            Err("LSP position line is outside the document".to_owned())
        }
    }
}

fn trim_carriage_return(text: &str, line_start: usize, line_end: usize) -> usize {
    if line_end > line_start && text.as_bytes()[line_end - 1] == b'\r' {
        line_end - 1
    } else {
        line_end
    }
}

fn utf16_character_offset(line_text: &str, character: usize) -> Result<usize, String> {
    let mut utf16_units = 0usize;
    for (offset, ch) in line_text.char_indices() {
        if utf16_units == character {
            return Ok(offset);
        }
        let next_units = utf16_units + ch.len_utf16();
        if character < next_units {
            return Err("LSP position splits a UTF-16 character".to_owned());
        }
        utf16_units = next_units;
    }
    if utf16_units == character {
        Ok(line_text.len())
    } else {
        Err("LSP position character is outside the line".to_owned())
    }
}

fn utf16_character_offset_clamped(line_text: &str, character: usize) -> Result<usize, String> {
    let mut utf16_units = 0usize;
    for (offset, ch) in line_text.char_indices() {
        if utf16_units == character {
            return Ok(offset);
        }
        let next_units = utf16_units + ch.len_utf16();
        if character < next_units {
            return Err("LSP position splits a UTF-16 character".to_owned());
        }
        utf16_units = next_units;
    }
    Ok(line_text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_crlf_boundaries_preserve_visible_unicode_line_ends() {
        let text = "中😀 broken\r\nnext";
        let index = LineIndex::new(text);
        let visible_end = "中😀 broken".len();
        for column in [visible_end, visible_end + 1] {
            assert_eq!(
                index
                    .diagnostic_position(Position::new(0, column))
                    .expect("CRLF boundary"),
                lsp_types::Position::new(0, 10)
            );
        }
        for column in [1, 4, visible_end + 2] {
            assert!(index.diagnostic_position(Position::new(0, column)).is_err());
        }
        assert!(
            index
                .lsp_position(Position::new(0, visible_end + 1))
                .is_err()
        );
        assert_eq!(
            index.lsp_position(Position::new(1, 0)).expect("next line"),
            lsp_types::Position::new(1, 0)
        );
        for text in ["中😀 broken\nnext", "中😀 broken\r"] {
            assert!(
                LineIndex::new(text)
                    .diagnostic_position(Position::new(
                        0,
                        text.find('\n').unwrap_or(text.len()) + 1
                    ))
                    .is_err()
            );
        }
    }

    #[test]
    fn utf16_offsets_count_wide_characters_as_two_units() {
        let text = "let icon = \"💎\"\nnext";
        let index = LineIndex::new(text);
        let diamond_start = text.find('💎').expect("diamond should exist");
        let after_diamond = diamond_start + '💎'.len_utf8();

        assert_eq!(
            index
                .offset(LspPosition {
                    line: 0,
                    character: 12
                })
                .expect("position before diamond should resolve"),
            diamond_start
        );
        assert_eq!(
            index
                .offset(LspPosition {
                    line: 0,
                    character: 14
                })
                .expect("position after diamond should resolve"),
            after_diamond
        );
        assert!(
            index
                .offset(LspPosition {
                    line: 0,
                    character: 13
                })
                .is_err(),
            "halfway through a surrogate pair must be rejected"
        );
    }

    #[test]
    fn crlf_line_endings_do_not_expose_carriage_return_columns() {
        let text = "one\r\ntwo";
        let index = LineIndex::new(text);

        assert_eq!(
            index
                .offset(LspPosition {
                    line: 0,
                    character: 3
                })
                .expect("end of first line should resolve"),
            3
        );
        assert!(
            index
                .offset(LspPosition {
                    line: 0,
                    character: 4
                })
                .is_err(),
            "the carriage return is not an addressable LSP column"
        );
        assert_eq!(
            index
                .offset(LspPosition {
                    line: 1,
                    character: 0
                })
                .expect("second line start should resolve"),
            5
        );
    }

    #[test]
    fn service_positions_use_byte_columns_after_utf16_conversion() {
        let text = "let icon = \"💎\"\nnext";
        let index = LineIndex::new(text);

        assert_eq!(
            index
                .service_position(LspPosition {
                    line: 0,
                    character: 14
                })
                .expect("position after diamond should resolve"),
            Position::new(0, "let icon = \"💎".len())
        );
    }

    #[test]
    fn service_ranges_clamp_oversized_editor_ranges() {
        let text = "one\ntwo";
        let index = LineIndex::new(text);

        assert_eq!(
            index
                .service_range(LspRange {
                    start: LspPosition {
                        line: 0,
                        character: 2
                    },
                    end: LspPosition {
                        line: 99,
                        character: 99
                    }
                })
                .expect("oversized range should clamp to document end"),
            DiagnosticRange::new(Position::new(0, 2), Position::new(1, 3))
        );
    }
}
