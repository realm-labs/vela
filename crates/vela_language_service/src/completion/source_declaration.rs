use vela_analysis::{
    completion::{CompletionKind as AnalysisCompletionKind, declaration_completion},
    facts::AnalysisFacts,
};
use vela_hir::module_graph::ModuleGraph;

use crate::symbol_ref::source_symbol;
use crate::{QueryContext, TextRange};

use super::{
    CompletionItem, accumulator::CompletionAccumulator,
    analysis_item::service_item_from_analysis_completion, label_segment_matches,
    type_display::type_completion_item,
};

pub(super) fn source_const_completion_items(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    source_declaration_completion_items(graph, facts, query, replace_range, prefix, |kind| {
        matches!(kind, AnalysisCompletionKind::Const)
    })
}

pub(super) fn source_function_completion_items(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    source_declaration_completion_items(graph, facts, query, replace_range, prefix, |kind| {
        matches!(kind, AnalysisCompletionKind::Function)
    })
}

pub(super) fn source_type_completion_items(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    source_declaration_completion_items(graph, facts, query, replace_range, prefix, |kind| {
        matches!(
            kind,
            AnalysisCompletionKind::Type | AnalysisCompletionKind::Trait
        )
    })
}

fn source_declaration_completion_items(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
    accepts_kind: impl Fn(AnalysisCompletionKind) -> bool,
) -> Vec<CompletionItem> {
    let Some(current_module) = query.module_key() else {
        return Vec::new();
    };
    let mut accumulator = CompletionAccumulator::new(replace_range, prefix);
    let local_names = query
        .local_bindings_before_cursor()
        .map(|binding| binding.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let declarations = graph.declarations_by_name_prefix(prefix);
    for declaration in declarations {
        if declaration.visibility != vela_hir::module_graph::Visibility::Public
            && graph.module_key(declaration.module) != Some(current_module)
        {
            continue;
        }
        let Some(address) =
            super::source_address::declaration_address(graph, current_module, declaration)
        else {
            continue;
        };
        let Some(mut item) = declaration_completion(graph, facts, declaration) else {
            continue;
        };
        let symbol = item.label.clone();
        item.label = if graph.module_key(declaration.module) == Some(current_module) {
            declaration.name.clone()
        } else {
            address.clone()
        };
        if accepts_kind(item.kind) && label_segment_matches(&item.label, prefix) {
            let shadowed = local_names.contains(item.label.as_str());
            let mut completion = if matches!(
                item.kind,
                AnalysisCompletionKind::Type | AnalysisCompletionKind::Trait
            ) {
                // A short label is presentation, not a reference to a type in
                // another module. Keep current-module spelling only when a
                // visible local does not own that name in expression scope.
                let insertion = if local_names.contains(item.label.as_str()) {
                    address.clone()
                } else {
                    item.label.clone()
                };
                let mut completion = type_completion_item(item, &symbol, prefix);
                completion.insert_text = Some(insertion);
                completion
            } else {
                service_item_from_analysis_completion(item, prefix)
            };
            if shadowed {
                completion.metadata.lookup = Some(address.clone());
                completion.metadata.filter_text = Some(address.clone());
                completion.insert_text = Some(
                    super::analysis_item::callable_insert_text(completion.kind, &address)
                        .unwrap_or_else(|| address.clone()),
                );
            }
            completion
                .insert_text
                .get_or_insert_with(|| completion.label.clone());
            if let Some(signature) = graph.function_signature(declaration.id) {
                completion = completion.with_callable_asyncness(signature.asyncness);
            }
            accumulator.add(completion.with_symbol(source_symbol(symbol)));
        }
    }
    accumulator.into_items()
}
