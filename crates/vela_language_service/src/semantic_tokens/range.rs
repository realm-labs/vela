use crate::{DiagnosticRange, Position};

use super::{SemanticToken, SemanticTokens};

impl SemanticTokens {
    #[must_use]
    pub fn in_range(&self, range: DiagnosticRange) -> Self {
        if !position_before(range.start(), range.end()) {
            return Self::from_source_hash(Vec::new(), self.source_hash);
        }
        let tokens = self
            .tokens()
            .iter()
            .copied()
            .filter(|token| token_overlaps_range(*token, range))
            .collect();
        Self::from_source_hash(tokens, self.source_hash)
    }
}

fn token_overlaps_range(token: SemanticToken, range: DiagnosticRange) -> bool {
    let start = token.start();
    let end = Position::new(start.line, start.character.saturating_add(token.length()));
    position_before(start, range.end()) && position_before(range.start(), end)
}

const fn position_before(left: Position, right: Position) -> bool {
    left.line < right.line || (left.line == right.line && left.character < right.character)
}
