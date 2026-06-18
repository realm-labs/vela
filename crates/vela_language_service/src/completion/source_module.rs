use vela_analysis::{completion::CompletionItem as AnalysisCompletionItem, type_fact::TypeFact};
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
        graph
            .module_completion_labels()
            .into_iter()
            .map(|label| AnalysisCompletionItem {
                label: label.clone(),
                kind: vela_analysis::completion::CompletionKind::Module,
                fact: TypeFact::module(label),
            })
            .collect(),
        replace_range,
        prefix,
        None,
        |item| label_segment_matches(&item.label, prefix),
    )
}
