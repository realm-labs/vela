use super::{
    CallArgumentContext, CompletionInsertFormat, CompletionItem, CompletionKind,
    display_type_detail_parts, is_identifier_continue,
};
use crate::QueryContext;
use crate::callable_context::CallableFacts;
use vela_syntax::ast::AstNode;

pub(super) fn named_argument_completion_context(
    query: &QueryContext<'_>,
) -> Option<CallArgumentContext> {
    let call = query.call_argument_facts()?;
    let offset = query.cursor().replace_range().end;
    let expression = query.syntax_call()?;
    let arguments = expression.arguments();
    for argument in &arguments {
        let range = argument.syntax().text_range();
        if usize::from(range.start()) <= offset && offset <= usize::from(range.end()) {
            if argument
                .equal_token()
                .is_some_and(|token| usize::from(token.text_range().start()) < offset)
            {
                return None;
            }
            let text = query.text().get(usize::from(range.start())..offset)?;
            if !text
                .chars()
                .all(|ch| is_identifier_continue(ch) || ch.is_whitespace())
            {
                return None;
            }
        }
    }
    Some(CallArgumentContext {
        callee_range: Some(call.callee_range()),
        used_names: arguments.iter().filter_map(|arg| arg.name_text()).collect(),
        positional_count: arguments
            .iter()
            .filter(|arg| {
                arg.name_token().is_none()
                    && usize::from(arg.syntax().text_range().end())
                        < query.cursor().replace_range().start
            })
            .count(),
    })
}

pub(super) fn script_function_parameter_completions(
    callables: &[CallableFacts],
    used_names: &[&str],
    positional_count: usize,
) -> Vec<CompletionItem> {
    callables
        .iter()
        .filter(|callable| callable.supports_named_arguments())
        .flat_map(|callable| {
            callable
                .params()
                .iter()
                .skip(positional_count)
                .filter(|param| !used_names.contains(&param.name()))
                .map(|param| {
                    let mut detail_parts =
                        display_type_detail_parts(param.type_fact().display_name());
                    if param.defaulted() {
                        detail_parts.extend(crate::DisplayParts::plain(" (defaulted)"));
                    }
                    CompletionItem {
                        label: param.name().to_owned(),
                        kind: CompletionKind::Parameter,
                        detail: detail_parts.render(),
                        insert_text: Some(format!("{} = ", param.name())),
                        insert_format: CompletionInsertFormat::PlainText,
                        sort_text: None,
                        metadata: Default::default(),
                    }
                    .with_detail_parts(detail_parts)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
