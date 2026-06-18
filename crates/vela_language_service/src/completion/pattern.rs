use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};

use crate::{
    TextRange,
    completion::{
        CompletionInsertFormat, CompletionItem, CompletionKind, dedupe_and_filter_service_items,
        display_qualified_detail_parts, display_type_detail_parts, label_segment_matches,
    },
    symbol_ref::{schema_variant_symbol, source_enum_variant_symbol},
};

pub(super) fn pattern_completion_items(
    graph: &ModuleGraph,
    schema: &vela_analysis::registry::RegistryFacts,
    current_module: &[String],
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut items = script_pattern_variant_completions(graph, current_module);
    items.extend(schema_pattern_variant_completions(schema));
    dedupe_and_filter_service_items(items, replace_range, prefix, |item| {
        label_segment_matches(item.label(), prefix)
    })
}

fn script_pattern_variant_completions(
    graph: &ModuleGraph,
    current_module: &[String],
) -> Vec<CompletionItem> {
    graph
        .declarations()
        .filter(|declaration| declaration.kind == DeclarationKind::Enum)
        .filter_map(|declaration| {
            let shape = graph.enum_shape(declaration.id)?;
            let detail = enum_pattern_detail(graph, declaration, current_module);
            Some(shape.variants.iter().filter_map(move |variant| {
                let symbol = source_enum_variant_symbol(graph, declaration.id, &variant.name)?;
                Some(
                    CompletionItem {
                        label: variant.name.clone(),
                        kind: CompletionKind::Variant,
                        detail: detail.render(),
                        insert_text: None,
                        insert_format: CompletionInsertFormat::PlainText,
                        sort_text: None,
                        metadata: Default::default(),
                    }
                    .with_detail_parts(detail.clone())
                    .with_symbol(symbol),
                )
            }))
        })
        .flatten()
        .collect()
}

fn enum_pattern_detail(
    graph: &ModuleGraph,
    declaration: &Declaration,
    current_module: &[String],
) -> crate::DisplayParts {
    let Some(module_path) = graph.module_path(declaration.module) else {
        return crate::DisplayParts::symbol(&declaration.name);
    };
    if module_path.segments() == current_module {
        crate::DisplayParts::symbol(&declaration.name)
    } else {
        display_qualified_detail_parts(&module_path.join(), &declaration.name)
    }
}

fn schema_pattern_variant_completions(
    schema: &vela_analysis::registry::RegistryFacts,
) -> Vec<CompletionItem> {
    schema
        .variants()
        .map(|variant| {
            let owner = variant.owner;
            let name = variant.name;
            let detail_parts = display_type_detail_parts(&owner);
            CompletionItem {
                label: name.clone(),
                kind: CompletionKind::Variant,
                detail: detail_parts.render(),
                insert_text: None,
                insert_format: CompletionInsertFormat::PlainText,
                sort_text: None,
                metadata: Default::default(),
            }
            .with_detail_parts(detail_parts)
            .with_documentation(schema.variant_docs(&owner, &name))
            .with_symbol(schema_variant_symbol(&owner, &name))
        })
        .collect()
}
