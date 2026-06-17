use vela_analysis::completion::module_completions;
use vela_hir::module_graph::ModuleGraph;

use crate::TextRange;

use super::{
    CompletionItem, analysis_item::dedupe_and_filter_analysis_items, label_segment_matches,
};

pub(super) fn source_module_completion_items(
    graph: &ModuleGraph,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    dedupe_and_filter_analysis_items(
        module_completions(graph),
        replace_range,
        prefix,
        None,
        |item| label_segment_matches(&item.label, prefix),
    )
}
