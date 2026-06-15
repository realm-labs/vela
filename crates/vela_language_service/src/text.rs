#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}

impl Position {
    #[must_use]
    pub const fn new(line: usize, character: usize) -> Self {
        Self { line, character }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    line_starts: Vec<usize>,
    text_len: usize,
}

impl LineIndex {
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self {
            line_starts,
            text_len: text.len(),
        }
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    #[must_use]
    pub fn position(&self, offset: usize) -> Position {
        let offset = offset.min(self.text_len);
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next_line) => next_line.saturating_sub(1),
        };
        Position::new(line, offset - self.line_starts[line])
    }

    #[must_use]
    pub fn offset(&self, position: Position) -> usize {
        let line = position.line.min(self.line_starts.len().saturating_sub(1));
        let line_start = self.line_starts[line];
        let next_line_start = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text_len);
        line_start
            .saturating_add(position.character)
            .min(next_line_start)
            .min(self.text_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_maps_offsets_and_positions() {
        let index = LineIndex::new("alpha\nbeta\n");

        assert_eq!(index.line_count(), 3);
        assert_eq!(index.position(0), Position::new(0, 0));
        assert_eq!(index.position(6), Position::new(1, 0));
        assert_eq!(index.position(10), Position::new(1, 4));
        assert_eq!(index.position(11), Position::new(2, 0));
        assert_eq!(index.position(usize::MAX), Position::new(2, 0));

        assert_eq!(index.offset(Position::new(0, 2)), 2);
        assert_eq!(index.offset(Position::new(1, 2)), 8);
        assert_eq!(index.offset(Position::new(9, 9)), 11);
    }
}
