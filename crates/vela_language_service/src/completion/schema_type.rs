use vela_analysis::{
    completion::{CompletionKind as AnalysisCompletionKind, type_completions},
    registry::RegistryFacts,
};

use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::{QueryContext, TextRange, symbol_ref::schema_symbol};

use super::{
    CompletionItem, accumulator::CompletionAccumulator, label_segment_matches,
    type_display::type_completion_item,
};

pub(super) fn schema_type_completion_items(
    schema: &RegistryFacts,
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut accumulator = CompletionAccumulator::new(replace_range, prefix);
    let local_names = query
        .local_bindings_before_cursor()
        .map(|binding| binding.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let module = query.module_key().and_then(|key| graph.module_id(key));
    for item in type_completions(schema) {
        if !matches!(
            item.kind,
            AnalysisCompletionKind::Type | AnalysisCompletionKind::Trait
        ) || !label_segment_matches(&item.label, prefix)
        {
            continue;
        }
        let qualified_name = item.label.clone();
        let path = qualified_name
            .split("::")
            .map(str::to_owned)
            .collect::<Vec<_>>();
        // Registry metadata cannot replace a source owner, including a private
        // declaration that makes this spelling unavailable from this module.
        if module.is_some_and(|module| {
            [
                DeclarationKind::Struct,
                DeclarationKind::Enum,
                DeclarationKind::Trait,
                DeclarationKind::Function,
                DeclarationKind::Const,
                DeclarationKind::State,
            ]
            .into_iter()
            .any(|kind| {
                graph
                    .resolve_visible_declaration_path(module, &path, kind)
                    .is_some()
            })
        }) || (path.len() == 1 && local_names.contains(qualified_name.as_str()))
        {
            continue;
        }
        let mut completion = type_completion_item(item, &qualified_name, prefix);
        completion.insert_text = Some(qualified_name.clone());
        accumulator.add(completion.with_symbol(schema_symbol(qualified_name)));
    }
    accumulator.into_items()
}
