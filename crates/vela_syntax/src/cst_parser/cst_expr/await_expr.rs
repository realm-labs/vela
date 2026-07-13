use super::{CstParser, SyntaxKind};

impl CstParser<'_, '_> {
    pub(super) fn previous_significant_before(&self, start: usize, end: usize) -> Option<usize> {
        (start..end)
            .rev()
            .find(|cursor| !self.tokens[*cursor].kind.is_trivia())
    }

    pub(super) fn await_expression_body(&mut self, start: usize, end: usize) {
        let Some(dot) = self.trailing_await_suffix_start(start, end) else {
            self.emit_until(end);
            return;
        };
        self.expression_range(start, dot);
        self.emit_until(end);
    }

    pub(super) fn trailing_await_suffix_start(&self, start: usize, end: usize) -> Option<usize> {
        let await_kw = self.previous_significant_before(start, end)?;
        if self.kind_at(await_kw) != Some(SyntaxKind::AwaitKw) {
            return None;
        }
        let dot = self.previous_significant_before(start, await_kw)?;
        (self.kind_at(dot) == Some(SyntaxKind::Dot)
            && self.previous_significant_before(start, dot).is_some())
        .then_some(dot)
    }
}
