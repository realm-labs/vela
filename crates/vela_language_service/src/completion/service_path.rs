use vela_hir::body::HirBodyOwner;

use crate::callable_context::service_callable_fact;
use crate::{CursorContextKind, DisplayParts, LanguageServiceDatabases, QueryContext};

use super::{
    CompletionContext, CompletionContextKind, CompletionItem, CompletionKind, CompletionSymbol,
    dedupe_and_filter_service_items,
};

use super::callable_path::{callable_item, has_argument_list, item};

// Reserved namespaces own even invalid/unknown paths: ordinary source or
// registry functions with the same spelling must never supply candidates.
pub(super) fn completion_items(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Option<Vec<CompletionItem>> {
    let base = query.cursor().module_base()?;
    let path = base.split("::").collect::<Vec<_>>();
    if !matches!(
        path.as_slice(),
        ["service"] | ["service", "base" | "pinned", ..]
    ) {
        return None;
    }
    let mut items = Vec::new();
    let replace_range = query
        .cursor()
        .identifier_range()
        .unwrap_or(context.replace_range());
    let has_arguments = has_argument_list(query, replace_range.end);
    if context.kind() != CompletionContextKind::ModulePath
        || query.cursor().kind() == CursorContextKind::UseImport
        || query.body().is_none_or(|body| {
            matches!(
                body.owner,
                HirBodyOwner::Lambda { .. } | HirBodyOwner::ParameterDefault { .. }
            )
        })
    {
        return Some(items);
    }
    if path == ["service"] {
        if query
            .service_path_owner(databases, &["service", "base"])
            .is_some()
        {
            items.push(namespace_item("base"));
        }
        if databases
            .schema_db()
            .service_set()
            .is_some_and(|set| !set.services().is_empty())
        {
            items.push(namespace_item("pinned"));
        }
    } else if path == ["service", "pinned"] {
        if let Some(schema) = databases.schema_db().service_set() {
            items.extend(schema.services().iter().map(|service| {
                item(
                    service.member(),
                    CompletionKind::Module,
                    service.member().to_owned(),
                )
                .with_detail_parts(DisplayParts::type_name(service.path()))
                .with_symbol(CompletionSymbol::Schema(service.path().to_owned()))
            }));
        }
    } else if let Some(owner) = query.service_path_owner(databases, &path) {
        for method in owner.methods() {
            let Some(callable) = service_callable_fact(
                databases.schema_db().facts(),
                owner.path(),
                method.name(),
                &format!("{base}::{}", method.name()),
            ) else {
                continue;
            };
            items.push(callable_item(&callable, method.name(), has_arguments));
        }
    }
    Some(dedupe_and_filter_service_items(
        items,
        replace_range,
        context.prefix(),
        |item| item.label().starts_with(context.prefix()),
    ))
}

fn namespace_item(name: &str) -> CompletionItem {
    let path = format!("service::{name}");
    item(name, CompletionKind::Module, name.to_owned())
        .with_detail_parts(DisplayParts::type_name(&path))
        .with_symbol(CompletionSymbol::Builtin(path))
}
