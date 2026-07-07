use vela_common::{HostMethodId, SourceId, Span};
use vela_hir::ids::HirDeclId;
use vela_hir::type_hint::ParamHint;
use vela_syntax::ast::{SyntaxArgument, SyntaxExpression};

use crate::compiler::body_payloads::expression_syntax_literal;
use crate::compiler::call_args::{
    ScriptCallArgs, SyntaxCallArgument, resolve_syntax_call_arguments,
};
use crate::compiler::calls::metadata::registry_param_hints;
use crate::compiler::calls::{mutation_arg_debug_name, typed_container_mutation_arg_contract};
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::record_shapes::{ValueShape, callback_param_shapes};
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, StandardRuntimeType, TypeContractContext,
    check_expected_type, type_hint_value_type,
};
use crate::compiler::{
    CompileError, CompileErrorKind, CompileResult, Compiler, type_guard_plan_for_runtime_type,
};
use crate::{
    CallArgument, DynamicCallArgument, FunctionId, GuardKind, Register, ScriptCallMode,
    UnlinkedGuardContext, UnlinkedInstructionKind, UnlinkedTypeGuard,
};

use super::syntax_statement_values::syntax_expression_span;

struct SyntaxMethodArgument<'a> {
    source: SourceId,
    receiver_type: Option<&'a RuntimeTypeFact>,
    receiver_shape: Option<&'a ValueShape>,
    method: &'a str,
    param_name: &'a str,
    position: usize,
    expression: &'a SyntaxExpression,
    param: Option<&'a ParamHint>,
}

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
                            None,
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
                    None,
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

    pub(in crate::compiler::control_flow) fn compile_syntax_value_method_call_arguments(
        &mut self,
        source: SourceId,
        receiver_shape: Option<&ValueShape>,
        receiver_type: Option<&RuntimeTypeFact>,
        method: &str,
        arguments: &[SyntaxArgument],
        call_span: Span,
    ) -> CompileResult<Option<Vec<CallArgument>>> {
        let registry_params = self.registry_value_method_params(receiver_type, method);
        let Some(params) = registry_params else {
            if arguments
                .iter()
                .any(|argument| argument.name_text().is_some())
            {
                return Ok(None);
            }
            return arguments
                .iter()
                .enumerate()
                .map(|(index, argument)| {
                    let Some(expression) = argument.expression() else {
                        return Ok(None);
                    };
                    let register = self.compile_syntax_method_argument(SyntaxMethodArgument {
                        source,
                        receiver_type,
                        receiver_shape,
                        method,
                        param_name: "",
                        position: index,
                        expression: &expression,
                        param: None,
                    })?;
                    Ok(register.map(CallArgument::Register))
                })
                .collect::<CompileResult<Option<Vec<_>>>>();
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
                        self.compile_syntax_method_argument(SyntaxMethodArgument {
                            source,
                            receiver_type,
                            receiver_shape,
                            method,
                            param_name: param.name.as_str(),
                            position: index,
                            expression: &expression,
                            param: Some(param),
                        })?
                    } else {
                        self.compile_syntax_expression(source, &expression)?
                    };
                    Ok(register.map(CallArgument::Register))
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
                let Some(register) = self.compile_syntax_method_argument(SyntaxMethodArgument {
                    source,
                    receiver_type,
                    receiver_shape,
                    method,
                    param_name: param.name.as_str(),
                    position: index,
                    expression: &arg.value,
                    param: Some(param),
                })?
                else {
                    return Ok(None);
                };
                registers.push(CallArgument::Register(register));
            } else if param.default_value_span.is_none() {
                unreachable!("syntax call argument resolver rejects missing required arguments");
            }
        }
        Ok(Some(registers))
    }

    fn compile_syntax_method_argument(
        &mut self,
        argument: SyntaxMethodArgument<'_>,
    ) -> CompileResult<Option<Register>> {
        let SyntaxMethodArgument {
            source,
            receiver_type,
            receiver_shape,
            method,
            param_name,
            position,
            expression,
            param,
        } = argument;
        if let Some(expected) =
            typed_container_mutation_arg_contract(receiver_type, method, param_name, position)
        {
            return self.compile_syntax_expression_for_expected_type(
                source,
                expression,
                expected,
                TypeContractContext::NativeParameter {
                    function: method.to_owned(),
                    name: mutation_arg_debug_name(method, param_name, position),
                    index: u16::try_from(position).unwrap_or(u16::MAX),
                },
                &[],
            );
        }

        let Some(param) = param else {
            return self.compile_syntax_expression(source, expression);
        };
        let callback_shapes = syntax_callback_param_shapes(receiver_shape, method, expression);
        self.compile_syntax_argument_for_param(
            source,
            method,
            u16::try_from(position).unwrap_or(u16::MAX),
            expression,
            param,
            callback_shapes.as_deref(),
        )
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_script_function_call_arguments(
        &mut self,
        source: SourceId,
        declaration: HirDeclId,
        arguments: &[SyntaxArgument],
        call_span: Span,
    ) -> CompileResult<Option<ScriptCallArgs>> {
        let params = self
            .facts
            .script_function_signatures
            .get(&declaration)
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("script call")))?
            .clone();
        let syntax_args = syntax_call_arguments(source, arguments);
        let slots = resolve_syntax_call_arguments(&params, &syntax_args, call_span).map_err(
            |diagnostics| CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics)),
        )?;

        let mut mode = ScriptCallMode::Unchecked;
        let mut args = Vec::new();
        for (slot, param) in slots.into_iter().zip(params.iter()) {
            if let Some(arg) = slot {
                let Some((register, requires_guard)) =
                    self.compile_syntax_script_argument_for_param(source, &arg.value, param)?
                else {
                    return Ok(None);
                };
                if requires_guard {
                    mode = ScriptCallMode::Checked;
                }
                args.push(CallArgument::Register(register));
            } else if param.default_value_span.is_some() {
                if param.type_hint.is_some() {
                    mode = ScriptCallMode::Checked;
                }
                args.push(CallArgument::Missing);
            } else {
                unreachable!("syntax call argument resolver rejects missing required arguments");
            }
        }
        Ok(Some(ScriptCallArgs { args, mode }))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_script_method_call_arguments(
        &mut self,
        source: SourceId,
        receiver_type: &str,
        method: &str,
        arguments: &[SyntaxArgument],
        call_span: Span,
    ) -> CompileResult<Option<Vec<CallArgument>>> {
        let Some(params) = self.script_method_params(receiver_type, method) else {
            return self.compile_syntax_value_method_call_arguments(
                source, None, None, method, arguments, call_span,
            );
        };
        let params = params.into_iter().skip(1).collect::<Vec<_>>();
        let syntax_args = syntax_call_arguments(source, arguments);
        let slots = resolve_syntax_call_arguments(&params, &syntax_args, call_span).map_err(
            |diagnostics| CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics)),
        )?;

        let mut registers = Vec::new();
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            if let Some(arg) = slot {
                let Some(register) = self.compile_syntax_argument_for_param(
                    source,
                    method,
                    u16::try_from(index).unwrap_or(u16::MAX),
                    &arg.value,
                    param,
                    None,
                )?
                else {
                    return Ok(None);
                };
                registers.push(CallArgument::Register(register));
            } else if param.default_value_span.is_some() {
                registers.push(CallArgument::Missing);
            } else {
                unreachable!("syntax call argument resolver rejects missing required arguments");
            }
        }
        Ok(Some(registers))
    }

    fn compile_syntax_script_argument_for_param(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        param: &ParamHint,
    ) -> CompileResult<Option<(Register, bool)>> {
        let Some(expected) = param.type_hint.as_ref().and_then(type_hint_value_type) else {
            return self
                .compile_syntax_expression(source, expression)
                .map(|register| register.map(|register| (register, false)));
        };
        let span = syntax_expression_span(source, expression);
        let context = TypeContractContext::FunctionParameter {
            name: param.name.clone(),
        };
        let expected_is_function =
            expected == RuntimeTypeFact::Standard(StandardRuntimeType::Function);
        let static_type = self.syntax_static_type_for_expression(Some(source), expression);
        let outcome = check_expected_type(static_type, expected, span, context.clone())?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some(literal) = expression_syntax_literal(expression)
            && let Some(constant) = compile_literal_constant_for_type(&literal, *tag)
                .map_err(|error| error.with_span(span))?
        {
            let register = self.emit_constant(constant)?;
            return Ok(Some((
                register,
                matches!(outcome, ExpectedTypeOutcome::RequiresRuntimeGuard(_)),
            )));
        }
        if expected_is_function
            && let Some(register) =
                self.compile_syntax_lambda_with_callback_shapes(source, expression, &[])?
        {
            return Ok(Some((register, false)));
        }
        let Some(register) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        let requires_guard = matches!(outcome, ExpectedTypeOutcome::RequiresRuntimeGuard(_));
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
        Ok(Some((register, requires_guard)))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_host_method_call_arguments(
        &mut self,
        source: SourceId,
        method: HostMethodId,
        arguments: &[SyntaxArgument],
        call_span: Span,
    ) -> CompileResult<Option<Vec<Register>>> {
        let registry_params = self
            .facts
            .registry
            .and_then(|registry| registry.host_method_params_by_runtime_id(method.get()));
        let Some(params) = registry_params else {
            if arguments
                .iter()
                .any(|argument| argument.name_text().is_some())
            {
                return Ok(None);
            }
            return self.compile_syntax_call_arguments(source, arguments);
        };
        if arguments
            .iter()
            .all(|argument| argument.name_text().is_none())
        {
            return self.compile_syntax_call_arguments(source, arguments);
        }

        let params = registry_param_hints(params, call_span);
        let syntax_args = syntax_call_arguments(source, arguments);
        let slots = resolve_syntax_call_arguments(&params, &syntax_args, call_span).map_err(
            |diagnostics| CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics)),
        )?;
        let mut registers = Vec::new();
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            if let Some(arg) = slot {
                let Some(register) = self.compile_syntax_argument_for_param(
                    source,
                    "host method",
                    u16::try_from(index).unwrap_or(u16::MAX),
                    &arg.value,
                    param,
                    None,
                )?
                else {
                    return Ok(None);
                };
                registers.push(register);
            } else if param.default_value_span.is_none() {
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
        callback_shapes: Option<&[Option<ValueShape>]>,
    ) -> CompileResult<Option<Register>> {
        let Some(expected) = param.type_hint.as_ref().and_then(type_hint_value_type) else {
            return self.compile_syntax_expression(source, expression);
        };
        let context = TypeContractContext::NativeParameter {
            function: function.to_owned(),
            name: param.name.clone(),
            index,
        };
        self.compile_syntax_expression_for_expected_type(
            source,
            expression,
            expected,
            context,
            callback_shapes.unwrap_or(&[]),
        )
    }

    fn compile_syntax_expression_for_expected_type(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        expected: RuntimeTypeFact,
        context: TypeContractContext,
        callback_shapes: &[Option<ValueShape>],
    ) -> CompileResult<Option<Register>> {
        let span = syntax_expression_span(source, expression);
        let expected_is_function =
            expected == RuntimeTypeFact::Standard(StandardRuntimeType::Function);
        let static_type = self.syntax_static_type_for_expression(Some(source), expression);
        let outcome = check_expected_type(static_type, expected, span, context.clone())?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some(literal) = expression_syntax_literal(expression)
            && let Some(constant) = compile_literal_constant_for_type(&literal, *tag)
                .map_err(|error| error.with_span(span))?
        {
            return self.emit_constant(constant).map(Some);
        }
        if expected_is_function
            && let Some(register) = self.compile_syntax_lambda_with_callback_shapes(
                source,
                expression,
                callback_shapes,
            )?
        {
            return Ok(Some(register));
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

fn syntax_callback_param_shapes(
    receiver_shape: Option<&ValueShape>,
    method: &str,
    expression: &SyntaxExpression,
) -> Option<Vec<Option<ValueShape>>> {
    let param_count = expression.as_lambda()?.param_list()?.params().count();
    callback_param_shapes(receiver_shape?, method, param_count)
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
