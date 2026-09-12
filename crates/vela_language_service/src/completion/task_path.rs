use crate::callable_context::stdlib_callable_facts;
use crate::{CursorContextKind, QueryContext};

use super::callable_path::{callable_item, has_argument_list};
use super::{
    CompletionContext, CompletionContextKind, CompletionInsertFormat, CompletionItem,
    CompletionKind, dedupe_and_filter_service_items,
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

pub(super) fn adjust_continuation_items(query: &QueryContext<'_>, items: &mut [CompletionItem]) {
    use vela_syntax::ast::SyntaxExpressionKind;

    let is_continuation = query.call_active_parameter_index() == Some(1)
        && query.call_argument_facts().and_then(|call| call.callee_path()).is_some_and(|path| {
            matches!(path, [root, operation] if root == "task" && operation == "spawn_scoped_then")
        });
    if !is_continuation {
        return;
    }
    // A completion inside a callback body or another expression still needs
    // its ordinary call insertion, even when the outer task operand is invalid.
    if query.syntax_call().is_some_and(|call| {
        call.arguments()
            .get(1)
            .and_then(|arg| arg.expression())
            .is_some_and(|expr| expr.expression_kind() != SyntaxExpressionKind::Path)
    }) {
        return;
    }
    for item in items
        .iter_mut()
        .filter(|item| item.kind() == CompletionKind::Function)
    {
        let Some(path) = item
            .insert_text()
            .and_then(|text| text.strip_suffix("($0)"))
        else {
            continue;
        };
        let path = path.to_owned();
        item.insert_text = Some(path.clone());
        item.insert_format = CompletionInsertFormat::PlainText;
        if let Some(edit) = &mut item.metadata.text_edit {
            edit.new_text = path;
            if let Some(range) = query.cursor().identifier_range() {
                edit.range = range;
                item.metadata.edit_range = Some(range);
            }
        }
    }
}
