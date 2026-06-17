use vela_analysis::{
    completion::{CompletionItem as AnalysisCompletionItem, declaration_completions},
    facts::AnalysisFacts,
};
use vela_hir::module_graph::ModuleGraph;

use crate::{QueryContext, TextRange};

use super::{
    CompletionItem, analysis_item::dedupe_and_filter_analysis_items, label_segment_matches,
};

pub(super) fn source_declaration_completion_items(
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    let current_module = query
        .module_path()
        .map(|module| module.join())
        .unwrap_or_default();
    let facts = AnalysisFacts::from_module_graph(graph);
    dedupe_and_filter_analysis_items(
        relative_current_module_items(declaration_completions(graph, &facts), &current_module),
        replace_range,
        prefix,
        None,
        |item| label_segment_matches(&item.label, prefix),
    )
}

fn relative_current_module_items(
    items: Vec<AnalysisCompletionItem>,
    current_module: &str,
) -> Vec<AnalysisCompletionItem> {
    if current_module.is_empty() {
        return items;
    }
    let prefix = format!("{current_module}::");
    items
        .into_iter()
        .map(|mut item| {
            if let Some(relative_label) = item
                .label
                .strip_prefix(&prefix)
                .filter(|relative| !relative.contains("::"))
            {
                item.label = relative_label.to_owned();
            }
            item
        })
        .collect()
}
