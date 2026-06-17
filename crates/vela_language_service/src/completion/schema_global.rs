use vela_analysis::{completion::global_completions, registry::RegistryFacts};

use crate::TextRange;

use super::{
    CompletionItem, analysis_item::dedupe_and_filter_analysis_items, label_segment_matches,
};

pub(super) fn schema_global_completion_items(
    schema: &RegistryFacts,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    dedupe_and_filter_analysis_items(
        global_completions(schema),
        replace_range,
        prefix,
        Some(schema),
        |item| label_segment_matches(&item.label, prefix),
    )
}
