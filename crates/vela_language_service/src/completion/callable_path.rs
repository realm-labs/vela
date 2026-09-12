use vela_syntax::ast::AstNode;
use vela_syntax::{SyntaxKind, TextSize};

use crate::callable_context::CallableFacts;
use crate::{DisplayParts, QueryContext};

use super::{CompletionInsertFormat, CompletionItem, CompletionKind};

pub(super) fn callable_item(
    callable: &CallableFacts,
    label: &str,
    has_arguments: bool,
) -> CompletionItem {
    let detail = DisplayParts::callable_signature_with_asyncness(
        callable.asyncness(),
        callable.name(),
        callable
            .params()
            .iter()
            .map(|param| DisplayParts::parameter(param.name(), &param.type_fact().display_name())),
        Some(&callable.return_display_name()),
    );
    item(
        label,
        CompletionKind::Function,
        if has_arguments {
            label.to_owned()
        } else {
            format!("{label}($0)")
        },
    )
    .with_detail_parts(detail)
    .with_symbol(callable.symbol().clone())
}

pub(super) fn item(label: &str, kind: CompletionKind, insertion: String) -> CompletionItem {
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

pub(super) fn preserve_argument_list(query: &QueryContext<'_>, items: &mut [CompletionItem]) {
    let range = query
        .identifier_range()
        .unwrap_or(query.cursor().replace_range());
    if !has_argument_list(query, range.end) {
        return;
    }
    for item in items {
        if !matches!(
            item.kind(),
            CompletionKind::Function | CompletionKind::Method
        ) {
            continue;
        }
        let Some(path) = item
            .insert_text()
            .and_then(|text| text.strip_suffix("($0)"))
        else {
            continue;
        };
        super::call_operand::set_path_insertion(item, query, path.to_owned());
    }
}

pub(super) fn has_argument_list(query: &QueryContext<'_>, end: usize) -> bool {
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
