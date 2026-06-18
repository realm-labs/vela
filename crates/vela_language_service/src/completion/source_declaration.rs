use vela_analysis::{
    completion::{
        CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
        declaration_completions,
    },
    facts::AnalysisFacts,
};
use vela_hir::module_graph::ModuleGraph;

use crate::symbol_ref::source_symbol;
use crate::{QueryContext, TextRange};

use super::{
    CompletionItem, accumulator::CompletionAccumulator,
    analysis_item::service_item_from_analysis_completion, label_segment_matches,
};

pub(super) fn source_const_completion_items(
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    source_declaration_completion_items(graph, query, replace_range, prefix, |kind| {
        matches!(kind, AnalysisCompletionKind::Const)
    })
}

pub(super) fn source_function_completion_items(
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    source_declaration_completion_items(graph, query, replace_range, prefix, |kind| {
        matches!(kind, AnalysisCompletionKind::Function)
    })
}

pub(super) fn source_type_completion_items(
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    source_declaration_completion_items(graph, query, replace_range, prefix, |kind| {
        matches!(
            kind,
            AnalysisCompletionKind::Type | AnalysisCompletionKind::Trait
        )
    })
}

fn source_declaration_completion_items(
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
    accepts_kind: impl Fn(AnalysisCompletionKind) -> bool,
) -> Vec<CompletionItem> {
    let current_module = query
        .module_path()
        .map(|module| module.join())
        .unwrap_or_default();
    let facts = AnalysisFacts::from_module_graph(graph);
    let mut accumulator = CompletionAccumulator::new(replace_range, prefix);
    for (item, symbol) in
        relative_current_module_items(declaration_completions(graph, &facts), &current_module)
    {
        if accepts_kind(item.kind) && label_segment_matches(&item.label, prefix) {
            accumulator.add(
                service_item_from_analysis_completion(item, prefix)
                    .with_symbol(source_symbol(symbol)),
            );
        }
    }
    accumulator.into_items()
}

fn relative_current_module_items(
    items: Vec<AnalysisCompletionItem>,
    current_module: &str,
) -> Vec<(AnalysisCompletionItem, String)> {
    if current_module.is_empty() {
        return items
            .into_iter()
            .map(|item| {
                let symbol = item.label.clone();
                (item, symbol)
            })
            .collect();
    }
    let prefix = format!("{current_module}::");
    items
        .into_iter()
        .map(|mut item| {
            let symbol = item.label.clone();
            if let Some(relative_label) = item
                .label
                .strip_prefix(&prefix)
                .filter(|relative| !relative.contains("::"))
            {
                item.label = relative_label.to_owned();
            }
            (item, symbol)
        })
        .collect()
}
