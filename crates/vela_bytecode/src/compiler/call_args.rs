use std::collections::BTreeSet;

use vela_common::{Diagnostic, Span};
use vela_hir::ids::HirDeclId;
use vela_hir::type_hint::ParamHint;
use vela_syntax::ast::{Argument, SyntaxExpression};

use crate::{CallArgument, ScriptCallMode};

use super::body_payloads::{CompilerArgumentPayload, CompilerExpressionPayload};
use super::value_types::{ExpectedTypeOutcome, TypeContractContext, type_hint_value_type};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScriptCallArgs {
    pub(super) args: Vec<CallArgument>,
    pub(super) mode: ScriptCallMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::compiler) struct SyntaxCallArgument {
    pub(in crate::compiler) name: Option<String>,
    pub(in crate::compiler) span: Span,
    pub(in crate::compiler) value: SyntaxExpression,
}

#[derive(Clone, Copy)]
pub(in crate::compiler) struct CallArgumentSyntax<'payload, 'ast> {
    args: &'payload [Argument],
    payloads: Option<&'payload [CompilerArgumentPayload<'ast>]>,
}

impl<'payload, 'ast> CallArgumentSyntax<'payload, 'ast> {
    pub(in crate::compiler) fn new(
        args: &'payload [Argument],
        payloads: Option<&'payload [CompilerArgumentPayload<'ast>]>,
    ) -> Self {
        Self { args, payloads }
    }

    fn payload_for(self, arg: &Argument) -> Option<&'payload CompilerArgumentPayload<'ast>> {
        let index = self
            .args
            .iter()
            .position(|candidate| std::ptr::eq(candidate, arg))?;
        self.payloads?.get(index)
    }

    pub(in crate::compiler) fn value_expression_payload_for(
        self,
        arg: &Argument,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        Some(self.payload_for(arg)?.value_expression_payload())
    }

    pub(in crate::compiler) fn name_for(self, arg: &Argument) -> Option<String> {
        self.payload_for(arg)
            .and_then(CompilerArgumentPayload::syntax_name)
            .or_else(|| arg.name.clone())
    }

    pub(in crate::compiler) fn has_named_args(self) -> bool {
        self.args.iter().any(|arg| self.name_for(arg).is_some())
    }
}

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_script_call_args_with_payloads(
        &mut self,
        declaration: HirDeclId,
        args: &[Argument],
        call_span: Span,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<ScriptCallArgs> {
        let params = self
            .facts
            .script_function_signatures
            .get(&declaration)
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("script call")))?
            .clone();
        let slots = resolve_script_call_arguments(&params, args, call_span, arg_syntax).map_err(
            |diagnostics| CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics)),
        )?;

        let mut mode = ScriptCallMode::Unchecked;
        let args = slots
            .into_iter()
            .zip(params)
            .map(|(slot, param)| {
                if let Some(arg) = slot {
                    let (register, requires_guard) =
                        self.compile_argument_for_param_with_payloads(arg, &param, arg_syntax)?;
                    if requires_guard {
                        mode = ScriptCallMode::Checked;
                    }
                    Ok(CallArgument::Register(register))
                } else if param.default_value_span.is_some() {
                    if param.type_hint.is_some() {
                        mode = ScriptCallMode::Checked;
                    }
                    Ok(CallArgument::Missing)
                } else {
                    unreachable!("call argument resolver rejects missing required arguments")
                }
            })
            .collect::<CompileResult<Vec<_>>>()?;
        Ok(ScriptCallArgs { args, mode })
    }

    pub(in crate::compiler) fn compile_argument_for_param_with_payloads(
        &mut self,
        arg: &Argument,
        param: &ParamHint,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<(crate::Register, bool)> {
        let value = &arg.value;
        let Some(expected) = param.type_hint.as_ref().and_then(type_hint_value_type) else {
            return self
                .compile_call_argument_value(arg, arg_syntax)
                .map(|register| (register, false));
        };
        let context = TypeContractContext::FunctionParameter {
            name: param.name.clone(),
        };
        let payload = arg_syntax.value_expression_payload_for(arg);
        let outcome = self.expected_type_for_expr_with_payload(
            value,
            expected.clone(),
            context.clone(),
            payload.as_ref(),
        )?;
        let requires_guard = matches!(outcome, ExpectedTypeOutcome::RequiresRuntimeGuard(_));
        self.compile_expr_with_expected_type_and_payload(value, expected, context, payload.as_ref())
            .map(|register| (register, requires_guard))
    }

    pub(in crate::compiler) fn compile_call_argument_value(
        &mut self,
        arg: &Argument,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<crate::Register> {
        let value = &arg.value;
        if let Some(value_payload) = arg_syntax.value_expression_payload_for(arg) {
            return self.compile_expr_with_payload(value, Some(&value_payload));
        }
        self.compile_expr(value)
    }
}

pub(super) fn resolve_script_call_arguments<'ast>(
    params: &[ParamHint],
    args: &'ast [Argument],
    call_span: Span,
    arg_syntax: CallArgumentSyntax<'_, 'ast>,
) -> Result<Vec<Option<&'ast Argument>>, Vec<Diagnostic>> {
    let mut slots = vec![None; params.len()];
    let mut slot_spans = vec![None; params.len()];
    let mut diagnostics = Vec::new();
    let mut next_positional = 0_usize;
    let mut seen_named = false;

    for arg in args {
        let arg_span = arg.value.span;
        let arg_name = arg_syntax.name_for(arg);
        let Some(index) = argument_index(
            params,
            arg_name.as_deref(),
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

pub(in crate::compiler) fn resolve_syntax_call_arguments(
    params: &[ParamHint],
    args: &[SyntaxCallArgument],
    call_span: Span,
) -> Result<Vec<Option<SyntaxCallArgument>>, Vec<Diagnostic>> {
    let mut slots = vec![None; params.len()];
    let mut slot_spans = vec![None; params.len()];
    let mut diagnostics = Vec::new();
    let mut next_positional = 0_usize;
    let mut seen_named = false;

    for arg in args {
        let Some(index) = argument_index(
            params,
            arg.name.as_deref(),
            arg.span,
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
                arg.span,
            ));
            continue;
        }
        slots[index] = Some(arg.clone());
        slot_spans[index] = Some(arg.span);
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
