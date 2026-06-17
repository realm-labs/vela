use vela_analysis::{
    completion::{
        CompletionItem as AnalysisCompletionItem, declaration_completions, global_completions,
        module_completions,
    },
    facts::AnalysisFacts,
    registry::RegistryFacts,
};
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};

use super::{
    CompletionContext, CompletionInsertFormat, CompletionItem, CompletionKind, CompletionSymbol,
    callable_insert_text, completion_insert_format, completion_sort_text,
    dedupe_and_filter_service_items, label_segment_matches,
};

pub(super) fn module_path_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    let facts = AnalysisFacts::from_module_graph(graph);
    let Some(base) = context.module_base() else {
        return Vec::new();
    };
    let mut analysis_items = global_completions(schema);
    analysis_items.extend(declaration_completions(graph, &facts));
    analysis_items.extend(module_completions(graph));
    let mut service_items = Vec::new();
    service_items.extend(script_enum_variant_path_completions(
        graph,
        base,
        context.prefix(),
    ));
    service_items.extend(schema_enum_variant_path_completions(
        schema,
        base,
        context.prefix(),
    ));
    for item in analysis_items {
        if let Some(service_item) = service_item_for_module_path(item, base, context.prefix()) {
            service_items.push(service_item);
        }
    }
    dedupe_and_filter_service_items(
        service_items,
        context.replace_range(),
        context.prefix(),
        |item| label_segment_matches(item.label(), context.prefix()),
    )
}

fn service_item_for_module_path(
    item: AnalysisCompletionItem,
    base: &str,
    prefix: &str,
) -> Option<CompletionItem> {
    let suffix = item
        .label
        .strip_prefix(base)
        .and_then(|suffix| suffix.strip_prefix("::"))?;
    if !suffix.starts_with(prefix) {
        return None;
    }
    let label = suffix
        .split_once("::")
        .map_or(suffix, |(segment, _)| segment)
        .to_owned();
    let kind = if suffix.contains("::") {
        CompletionKind::Module
    } else {
        item.kind.into()
    };
    let insert_text = callable_insert_text(kind, &label);
    let insert_format = completion_insert_format(insert_text.as_ref());
    Some(CompletionItem {
        sort_text: Some(completion_sort_text(kind, &label, prefix)),
        metadata: Default::default(),
        label,
        kind,
        detail: item.fact.display_name(),
        insert_text,
        insert_format,
    })
}

fn script_enum_variant_path_completions(
    graph: &ModuleGraph,
    base: &str,
    prefix: &str,
) -> Vec<CompletionItem> {
    graph
        .declarations()
        .filter(|declaration| declaration.kind == DeclarationKind::Enum)
        .filter(|declaration| declaration_owner_matches(graph, declaration, base))
        .filter_map(|declaration| {
            let owner = declaration_owner_label(graph, declaration)?;
            let shape = graph.enum_shape(declaration.id)?;
            Some(shape.variants.iter().map(move |variant| CompletionItem {
                label: variant.name.clone(),
                kind: CompletionKind::Variant,
                detail: owner.clone(),
                insert_text: None,
                insert_format: CompletionInsertFormat::PlainText,
                metadata: Default::default(),
                sort_text: Some(completion_sort_text(
                    CompletionKind::Variant,
                    &variant.name,
                    prefix,
                )),
            }))
        })
        .flatten()
        .collect()
}

fn schema_enum_variant_path_completions(
    schema: &RegistryFacts,
    base: &str,
    prefix: &str,
) -> Vec<CompletionItem> {
    schema
        .variants()
        .filter(|variant| owner_matches_path_base(&variant.owner, base))
        .map(|variant| {
            let owner = variant.owner;
            let name = variant.name;
            let sort_text = completion_sort_text(CompletionKind::Variant, &name, prefix);
            CompletionItem {
                label: name.clone(),
                kind: CompletionKind::Variant,
                detail: owner.clone(),
                insert_text: None,
                insert_format: CompletionInsertFormat::PlainText,
                sort_text: Some(sort_text),
                metadata: Default::default(),
            }
            .with_documentation(schema.variant_docs(&owner, &name))
            .with_symbol(CompletionSymbol::Schema(format!("{owner}::{name}")))
        })
        .collect()
}

fn declaration_owner_matches(graph: &ModuleGraph, declaration: &Declaration, base: &str) -> bool {
    declaration_owner_label(graph, declaration)
        .as_deref()
        .is_some_and(|owner| owner_matches_path_base(owner, base))
}

fn declaration_owner_label(graph: &ModuleGraph, declaration: &Declaration) -> Option<String> {
    let module_path = graph.module_path(declaration.module)?;
    if module_path.segments().is_empty() {
        Some(declaration.name.clone())
    } else {
        Some(format!("{}::{}", module_path.join(), declaration.name))
    }
}

fn owner_matches_path_base(owner: &str, base: &str) -> bool {
    owner == base || owner.rsplit("::").next() == Some(base)
}
