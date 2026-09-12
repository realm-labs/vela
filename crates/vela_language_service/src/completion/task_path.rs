use crate::callable_context::stdlib_callable_facts;
use crate::{CursorContextKind, QueryContext};

use super::callable_path::{callable_item, has_argument_list};
use super::{
    CompletionContext, CompletionContextKind, CompletionItem, dedupe_and_filter_service_items,
};

pub(super) fn completion_items(
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Option<Vec<CompletionItem>> {
    let base = query.cursor().module_base()?;
    if !(base == "task" || base.starts_with("task::"))
        || context.kind() != CompletionContextKind::ModulePath
        || query.cursor().kind() == CursorContextKind::UseImport
    {
        return None;
    }
    let range = query
        .cursor()
        .identifier_range()
        .unwrap_or(context.replace_range());
    let has_arguments = has_argument_list(query, range.end);
    let items = if base == "task" {
        ["task::spawn_scoped", "task::spawn_scoped_then"]
            .into_iter()
            .flat_map(stdlib_callable_facts)
            .map(|callable| {
                callable_item(
                    &callable,
                    callable.name().trim_start_matches("task::"),
                    has_arguments,
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    Some(dedupe_and_filter_service_items(
        items,
        range,
        context.prefix(),
        |item| item.label().starts_with(context.prefix()),
    ))
}
