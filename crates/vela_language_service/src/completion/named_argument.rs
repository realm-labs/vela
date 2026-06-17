use crate::CallArgumentFacts;
use crate::callable_context::CallableFacts;

use super::{
    CallArgumentContext, CompletionInsertFormat, CompletionItem, CompletionKind,
    display_type_detail, is_identifier_continue,
};

pub(super) fn named_argument_completion_context(
    call: Option<CallArgumentFacts<'_>>,
) -> Option<CallArgumentContext> {
    let call = call?;
    let args_before_cursor = call.args_prefix();
    let current_arg = current_argument_text(args_before_cursor);
    if current_arg.contains(':') || !is_argument_name_prefix(current_arg.trim_start()) {
        return None;
    }
    Some(CallArgumentContext {
        callee: call.callee().to_owned(),
        callee_range: Some(call.callee_range()),
        used_names: used_named_arguments(args_before_cursor),
    })
}

pub(super) fn script_function_parameter_completions(
    callables: &[CallableFacts],
    used_names: &[&str],
) -> Vec<CompletionItem> {
    callables
        .iter()
        .flat_map(|callable| {
            callable
                .params()
                .iter()
                .filter(|param| !used_names.contains(&param.name()))
                .map(|param| {
                    let mut detail = display_type_detail(param.type_fact().display_name());
                    if param.defaulted() {
                        detail.push_str(" (defaulted)");
                    }
                    CompletionItem {
                        label: param.name().to_owned(),
                        kind: CompletionKind::Parameter,
                        detail,
                        insert_text: Some(format!("{}: ", param.name())),
                        insert_format: CompletionInsertFormat::PlainText,
                        sort_text: None,
                        metadata: Default::default(),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn current_argument_text(args_before_cursor: &str) -> &str {
    let mut depth = 0_usize;
    let mut start = 0_usize;
    for (index, ch) in args_before_cursor.char_indices() {
        match ch {
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => start = index + ch.len_utf8(),
            _ => {}
        }
    }
    &args_before_cursor[start..]
}

fn used_named_arguments(args_before_cursor: &str) -> Vec<String> {
    let mut names = Vec::new();
    for argument in top_level_argument_texts(args_before_cursor) {
        let argument = argument.trim_start();
        let Some(colon) = top_level_colon(argument) else {
            continue;
        };
        let candidate = argument[..colon].trim();
        if !candidate.is_empty() && candidate.chars().all(is_identifier_continue) {
            names.push(candidate.to_owned());
        }
    }
    names
}

fn top_level_argument_texts(args_before_cursor: &str) -> Vec<&str> {
    let mut depth = 0_usize;
    let mut start = 0_usize;
    let mut args = Vec::new();
    for (index, ch) in args_before_cursor.char_indices() {
        match ch {
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                args.push(&args_before_cursor[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    args.push(&args_before_cursor[start..]);
    args
}

fn top_level_colon(argument: &str) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, ch) in argument.char_indices() {
        match ch {
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn is_argument_name_prefix(text: &str) -> bool {
    text.chars()
        .all(|ch| is_identifier_continue(ch) || ch.is_whitespace())
}
