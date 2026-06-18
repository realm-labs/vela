use vela_analysis::{
    completion::{CompletionKind as AnalysisCompletionKind, type_completions},
    registry::RegistryFacts,
};

use crate::{TextRange, symbol_ref::schema_symbol};

use super::{
    CompletionItem, accumulator::CompletionAccumulator, label_segment_matches,
    type_display::type_completion_item,
};

pub(super) fn schema_type_completion_items(
    schema: &RegistryFacts,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut accumulator = CompletionAccumulator::new(replace_range, prefix);
    for item in type_completions(schema) {
        if !matches!(
            item.kind,
            AnalysisCompletionKind::Type | AnalysisCompletionKind::Trait
        ) || !label_segment_matches(&item.label, prefix)
        {
            continue;
        }
        let qualified_name = item.label.clone();
        accumulator.add(
            type_completion_item(item, &qualified_name, prefix)
                .with_symbol(schema_symbol(qualified_name)),
        );
    }
    accumulator.into_items()
}
