use std::collections::BTreeMap;

use crate::TextRange;

use super::{CompletionItem, CompletionKind};

#[derive(Debug)]
pub(super) struct CompletionAccumulator {
    replace_range: TextRange,
    prefix: String,
    items: BTreeMap<CompletionIdentity, CompletionItem>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct CompletionIdentity {
    lookup: String,
    replace_start: usize,
    replace_end: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct CompletionRelevance {
    kind_rank: u16,
    match_rank: u8,
}

impl CompletionAccumulator {
    pub(super) fn new(replace_range: TextRange, prefix: &str) -> Self {
        Self {
            replace_range,
            prefix: prefix.to_owned(),
            items: BTreeMap::new(),
        }
    }

    pub(super) fn add(&mut self, item: CompletionItem) {
        let identity = CompletionIdentity {
            lookup: item.lookup_identity(),
            replace_start: self.replace_range.start,
            replace_end: self.replace_range.end,
        };
        self.items
            .entry(identity)
            .and_modify(|existing| {
                if completion_item_order(&item, existing, &self.prefix).is_lt() {
                    *existing = item.clone();
                }
            })
            .or_insert(item);
    }

    pub(super) fn add_many(&mut self, items: impl IntoIterator<Item = CompletionItem>) {
        for item in items {
            self.add(item);
        }
    }

    pub(super) fn add_many_matching(
        &mut self,
        items: impl IntoIterator<Item = CompletionItem>,
        matches_context: impl Fn(&CompletionItem) -> bool,
    ) {
        for item in items.into_iter().filter(matches_context) {
            self.add(item);
        }
    }

    pub(super) fn into_items(self) -> Vec<CompletionItem> {
        let mut items = self.items.into_values().collect::<Vec<_>>();
        items.sort_by(|left, right| completion_item_order(left, right, &self.prefix));
        items
    }
}

impl CompletionItem {
    fn lookup_identity(&self) -> String {
        self.label.clone()
    }
}

fn completion_item_order(
    left: &CompletionItem,
    right: &CompletionItem,
    prefix: &str,
) -> std::cmp::Ordering {
    left.sort_text
        .cmp(&right.sort_text)
        .then_with(|| {
            CompletionRelevance::for_item(left, prefix)
                .cmp(&CompletionRelevance::for_item(right, prefix))
        })
        .then_with(|| left.label.cmp(&right.label))
        .then_with(|| left.kind.cmp(&right.kind))
}

impl CompletionRelevance {
    fn for_item(item: &CompletionItem, prefix: &str) -> Self {
        Self {
            kind_rank: completion_kind_rank(item.kind),
            match_rank: completion_match_rank(&item.label, prefix),
        }
    }
}

impl Ord for CompletionRelevance {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.kind_rank
            .cmp(&other.kind_rank)
            .then_with(|| self.match_rank.cmp(&other.match_rank))
    }
}

impl PartialOrd for CompletionRelevance {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn completion_kind_rank(kind: CompletionKind) -> u16 {
    match kind {
        CompletionKind::Parameter => 0,
        CompletionKind::Keyword => 0,
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
    use crate::TextRange;

    use super::*;
    use crate::completion::{CompletionInsertFormat, CompletionItem, CompletionKind};

    #[test]
    fn accumulator_dedupes_by_lookup_and_replace_range() {
        let mut accumulator = CompletionAccumulator::new(TextRange::new(4, 6), "le");
        accumulator.add(item("level", CompletionKind::Field, None));
        accumulator.add(item("level", CompletionKind::Field, None));

        let items = accumulator.into_items();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label(), "level");
    }

    #[test]
    fn accumulator_keeps_most_relevant_duplicate() {
        let mut accumulator = CompletionAccumulator::new(TextRange::new(0, 2), "ma");
        accumulator.add(item(
            "map",
            CompletionKind::Function,
            Some("0040_00_map".to_owned()),
        ));
        accumulator.add(item(
            "map",
            CompletionKind::Function,
            Some("0001_00_map".to_owned()),
        ));

        let items = accumulator.into_items();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].sort_text(), Some("0001_00_map"));
    }

    #[test]
    fn accumulator_sorts_by_relevance_without_filtering_prefix() {
        let mut accumulator = CompletionAccumulator::new(TextRange::new(0, 1), "f");
        accumulator.add(item("map", CompletionKind::Function, None));
        accumulator.add(item("fn", CompletionKind::Keyword, None));
        accumulator.add(item("game::foo", CompletionKind::Function, None));

        let labels = accumulator
            .into_items()
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();

        assert_eq!(labels, ["fn", "game::foo", "map"]);
    }

    fn item(label: &str, kind: CompletionKind, sort_text: Option<String>) -> CompletionItem {
        CompletionItem {
            label: label.to_owned(),
            kind,
            detail: String::new(),
            insert_text: None,
            insert_format: CompletionInsertFormat::PlainText,
            sort_text,
        }
    }
}
