use vela_hir::body::HirBodyOwner;
use vela_syntax::ast::AstNode;
use vela_syntax::{SyntaxKind, TextSize};

use crate::callable_context::service_callable_fact;
use crate::{CursorContextKind, DisplayParts, LanguageServiceDatabases, QueryContext};

use super::{
    CompletionContext, CompletionContextKind, CompletionInsertFormat, CompletionItem,
    CompletionKind, CompletionSymbol, dedupe_and_filter_service_items,
};

// Reserved namespaces own even invalid/unknown paths: ordinary source or
// registry functions with the same spelling must never supply candidates.
pub(super) fn completion_items(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Option<Vec<CompletionItem>> {
    let base = query.cursor().module_base()?;
    let path = base.split("::").collect::<Vec<_>>();
    if !matches!(path.as_slice(), ["service", "base" | "pinned", ..]) {
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
    if path == ["service", "pinned"] {
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
            let detail = DisplayParts::callable_signature_with_asyncness(
                callable.asyncness(),
                callable.name(),
                callable.params().iter().map(|param| {
                    DisplayParts::parameter(param.name(), &param.type_fact().display_name())
                }),
                Some(&callable.return_display_name()),
            );
            items.push(
                item(
                    method.name(),
                    CompletionKind::Function,
                    if has_arguments {
                        method.name().to_owned()
                    } else {
                        format!("{}($0)", method.name())
                    },
                )
                .with_detail_parts(detail)
                .with_symbol(callable.symbol().clone()),
            );
        }
    }
    Some(dedupe_and_filter_service_items(
        items,
        replace_range,
        context.prefix(),
        |item| item.label().starts_with(context.prefix()),
    ))
}

fn item(label: &str, kind: CompletionKind, insertion: String) -> CompletionItem {
    let insert_format = if insertion.contains("$0") {
        CompletionInsertFormat::Snippet
    } else {
        CompletionInsertFormat::PlainText
    };
    CompletionItem {
        label: label.to_owned(),
        kind,
        detail: String::new(),
        insert_text: Some(insertion),
        insert_format,
        sort_text: None,
        metadata: Default::default(),
    }
}

fn has_argument_list(query: &QueryContext<'_>, end: usize) -> bool {
    let Some(parse) = query.syntax_parse() else {
        return false;
    };
    let Ok(end) = u32::try_from(end) else {
        return false;
    };
    let end = TextSize::from(end);
    let first = parse.tree().syntax().token_at_offset(end).right_biased();
    std::iter::successors(first, |token| token.next_token())
        .find(|token| token.text_range().start() >= end && !token.kind().is_trivia())
        .is_some_and(|token| token.kind() == SyntaxKind::LParen)
}
