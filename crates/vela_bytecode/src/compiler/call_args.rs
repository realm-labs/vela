use std::collections::BTreeSet;

use vela_common::{Diagnostic, Span};
use vela_hir::ids::HirDeclId;
use vela_hir::type_hint::ParamHint;
use vela_syntax::ast::Argument;

use crate::CallArgument;

use super::value_types::{TypeContractContext, type_hint_value_type};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(super) fn compile_script_call_args(
        &mut self,
        declaration: HirDeclId,
        args: &[Argument],
        call_span: Span,
    ) -> CompileResult<Vec<CallArgument>> {
        let params = self
            .facts
            .script_function_signatures
            .get(&declaration)
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("script call")))?
            .clone();
        let slots =
            resolve_script_call_arguments(&params, args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;

        slots
            .into_iter()
            .zip(params)
            .map(|(slot, param)| {
                if let Some(arg) = slot {
                    self.compile_argument_for_param(&arg.value, &param)
                        .map(CallArgument::Register)
                } else if param.default_value_span.is_some() {
                    Ok(CallArgument::Missing)
                } else {
                    unreachable!("call argument resolver rejects missing required arguments")
                }
            })
            .collect()
    }

    pub(super) fn compile_argument_for_param(
        &mut self,
        value: &vela_syntax::ast::Expr,
        param: &ParamHint,
    ) -> CompileResult<crate::Register> {
        let Some(expected) = param.type_hint.as_ref().and_then(type_hint_value_type) else {
            return self.compile_expr(value);
        };
        self.compile_expr_with_expected_type(
            value,
            expected,
            TypeContractContext::FunctionParameter {
                name: param.name.clone(),
            },
        )
    }
}

pub(super) fn resolve_script_call_arguments<'ast>(
    params: &[ParamHint],
    args: &'ast [Argument],
    call_span: Span,
) -> Result<Vec<Option<&'ast Argument>>, Vec<Diagnostic>> {
    let mut slots = vec![None; params.len()];
    let mut slot_spans = vec![None; params.len()];
    let mut diagnostics = Vec::new();
    let mut next_positional = 0_usize;
    let mut seen_named = false;

    for arg in args {
        let arg_span = arg.value.span;
        let Some(index) = argument_index(
            params,
            arg,
            arg_span,
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
                arg_span,
            ));
            continue;
        }
        slots[index] = Some(arg);
        slot_spans[index] = Some(arg_span);
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
    arg: &Argument,
    arg_span: Span,
    next_positional: &mut usize,
    seen_named: &mut bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<usize> {
    if let Some(name) = &arg.name {
        *seen_named = true;
        return match params.iter().position(|param| param.name == *name) {
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
