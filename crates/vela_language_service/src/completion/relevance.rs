use std::cmp::Ordering;

use super::{CompletionItem, CompletionKind};

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Default)]
pub struct CompletionRelevance {
    pub(super) kind_rank: u16,
    pub(super) match_rank: u8,
}

impl CompletionRelevance {
    #[must_use]
    pub const fn kind_rank(&self) -> u16 {
        self.kind_rank
    }

    #[must_use]
    pub const fn match_rank(&self) -> u8 {
        self.match_rank
    }
}

pub(super) fn completion_relevance(
    kind: CompletionKind,
    label: &str,
    prefix: &str,
) -> CompletionRelevance {
    CompletionRelevance {
        kind_rank: completion_kind_rank(kind),
        match_rank: completion_match_rank(label, prefix),
    }
}

pub(super) fn completion_sort_text(kind: CompletionKind, label: &str, prefix: &str) -> String {
    let relevance = completion_relevance(kind, label, prefix);
    format!(
        "{:04}_{:02}_{}",
        relevance.kind_rank(),
        relevance.match_rank(),
        label
    )
}

pub(super) fn completion_item_order(left: &CompletionItem, right: &CompletionItem) -> Ordering {
    left.sort_text
        .cmp(&right.sort_text)
        .then_with(|| left.relevance().cmp(&right.relevance()))
        .then_with(|| left.label.cmp(&right.label))
        .then_with(|| left.kind.cmp(&right.kind))
}

fn completion_kind_rank(kind: CompletionKind) -> u16 {
    match kind {
        CompletionKind::Parameter => 0,
        CompletionKind::Keyword => 0,
        CompletionKind::Snippet => 0,
        CompletionKind::Binding => 1,
        CompletionKind::Const => 10,
        CompletionKind::Module => 20,
        CompletionKind::Type | CompletionKind::Trait => 30,
        CompletionKind::Function | CompletionKind::Method => 40,
        CompletionKind::Field => 50,
        CompletionKind::Variant => 60,
    }
}

fn completion_match_rank(label: &str, prefix: &str) -> u8 {
    if prefix.is_empty() || label.starts_with(prefix) {
        return 0;
    }
    if label
        .rsplit("::")
        .next()
        .is_some_and(|segment| segment.starts_with(prefix))
    {
        return 1;
    }
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevance_prefers_keywords_and_parameters_before_callables() {
        assert!(
            completion_relevance(CompletionKind::Snippet, "fn", "f")
                < completion_relevance(CompletionKind::Function, "format", "f")
        );
        assert!(
            completion_relevance(CompletionKind::Parameter, "score", "s")
                < completion_relevance(CompletionKind::Binding, "score", "s")
        );
    }

    #[test]
    fn relevance_matches_last_path_segment_before_unmatched_labels() {
        assert!(
            completion_relevance(CompletionKind::Function, "game::grant", "gr")
                < completion_relevance(CompletionKind::Function, "reward", "gr")
        );
    }
}
