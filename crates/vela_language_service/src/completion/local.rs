use vela_analysis::facts::AnalysisFacts;
use vela_analysis::type_fact::TypeFact;
use vela_hir::binding::LocalBindingKind;
use vela_hir::module_graph::ModuleGraph;

use crate::QueryContext;

use super::{
    CompletionContext, CompletionInsertFormat, CompletionItem, CompletionKind,
    accumulator::CompletionAccumulator, relevance::completion_sort_text,
};

pub(super) fn local_completion_items(
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    let facts = AnalysisFacts::from_module_graph(graph);
    let items = query
        .local_bindings_before_cursor()
        .filter(|local| local.name.starts_with(context.prefix()))
        .map(|local| {
            let kind = match local.kind {
                LocalBindingKind::Parameter => CompletionKind::Parameter,
                LocalBindingKind::Let
                | LocalBindingKind::For
                | LocalBindingKind::LambdaParameter
                | LocalBindingKind::Pattern => CompletionKind::Binding,
            };
            let fact = facts.local(local.id).cloned().unwrap_or(TypeFact::Unknown);
            CompletionItem {
                sort_text: Some(completion_sort_text(kind, &local.name, "")),
                metadata: Default::default(),
                label: local.name.clone(),
                kind,
                detail: fact.display_name(),
                insert_text: None,
                insert_format: CompletionInsertFormat::PlainText,
            }
        })
        .collect::<Vec<_>>();
    let mut accumulator = CompletionAccumulator::new(context.replace_range(), context.prefix());
    accumulator.add_many(items);
    accumulator.into_items()
}
