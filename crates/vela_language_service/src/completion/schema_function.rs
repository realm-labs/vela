use vela_analysis::{
    completion::{
        CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
    },
    registry::RegistryFacts,
};

use crate::TextRange;

use super::{
    CompletionItem, analysis_item::dedupe_and_filter_analysis_items, label_segment_matches,
};

pub(super) fn schema_function_completion_items(
    schema: &RegistryFacts,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    dedupe_and_filter_analysis_items(
        schema
            .functions()
            .map(|function| AnalysisCompletionItem {
                label: function.name,
                kind: AnalysisCompletionKind::Function,
                fact: function.fact,
            })
            .collect(),
        replace_range,
        prefix,
        Some(schema),
        |item| label_segment_matches(&item.label, prefix),
    )
}
