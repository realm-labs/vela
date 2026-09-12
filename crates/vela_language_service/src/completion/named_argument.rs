use super::{
    CallArgumentContext, CompletionInsertFormat, CompletionItem, CompletionItemMetadata,
    CompletionKind, CompletionLabelDetails, display_type_detail_parts, is_identifier_continue,
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
        has_equal: query.call_argument_position()?.has_equal,
    })
}

pub(super) fn script_function_parameter_completions(
    callables: &[CallableFacts],
    query: &QueryContext<'_>,
    has_equal: bool,
) -> Vec<CompletionItem> {
    let Some(position) = query.call_argument_position() else {
        return Vec::new();
    };
    callables
        .iter()
        .filter(|callable| callable.supports_named_arguments())
        .flat_map(|callable| {
            let available = position.available_parameters(callable);
            callable
                .params()
                .iter()
                .zip(available)
                .filter(|(param, available)| *available && insertable_parameter_name(param.name()))
                .map(|(param, _)| {
                    let mut detail_parts =
                        display_type_detail_parts(param.type_fact().display_name());
                    if param.defaulted() {
                        detail_parts.extend(crate::DisplayParts::plain(" (defaulted)"));
                    }
                    CompletionItem {
                        label: param.name().to_owned(),
                        kind: CompletionKind::Parameter,
                        detail: detail_parts.render(),
                        insert_text: Some(if has_equal {
                            param.name().to_owned()
                        } else {
                            format!("{} = ", param.name())
                        }),
                        insert_format: CompletionInsertFormat::PlainText,
                        sort_text: None,
                        metadata: CompletionItemMetadata {
                            argument_name: true,
                            label_details: CompletionLabelDetails {
                                description: Some("named argument".to_owned()),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    }
                    .with_detail_parts(detail_parts)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn insertable_parameter_name(name: &str) -> bool {
    use vela_syntax::token::{Token, TokenKind};
    let tokens = vela_syntax::lexer::lex(vela_common::SourceId::new(0), name).tokens;
    matches!(tokens.as_slice(), [Token { kind: TokenKind::Ident(identifier), .. }, Token { kind: TokenKind::Eof, .. }] if identifier == name)
}
