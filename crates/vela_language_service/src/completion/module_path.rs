use vela_analysis::{
    completion::{
        CompletionItem as AnalysisCompletionItem, declaration_completion, global_completions,
    },
    registry::RegistryFacts,
    type_fact::TypeFact,
};
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph, Visibility};

use super::{
    CompletionContext, CompletionInsertFormat, CompletionItem, CompletionKind,
    dedupe_and_filter_service_items, display_qualified_detail, display_type_detail_parts,
    label_segment_matches, relevance::completion_sort_text,
};
use crate::symbol_ref::{schema_variant_symbol, source_enum_variant_symbol};

pub(super) fn module_path_completion_items(
    databases: &crate::LanguageServiceDatabases,
    query: &crate::QueryContext<'_>,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    let Some(current_module) = query.module_key() else {
        return Vec::new();
    };
    let scope = super::imports::ImportScope::new(databases, query);
    let Some(base) = context.module_base().and_then(|base| scope.expand(base)) else {
        return Vec::new();
    };
    let graph = databases.hir_db().graph();
    let schema = databases.schema_db().facts();
    let facts = databases.graph_analysis_facts();
    let mut analysis_items = global_completions(schema);
    let mut service_items = Vec::new();
    let segments = base.split("::").map(str::to_owned).collect::<Vec<_>>();
    let Some(base_key) = graph.resolve_module_path(current_module, &segments) else {
        return Vec::new();
    };
    if let Some(module) = graph.module_id(&base_key) {
        analysis_items.extend(graph.declarations_in_module(module).into_iter().filter_map(
            |declaration| {
                let mut item = declaration_completion(graph, facts, declaration)?;
                // Keep the spelling that addresses the selected package, including
                // crate/dependency aliases, separate from its canonical symbol.
                item.label = format!("{base}::{}", declaration.name);
                Some(item)
            },
        ));
    }
    analysis_items.extend(
        graph
            .module_child_segments(&base_key)
            .into_iter()
            .map(|segment| AnalysisCompletionItem {
                label: format!("{base}::{segment}"),
                kind: vela_analysis::completion::CompletionKind::Module,
                fact: TypeFact::module(format!("{base}::{segment}")),
            }),
    );
    let current_id = graph.module_id(current_module);
    let source_enum = current_id.and_then(|module| {
        graph.resolve_visible_declaration_path(module, &segments, DeclarationKind::Enum)
    });
    if let Some(declaration) = source_enum {
        if Some(declaration.module) != current_id && declaration.visibility != Visibility::Public {
            return Vec::new();
        }
        service_items.extend(script_enum_variant_path_completions(
            graph,
            declaration,
            context.prefix(),
        ));
    } else {
        service_items.extend(schema_enum_variant_path_completions(
            schema,
            &base,
            context.prefix(),
        ));
    }
    let namespace = format!("{base}::");
    let paths = analysis_items
        .iter()
        .filter_map(|item| {
            let suffix = item.label.strip_prefix(&namespace)?;
            let label = suffix.split("::").next()?;
            label.starts_with(context.prefix()).then_some(label)
        })
        .collect::<std::collections::BTreeSet<_>>();
    for label in paths {
        if let Some(item) =
            scope.item_for_path(&format!("{base}::{label}"), label, context.prefix())
        {
            service_items.push(item);
        }
    }
    dedupe_and_filter_service_items(
        service_items,
        context.replace_range(),
        context.prefix(),
        |item| label_segment_matches(item.label(), context.prefix()),
    )
}

fn script_enum_variant_path_completions(
    graph: &ModuleGraph,
    declaration: &Declaration,
    prefix: &str,
) -> Vec<CompletionItem> {
    let Some(owner) = declaration_owner_label(graph, declaration) else {
        return Vec::new();
    };
    let Some(shape) = graph.enum_shape(declaration.id) else {
        return Vec::new();
    };
    shape
        .variants
        .iter()
        .filter_map(|variant| {
            let symbol = source_enum_variant_symbol(graph, declaration.id, &variant.name)?;
            let detail_parts = display_type_detail_parts(&owner);
            Some(
                CompletionItem {
                    label: variant.name.clone(),
                    kind: CompletionKind::Variant,
                    detail: detail_parts.render(),
                    insert_text: Some(variant.name.clone()),
                    insert_format: CompletionInsertFormat::PlainText,
                    metadata: Default::default(),
                    sort_text: Some(completion_sort_text(
                        CompletionKind::Variant,
                        &variant.name,
                        prefix,
                    )),
                }
                .with_detail_parts(detail_parts)
                .with_symbol(symbol),
            )
        })
        .collect()
}

fn schema_enum_variant_path_completions(
    schema: &RegistryFacts,
    base: &str,
    prefix: &str,
) -> Vec<CompletionItem> {
    schema
        .variants_for_owner(base)
        .into_iter()
        .map(|variant| {
            let owner = variant.owner;
            let name = variant.name;
            let sort_text = completion_sort_text(CompletionKind::Variant, &name, prefix);
            let detail_parts = display_type_detail_parts(&owner);
            CompletionItem {
                label: name.clone(),
                kind: CompletionKind::Variant,
                detail: detail_parts.render(),
                insert_text: Some(name.clone()),
                insert_format: CompletionInsertFormat::PlainText,
                sort_text: Some(sort_text),
                metadata: Default::default(),
            }
            .with_detail_parts(detail_parts)
            .with_symbol(schema_variant_symbol(&owner, &name))
        })
        .collect()
}

fn declaration_owner_label(graph: &ModuleGraph, declaration: &Declaration) -> Option<String> {
    let module_path = graph.module_path(declaration.module)?;
    if module_path.segments().is_empty() {
        Some(declaration.name.clone())
    } else {
        Some(display_qualified_detail(
            &module_path.join(),
            &declaration.name,
        ))
    }
}
