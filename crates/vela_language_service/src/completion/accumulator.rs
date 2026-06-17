use std::collections::BTreeMap;

use crate::TextRange;

use super::{CompletionItem, CompletionKind, CompletionRelevance, CompletionTextEdit};

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

impl CompletionAccumulator {
    pub(super) fn new(replace_range: TextRange, prefix: &str) -> Self {
        Self {
            replace_range,
            prefix: prefix.to_owned(),
            items: BTreeMap::new(),
        }
    }

    pub(super) fn add(&mut self, item: CompletionItem) {
        let item = self.prepare_item(item);
        let identity = CompletionIdentity {
            lookup: item.lookup().to_owned(),
            replace_start: self.replace_range.start,
            replace_end: self.replace_range.end,
        };
        self.items
            .entry(identity)
            .and_modify(|existing| {
                if completion_item_order(&item, existing).is_lt() {
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
        items.sort_by(completion_item_order);
        items
    }

    fn prepare_item(&self, mut item: CompletionItem) -> CompletionItem {
        let lookup = item
            .metadata
            .lookup
            .get_or_insert_with(|| item.label.clone())
            .clone();
        item.metadata
            .filter_text
            .get_or_insert_with(|| lookup.clone());
        item.metadata.source_range.get_or_insert(self.replace_range);
        if item.metadata.text_edit.is_none()
            && let Some(insert_text) = item.insert_text.clone()
        {
            item.metadata.text_edit = Some(CompletionTextEdit {
                range: self.replace_range,
                new_text: insert_text,
            });
        }
        item.metadata
            .label_details
            .detail
            .get_or_insert_with(|| item.detail.clone());
        item.metadata.relevance = CompletionRelevance {
            kind_rank: completion_kind_rank(item.kind),
            match_rank: completion_match_rank(&item.label, &self.prefix),
        };
        item
    }
}

fn completion_item_order(left: &CompletionItem, right: &CompletionItem) -> std::cmp::Ordering {
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
            metadata: Default::default(),
        }
    }
}
