use vela_analysis::{
    completion::{
        CompletionItem as AnalysisCompletionItem, declaration_completions, global_completions,
        module_completions,
    },
    facts::AnalysisFacts,
    registry::RegistryFacts,
};
use vela_hir::module_graph::ModuleGraph;

use crate::QueryContext;

use super::{
    CompletionContext, CompletionItem, analysis_item::dedupe_and_filter_analysis_items,
    dedupe_and_filter_service_items, label_segment_matches, local::local_completion_items,
};

pub(super) fn global_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    let current_module = query
        .module_path()
        .map(|module| module.join())
        .unwrap_or_default();
    let facts = AnalysisFacts::from_module_graph(graph);
    let mut items = local_completion_items(graph, query, context);
    items.extend(dedupe_and_filter_analysis_items(
        global_completions(schema),
        context.replace_range(),
        context.prefix(),
        Some(schema),
        |item| label_segment_matches(&item.label, context.prefix()),
    ));
    items.extend(dedupe_and_filter_analysis_items(
        relative_current_module_items(
            declaration_completions(graph, &facts),
            current_module.as_str(),
        ),
        context.replace_range(),
        context.prefix(),
        None,
        |item| label_segment_matches(&item.label, context.prefix()),
    ));
    items.extend(dedupe_and_filter_analysis_items(
        module_completions(graph),
        context.replace_range(),
        context.prefix(),
        None,
        |item| label_segment_matches(&item.label, context.prefix()),
    ));
    dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |item| {
        label_segment_matches(&item.label, context.prefix())
    })
}

pub(super) fn expression_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    global_completion_items(graph, schema, query, context)
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
