use vela_analysis::{
    completion::{
        CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
    },
    stdlib::stdlib_function_completion_facts,
    type_fact::TypeFact,
};

use crate::TextRange;

use super::{
    CompletionItem, analysis_item::dedupe_and_filter_analysis_items, label_segment_matches,
};

pub(super) fn stdlib_function_completion_items(
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    dedupe_and_filter_analysis_items(
        stdlib_function_completion_facts()
            .into_iter()
            .map(|function| AnalysisCompletionItem {
                label: function.name.to_owned(),
                kind: AnalysisCompletionKind::Function,
                fact: TypeFact::function(function.params, function.returns),
            })
            .collect(),
        replace_range,
        prefix,
        None,
        |item| label_segment_matches(&item.label, prefix),
    )
}
