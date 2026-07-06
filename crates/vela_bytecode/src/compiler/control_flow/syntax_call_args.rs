use vela_common::{SourceId, Span};
use vela_hir::type_hint::ParamHint;
use vela_syntax::ast::{SyntaxArgument, SyntaxExpression};

use crate::compiler::body_payloads::expression_syntax_literal;
use crate::compiler::call_args::{SyntaxCallArgument, resolve_syntax_call_arguments};
use crate::compiler::calls::registry_param_hints;
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type,
    type_hint_value_type,
};
use crate::compiler::{
    CompileError, CompileErrorKind, CompileResult, Compiler, type_guard_plan_for_runtime_type,
};
use crate::{
    DynamicCallArgument, FunctionId, GuardKind, Register, UnlinkedGuardContext,
    UnlinkedInstructionKind, UnlinkedTypeGuard,
};

use super::syntax_statement_values::syntax_expression_span;

impl Compiler<'_, '_> {
    pub(in crate::compiler::control_flow) fn compile_syntax_call_arguments(
        &mut self,
        source: SourceId,
        arguments: &[SyntaxArgument],
    ) -> CompileResult<Option<Vec<Register>>> {
        arguments
            .iter()
            .map(|argument| {
                let Some(expression) = argument.expression() else {
                    return Ok(None);
                };
                self.compile_syntax_expression(source, &expression)
            })
            .collect::<CompileResult<Option<Vec<_>>>>()
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_native_call_arguments(
        &mut self,
        source: SourceId,
        name: &str,
        native: FunctionId,
        arguments: &[SyntaxArgument],
        call_span: Span,
    ) -> CompileResult<Option<Vec<Register>>> {
        let registry_params = self
            .facts
            .registry
            .and_then(|registry| registry.function_params(native));
        let Some(params) = registry_params else {
            if arguments
                .iter()
                .any(|argument| argument.name_text().is_some())
            {
                return Ok(None);
            }
            return self.compile_syntax_call_arguments(source, arguments);
        };
        let params = registry_param_hints(params, call_span);
        if arguments
            .iter()
            .all(|argument| argument.name_text().is_none())
        {
            return arguments
                .iter()
                .enumerate()
                .map(|(index, argument)| {
                    let Some(expression) = argument.expression() else {
                        return Ok(None);
                    };
                    let register = if let Some(param) = params.get(index) {
                        self.compile_syntax_argument_for_param(
                            source,
                            name,
                            u16::try_from(index).unwrap_or(u16::MAX),
                            &expression,
                            param,
                        )?
                    } else {
                        self.compile_syntax_expression(source, &expression)?
                    };
                    Ok(register)
                })
                .collect::<CompileResult<Option<Vec<_>>>>();
        }

        let syntax_args = syntax_call_arguments(source, arguments);
        let slots = resolve_syntax_call_arguments(&params, &syntax_args, call_span).map_err(
            |diagnostics| CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics)),
        )?;
        let mut registers = Vec::new();
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            if let Some(arg) = slot {
                let Some(register) = self.compile_syntax_argument_for_param(
                    source,
                    name,
                    u16::try_from(index).unwrap_or(u16::MAX),
                    &arg.value,
                    param,
                )?
                else {
                    return Ok(None);
                };
                registers.push(register);
            } else {
                unreachable!("syntax call argument resolver rejects missing required arguments");
            }
        }
        Ok(Some(registers))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_dynamic_call_arguments(
        &mut self,
        source: SourceId,
        arguments: &[SyntaxArgument],
    ) -> CompileResult<Option<Vec<DynamicCallArgument>>> {
        arguments
            .iter()
            .map(|argument| {
                let Some(expression) = argument.expression() else {
                    return Ok(None);
                };
                let Some(value) = self.compile_syntax_expression(source, &expression)? else {
                    return Ok(None);
                };
                Ok(Some(DynamicCallArgument {
                    name: argument.name_text(),
                    value,
                }))
            })
            .collect::<CompileResult<Option<Vec<_>>>>()
    }

    fn compile_syntax_argument_for_param(
        &mut self,
        source: SourceId,
        function: &str,
        index: u16,
        expression: &SyntaxExpression,
        param: &ParamHint,
    ) -> CompileResult<Option<Register>> {
        let Some(expected) = param.type_hint.as_ref().and_then(type_hint_value_type) else {
            return self.compile_syntax_expression(source, expression);
        };
        let span = syntax_expression_span(source, expression);
        let context = TypeContractContext::NativeParameter {
            function: function.to_owned(),
            name: param.name.clone(),
            index,
        };
        let static_type = self
            .syntax_value_type_for_expression(Some(source), expression)
            .map(StaticExprType::Exact)
            .unwrap_or(StaticExprType::Dynamic);
        let outcome = check_expected_type(static_type, expected, span, context.clone())?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some(literal) = expression_syntax_literal(expression)
            && let Some(constant) = compile_literal_constant_for_type(&literal, *tag)
                .map_err(|error| error.with_span(span))?
        {
            return self.emit_constant(constant).map(Some);
        }
        let Some(register) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = type_guard_plan_for_runtime_type(expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(GuardKind::Contract, location, name),
                    ),
                },
                span,
            );
        }
        Ok(Some(register))
    }
}

fn syntax_call_arguments(
    source: SourceId,
    arguments: &[SyntaxArgument],
) -> Vec<SyntaxCallArgument> {
    arguments
        .iter()
        .filter_map(|argument| {
            let value = argument.expression()?;
            Some(SyntaxCallArgument {
                name: argument.name_text(),
                span: syntax_expression_span(source, &value),
                value,
            })
        })
        .collect()
}
