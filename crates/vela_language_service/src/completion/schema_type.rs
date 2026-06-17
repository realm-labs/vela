use vela_analysis::{
    completion::{CompletionKind as AnalysisCompletionKind, type_completions},
    registry::RegistryFacts,
};

use crate::TextRange;

use super::{
    CompletionItem, analysis_item::dedupe_and_filter_analysis_items, label_segment_matches,
};

pub(super) fn schema_type_completion_items(
    schema: &RegistryFacts,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    dedupe_and_filter_analysis_items(
        type_completions(schema),
        replace_range,
        prefix,
        Some(schema),
        |item| {
            matches!(
                item.kind,
                AnalysisCompletionKind::Type | AnalysisCompletionKind::Trait
            ) && label_segment_matches(&item.label, prefix)
        },
    )
}
