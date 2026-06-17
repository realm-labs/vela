use vela_analysis::facts::AnalysisFacts;
use vela_analysis::type_fact::TypeFact;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::{LineIndex, Position};

use super::{
    CallArgumentContext, CompletionInsertFormat, CompletionItem, CompletionKind,
    completion_type_fact, is_identifier_continue,
};

pub(super) fn named_argument_completion_context(
    text: &str,
    position: Position,
) -> Option<CallArgumentContext> {
    let offset = LineIndex::new(text).offset(position);
    let open = active_call_open(text, offset)?;
    let callee = callee_before_open(text, open)?;
    let args_before_cursor = &text[open + 1..offset];
    let current_arg = current_argument_text(args_before_cursor);
    if current_arg.contains(':') || !is_argument_name_prefix(current_arg.trim_start()) {
        return None;
    }
    Some(CallArgumentContext {
        callee,
        used_names: used_named_arguments(args_before_cursor),
    })
}

pub(super) fn script_function_parameter_completions(
    graph: &ModuleGraph,
    schema: &vela_analysis::registry::RegistryFacts,
    callee: &str,
    used_names: &[&str],
) -> Vec<CompletionItem> {
    let facts = AnalysisFacts::from_module_graph(graph);
    graph
        .declarations()
        .filter(|declaration| {
            declaration.kind == DeclarationKind::Function
                && (declaration.name == callee
                    || qualified_declaration_label(graph, declaration.id) == callee)
        })
        .filter_map(|declaration| {
            let signature = graph.function_signature(declaration.id)?;
            let params = facts
                .declaration(declaration.id)
                .and_then(|fact| match fact {
                    TypeFact::Function { params, .. } => Some(params.as_slice()),
                    _ => None,
                })
                .unwrap_or(&[]);
            Some(
                signature
                    .params
                    .iter()
                    .enumerate()
                    .filter(|(_, param)| !used_names.contains(&param.name.as_str()))
                    .map(|(index, param)| {
                        let fact = params
                            .get(index)
                            .cloned()
                            .filter(|fact| !matches!(fact, TypeFact::Unknown))
                            .or_else(|| {
                                param
                                    .type_hint
                                    .as_ref()
                                    .map(|hint| completion_type_fact(graph, hint, schema))
                            })
                            .unwrap_or(TypeFact::Unknown);
                        let mut detail = fact.display_name();
                        if param.default_value_span.is_some() {
                            detail.push_str(" (defaulted)");
                        }
                        CompletionItem {
                            label: param.name.clone(),
                            kind: CompletionKind::Parameter,
                            detail,
                            insert_text: Some(format!("{}: ", param.name)),
                            insert_format: CompletionInsertFormat::PlainText,
                            sort_text: None,
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

fn active_call_open(text: &str, offset: usize) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, ch) in text[..offset].char_indices() {
        match ch {
            '(' => stack.push(index),
            ')' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.pop()
}

fn callee_before_open(text: &str, open: usize) -> Option<String> {
    let before = text[..open].trim_end();
    let end = before.len();
    let start = before
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_callee_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| before[start..end].to_owned())
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

fn is_callee_continue(ch: char) -> bool {
    ch == '_' || ch == ':' || ch == '.' || ch.is_ascii_alphanumeric()
}

fn qualified_declaration_label(
    graph: &ModuleGraph,
    declaration: vela_hir::ids::HirDeclId,
) -> String {
    let Some(declaration) = graph.declaration(declaration) else {
        return String::new();
    };
    let Some(module_path) = graph.module_path(declaration.module) else {
        return declaration.name.clone();
    };
    let module = module_path.join();
    if module.is_empty() {
        declaration.name.clone()
    } else {
        format!("{module}::{}", declaration.name)
    }
}
