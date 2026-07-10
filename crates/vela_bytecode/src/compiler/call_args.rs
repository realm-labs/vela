use std::collections::BTreeSet;

use vela_common::{Diagnostic, Span};
use vela_hir::ids::HirExprId;
use vela_hir::type_hint::ParamHint;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::compiler) struct HirCallArgument {
    pub(in crate::compiler) name: Option<String>,
    pub(in crate::compiler) span: Span,
    pub(in crate::compiler) value: HirExprId,
}

pub(in crate::compiler) fn resolve_hir_call_arguments(
    params: &[ParamHint],
    args: &[HirCallArgument],
    call_span: Span,
) -> Result<Vec<Option<HirCallArgument>>, Vec<Diagnostic>> {
    resolve_call_arguments(params, args, call_span, |arg| {
        (arg.name.as_deref(), arg.span)
    })
}

fn resolve_call_arguments<T: Clone>(
    params: &[ParamHint],
    args: &[T],
    call_span: Span,
    fields: impl Fn(&T) -> (Option<&str>, Span),
) -> Result<Vec<Option<T>>, Vec<Diagnostic>> {
    let mut slots = vec![None; params.len()];
    let mut slot_spans = vec![None; params.len()];
    let mut diagnostics = Vec::new();
    let mut next_positional = 0_usize;
    let mut seen_named = false;

    for arg in args {
        let (name, span) = fields(arg);
        let Some(index) = argument_index(
            params,
            name,
            span,
            &mut next_positional,
            &mut seen_named,
            &mut diagnostics,
        ) else {
            continue;
        };

        if let Some(previous_span) = slot_spans[index] {
            diagnostics.push(duplicate_argument_diagnostic(
                &params[index].name,
                previous_span,
                span,
            ));
            continue;
        }
        slots[index] = Some(arg.clone());
        slot_spans[index] = Some(span);
    }

    for (slot, param) in slots.iter().zip(params) {
        if slot.is_none() && param.default_value_span.is_none() {
            diagnostics.push(missing_argument_diagnostic(param, call_span));
        }
    }

    if diagnostics.is_empty() {
        Ok(slots)
    } else {
        Err(diagnostics)
    }
}

fn argument_index(
    params: &[ParamHint],
    arg_name: Option<&str>,
    arg_span: Span,
    next_positional: &mut usize,
    seen_named: &mut bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<usize> {
    if let Some(name) = arg_name {
        *seen_named = true;
        return match params.iter().position(|param| param.name == name) {
            Some(index) => Some(index),
            None => {
                diagnostics.push(unknown_named_argument_diagnostic(
                    name,
                    arg_span,
                    params.iter().map(|param| param.name.as_str()).collect(),
                ));
                None
            }
        };
    }

    if *seen_named {
        diagnostics.push(positional_after_named_diagnostic(arg_span));
        return None;
    }

    let index = *next_positional;
    *next_positional = next_positional.saturating_add(1);
    if index >= params.len() {
        diagnostics.push(too_many_arguments_diagnostic(arg_span, params.len()));
        return None;
    }
    Some(index)
}

fn unknown_named_argument_diagnostic(
    name: &str,
    span: Span,
    candidates: BTreeSet<&str>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(format!("unknown named argument `{name}`"))
        .with_code("compiler::unknown_named_argument")
        .with_span(span)
        .with_label(span, "argument name does not match any parameter");
    if !candidates.is_empty() {
        diagnostic =
            diagnostic.with_label(span, format!("available parameters: {}", join(candidates)));
    }
    diagnostic
}

fn positional_after_named_diagnostic(span: Span) -> Diagnostic {
    Diagnostic::error("positional argument after named argument")
        .with_code("compiler::positional_after_named_argument")
        .with_span(span)
        .with_label(
            span,
            "positional arguments must appear before named arguments",
        )
}

fn too_many_arguments_diagnostic(span: Span, expected: usize) -> Diagnostic {
    Diagnostic::error("too many arguments")
        .with_code("compiler::too_many_arguments")
        .with_span(span)
        .with_label(
            span,
            format!("call accepts {expected} positional argument(s)"),
        )
}

fn duplicate_argument_diagnostic(name: &str, previous_span: Span, span: Span) -> Diagnostic {
    Diagnostic::error(format!("duplicate argument for parameter `{name}`"))
        .with_code("compiler::duplicate_argument")
        .with_span(span)
        .with_label(previous_span, "previous argument is here")
        .with_label(span, "duplicate argument is here")
}

fn missing_argument_diagnostic(param: &ParamHint, call_span: Span) -> Diagnostic {
    Diagnostic::error(format!("missing required argument `{}`", param.name))
        .with_code("compiler::missing_required_argument")
        .with_span(call_span)
        .with_label(call_span, "call does not provide this required parameter")
        .with_label(param.span, "required parameter is declared here")
}

fn join(values: BTreeSet<&str>) -> String {
    values.into_iter().collect::<Vec<_>>().join(", ")
}
