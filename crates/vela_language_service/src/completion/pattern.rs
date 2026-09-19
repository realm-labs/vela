use super::{
    CompletionContext, CompletionItem, imports::ImportScope,
    module_path::enum_variant_path_completions, type_paths::TypePaths,
};
use crate::{LanguageServiceDatabases, QueryContext};

pub(super) fn pattern_completion_items(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    let graph = databases.hir_db().graph();
    if context.record_constructor.is_some() {
        return super::record_field::record_field_completion_items(
            graph,
            databases.schema_db().facts(),
            context,
        );
    }
    let Some(module) = query.module_key().and_then(|key| graph.module_id(key)) else {
        return Vec::new();
    };
    let scope = ImportScope::new(databases, query);
    let mut items = Vec::new();
    if let Some(base) = context.module_base() {
        let Some(base) = scope.expand(base) else {
            return items;
        };
        items = enum_variant_path_completions(
            graph,
            databases.schema_db().facts(),
            module,
            &base,
            context.prefix(),
        );
    } else if let Some(paths) = TypePaths::new(databases, query) {
        for base in paths
            .paths
            .keys()
            .filter(|base| scope.expand(base).as_ref() == Some(base))
        {
            for mut item in enum_variant_path_completions(
                graph,
                databases.schema_db().facts(),
                module,
                base,
                context.prefix(),
            ) {
                let insertion = format!("{base}::{}", item.label);
                item.metadata.lookup = Some(insertion.clone());
                item.metadata.filter_text = Some(insertion.clone());
                item.insert_text = Some(insertion);
                items.push(item);
            }
        }
    }
    super::dedupe_and_filter_service_items(
        items,
        context.replace_range(),
        context.prefix(),
        |item| super::label_segment_matches(item.label(), context.prefix()),
    )
}
