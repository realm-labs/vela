use crate::protocol::LspPosition;

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

    fn utf16_offset(&self, position: LspPosition) -> Result<usize, String> {
        let line = usize::try_from(position.line)
            .map_err(|_| "LSP position line is too large".to_owned())?;
        let character = usize::try_from(position.character)
            .map_err(|_| "LSP position character is too large".to_owned())?;
        let (line_start, line_end) = self.line_bounds(line)?;
        utf16_character_offset(&self.text[line_start..line_end], character)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
