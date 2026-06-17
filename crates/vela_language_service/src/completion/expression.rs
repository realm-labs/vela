use vela_analysis::registry::RegistryFacts;
use vela_hir::module_graph::ModuleGraph;

use crate::QueryContext;

use super::{
    CompletionContext, CompletionItem, dedupe_and_filter_service_items, label_segment_matches,
    local::local_completion_items,
    schema_function::schema_function_completion_items,
    schema_type::schema_type_completion_items,
    source_declaration::{
        source_const_completion_items, source_function_completion_items,
        source_type_completion_items,
    },
    source_module::source_module_completion_items,
    stdlib_function::stdlib_function_completion_items,
};

pub(super) fn expression_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    let mut items = local_completion_items(graph, query, context);
    items.extend(schema_type_completion_items(
        schema,
        context.replace_range(),
        context.prefix(),
    ));
    items.extend(schema_function_completion_items(
        schema,
        context.replace_range(),
        context.prefix(),
    ));
    items.extend(stdlib_function_completion_items(
        context.replace_range(),
        context.prefix(),
    ));
    items.extend(source_const_completion_items(
        graph,
        query,
        context.replace_range(),
        context.prefix(),
    ));
    items.extend(source_function_completion_items(
        graph,
        query,
        context.replace_range(),
        context.prefix(),
    ));
    items.extend(source_type_completion_items(
        graph,
        query,
        context.replace_range(),
        context.prefix(),
    ));
    items.extend(source_module_completion_items(
        graph,
        context.replace_range(),
        context.prefix(),
    ));
    dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |item| {
        label_segment_matches(&item.label, context.prefix())
    })
}

pub(super) fn statement_expression_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    expression_completion_items(graph, schema, query, context)
}
