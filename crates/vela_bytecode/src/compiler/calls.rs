mod callee;
pub(in crate::compiler) mod metadata;
mod payload_guards;

use vela_syntax::ast::{Argument, Expr, ExprKind, SyntaxArgument, SyntaxExpressionKind};

use crate::{CallArgument, DynamicCallArgument, UnlinkedInstructionKind};

use super::body_payloads::{CompilerArgumentPayload, CompilerExpressionPayload};
use super::call_args::{CallArgumentSyntax, resolve_script_call_arguments};
#[cfg(not(test))]
use super::expression_facts::ExpressionFacts;
#[cfg(test)]
use super::expression_facts::expression_facts;
use super::expression_facts::payload_matches_expression_facts;
use super::methods::host_method_call;
use super::record_shapes::{ValueShape, callback_param_shapes};
use super::value_types::{RuntimeTypeFact, TypeContractContext, type_hint_value_type};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};
use callee::{
    callable_name, callee_is_closure_call, callee_path_segments, local_path_method_call,
    path_root_is_local,
};
use metadata::{registry_param_hints, registry_type_hint, unresolved_static_method_error};
use payload_guards::{
    callback_lambda_payload_is_authoritative, reject_mismatched_call_argument_payloads,
    reject_mismatched_call_callee_payload, reject_missing_call_callee_payload,
};
use vela_common::{Diagnostic, HostMethodId, PrimitiveTag, Span};
use vela_def::{DefPath, FunctionId, MethodId, TypeId};
use vela_hir::type_hint::ParamHint;
use vela_registry::ParamDef;

#[derive(Clone, Copy)]
struct MethodCallFacts<'facts> {
    receiver_type: Option<&'facts str>,
    value_receiver_type: Option<&'facts RuntimeTypeFact>,
    receiver_shape: Option<&'facts ValueShape>,
}

#[derive(Clone, Copy)]
struct MethodArgContext<'facts> {
    receiver_type: Option<&'facts RuntimeTypeFact>,
    receiver_shape: Option<&'facts ValueShape>,
    method: &'facts str,
    param_name: &'facts str,
    position: usize,
}

impl Compiler<'_, '_> {
    pub(super) fn compile_call_expr(
        &mut self,
        expr: &Expr,
        callee: &Expr,
        args: &[Argument],
    ) -> CompileResult<crate::Register> {
        self.compile_call_expr_with_arg_payloads(expr, callee, args, None, None)
    }

    pub(in crate::compiler) fn compile_call_expr_with_arg_payloads(
        &mut self,
        expr: &Expr,
        callee: &Expr,
        args: &[Argument],
        callee_payload: Option<&CompilerExpressionPayload<'_>>,
        arg_payloads: Option<&[CompilerArgumentPayload]>,
    ) -> CompileResult<crate::Register> {
        let arg_syntax = CallArgumentSyntax::new(args, arg_payloads);
        reject_missing_call_callee_payload(callee_payload)?;
        #[cfg(test)]
        let callee_facts = expression_facts(callee);
        #[cfg(not(test))]
        let callee_facts = ExpressionFacts::span_only(callee.span);
        reject_mismatched_call_callee_payload(callee_facts, callee_payload)?;
        reject_mismatched_call_argument_payloads(callee_payload, args.len(), arg_payloads)?;
        let callee_path = callee_payload.and_then(CompilerExpressionPayload::syntax_path_segments);
        let callee_path = callee_path.as_deref();
        let callee_span = match callee_payload {
            Some(payload) => payload.syntax_span().ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST call callee",
                ))
            })?,
            None => callee.span,
        };
        let has_authoritative_callee_path = callee_path_segments(callee_path).is_some();
        if has_authoritative_callee_path
            && let Some(path) = callee_path_segments(callee_path)
            && let Some((enum_name, variant)) =
                self.tuple_enum_constructor_call_at_span(path, callee_span)
        {
            let Some((source, syntax_args)) = arg_syntax.syntax_arguments() else {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST tuple variant argument payloads",
                )));
            };
            let Some(fields) = self.compile_syntax_tuple_variant_fields(
                source,
                callee_span,
                &enum_name,
                &variant,
                &syntax_args,
            )?
            else {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST tuple variant argument payloads",
                )));
            };
            let dst = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::MakeEnum {
                dst,
                enum_name,
                variant,
                fields,
            });
            return Ok(dst);
        }

        let host_receiver_type = self.host_method_receiver_type(callee_payload);
        let path_root_is_local = path_root_is_local(callee_path, &self.locals);
        if let Some(call) = host_method_call(
            self,
            callee_payload,
            host_receiver_type.as_deref(),
            path_root_is_local,
        ) {
            let root = self.compile_host_path_root(&call.receiver)?;
            let arg_registers =
                self.compile_host_method_call_args(call.method, args, expr.span, arg_syntax)?;
            let dst = self.alloc_register()?;
            self.emit_host_call(
                Some(dst),
                root,
                super::host_paths::HostPath {
                    root: call.receiver,
                    segments: call.segments,
                },
                call.method,
                arg_registers,
                expr.span,
            )?;
            return Ok(dst);
        }

        if let Some(remove) = self.host_path_remove_call(callee, callee_payload, args)? {
            return Ok(remove);
        }

        if let Some(push) = self.host_path_push_call(callee, callee_payload, args, arg_syntax)? {
            return Ok(push);
        }

        if let Some(script_method_call) = self.compile_script_method_call_from_callee(
            expr,
            callee,
            args,
            callee_payload,
            arg_syntax,
        ) {
            return script_method_call;
        }
        if let Some((method, receiver_path)) = local_path_method_call(callee_path, &self.locals) {
            return self.compile_script_path_method_call(
                expr,
                callee,
                receiver_path,
                method,
                args,
                arg_syntax,
            );
        }

        let dst = self.alloc_register()?;
        let script_function_call = has_authoritative_callee_path
            .then(|| self.script_function_call_at_span(callee_span))
            .flatten();
        if let Some((declaration, name)) = script_function_call {
            let call_args = self.compile_script_call_args_with_payloads(
                declaration,
                args,
                callee_span,
                arg_syntax,
            )?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallFunction {
                    dst,
                    target: crate::function_id_for_script_name(&name),
                    name,
                    mode: call_args.mode,
                    args: call_args.args,
                },
                expr.span,
            );
        } else if self.local_callee_at_span(callee_span).is_some()
            || callee_is_closure_call(callee_payload)
        {
            reject_named_call_args(arg_syntax, "closure call")?;
            let callee = self.compile_expr_with_payload(callee, callee_payload)?;
            let args = args
                .iter()
                .map(|arg| self.compile_call_argument_value(arg, arg_syntax))
                .collect::<CompileResult<Vec<_>>>()?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallClosure { dst, callee, args },
                expr.span,
            );
        } else {
            let callee_name = callable_name(callee_path)?;
            if callee_name == "set::from_array" {
                reject_named_call_args(arg_syntax, "set::from_array")?;
                if args.len() != 1 {
                    return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                        vec![
                            Diagnostic::error(format!(
                                "set::from_array expects 1 argument, got {}",
                                args.len()
                            ))
                            .with_code("compiler::arity")
                            .with_span(callee.span),
                        ],
                    )));
                }
                let src = self.compile_call_argument_value(&args[0], arg_syntax)?;
                self.emit_spanned(
                    UnlinkedInstructionKind::MakeSetFromArray { dst, src },
                    expr.span,
                );
                return Ok(dst);
            }
            let native = self.resolve_native_function_id(&callee_name, callee.span)?;
            let arg_registers =
                self.compile_native_call_args(&callee_name, native, args, callee.span, arg_syntax)?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallNative {
                    dst: Some(dst),
                    name: callee_name,
                    native,
                    cache_site: None,
                    args: arg_registers,
                },
                expr.span,
            );
        }
        Ok(dst)
    }

    fn compile_host_method_call_args(
        &mut self,
        method: HostMethodId,
        args: &[Argument],
        call_span: Span,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<Vec<crate::Register>> {
        let has_named_args = arg_syntax.has_named_args();
        let registry_params = self
            .facts
            .registry
            .and_then(|registry| registry.host_method_params_by_runtime_id(method.get()));
        let Some(params) = registry_params else {
            reject_named_call_args(arg_syntax, "host method call")?;
            return args
                .iter()
                .map(|arg| self.compile_call_argument_value(arg, arg_syntax))
                .collect();
        };
        if !has_named_args {
            return args
                .iter()
                .map(|arg| self.compile_call_argument_value(arg, arg_syntax))
                .collect();
        }
        let params = registry_param_hints(params, call_span);
        self.compile_metadata_register_args(&params, args, call_span, arg_syntax)
    }

    fn compile_script_method_call(
        &mut self,
        expr: &Expr,
        base: &Expr,
        name: &str,
        args: &[Argument],
        base_payload: Option<&CompilerExpressionPayload<'_>>,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<crate::Register> {
        let receiver_type = self.script_type_for_expression_payload(base_payload);
        let receiver_shape = self.value_shape_for_expression_payload(base_payload);
        let value_receiver_type = self
            .value_type_for_expression_payload(base_payload)
            .or_else(|| receiver_shape.as_ref().and_then(ValueShape::value_type));
        self.reject_static_array_ordering_method_without_ord(
            name,
            arg_syntax,
            value_receiver_type.as_ref(),
            receiver_shape.as_ref(),
            expr.span,
        )?;
        let method_id = receiver_type
            .as_deref()
            .and_then(|type_name| self.script_method_id_for_type(type_name, name));
        let value_method_id = value_receiver_type
            .as_ref()
            .and_then(|type_name| self.value_method_id_for_type(type_name, name));
        let value_receiver_methods_known = value_receiver_type
            .as_ref()
            .is_some_and(|receiver_type| self.registry_value_type_id(receiver_type).is_some());
        let receiver = self.compile_expr_with_payload(base, base_payload)?;
        let dst = self.alloc_register()?;
        if let Some(method_id) = method_id {
            let arg_registers = self.compile_script_method_call_args(
                MethodCallFacts {
                    receiver_type: receiver_type.as_deref(),
                    value_receiver_type: value_receiver_type.as_ref(),
                    receiver_shape: receiver_shape.as_ref(),
                },
                name,
                args,
                expr.span,
                arg_syntax,
            )?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallMethodId {
                    dst,
                    receiver,
                    method: name.to_owned(),
                    method_id,
                    args: arg_registers,
                },
                expr.span,
            );
        } else if let Some(method_id) = value_method_id {
            let arg_registers = self.compile_script_method_call_args(
                MethodCallFacts {
                    receiver_type: receiver_type.as_deref(),
                    value_receiver_type: value_receiver_type.as_ref(),
                    receiver_shape: receiver_shape.as_ref(),
                },
                name,
                args,
                expr.span,
                arg_syntax,
            )?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallMethodId {
                    dst,
                    receiver,
                    method: name.to_owned(),
                    method_id,
                    args: arg_registers,
                },
                expr.span,
            );
        } else if receiver_type.is_some() || value_receiver_methods_known {
            return Err(unresolved_static_method_error(name, expr.span));
        } else {
            let args = self.compile_dynamic_method_call_args(args, arg_syntax)?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallDynamicMethod {
                    dst,
                    receiver,
                    method: name.to_owned(),
                    args,
                },
                expr.span,
            );
        }
        Ok(dst)
    }

    fn compile_script_method_call_from_callee(
        &mut self,
        expr: &Expr,
        callee: &Expr,
        args: &[Argument],
        callee_payload: Option<&CompilerExpressionPayload<'_>>,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> Option<CompileResult<crate::Register>> {
        let payload = callee_payload?;
        if !matches!(
            payload.syntax_kind(),
            Some(SyntaxExpressionKind::Field) | None
        ) {
            return None;
        }
        let name = payload.syntax_field_name()?;
        let ExprKind::Field { base, .. } = &callee.kind else {
            return None;
        };
        let base_payload = payload.field_base_payload()?;
        #[cfg(test)]
        let base_facts = expression_facts(base);
        #[cfg(not(test))]
        let base_facts = ExpressionFacts::span_only(base.span);
        if !payload_matches_expression_facts(&base_payload, base_facts) {
            return None;
        }
        Some(self.compile_script_method_call(
            expr,
            base,
            &name,
            args,
            Some(&base_payload),
            arg_syntax,
        ))
    }

    fn compile_script_path_method_call(
        &mut self,
        expr: &Expr,
        callee: &Expr,
        receiver_path: &[String],
        method: &str,
        args: &[Argument],
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<crate::Register> {
        let receiver_type = self.script_type_for_receiver_path(receiver_path);
        let receiver_shape = self.value_shape_for_receiver_path(receiver_path);
        let value_receiver_type = self
            .value_type_for_receiver_path(receiver_path)
            .or_else(|| receiver_shape.as_ref().and_then(ValueShape::value_type));
        self.reject_static_array_ordering_method_without_ord(
            method,
            arg_syntax,
            value_receiver_type.as_ref(),
            receiver_shape.as_ref(),
            expr.span,
        )?;
        let method_id = receiver_type
            .as_deref()
            .and_then(|type_name| self.script_method_id_for_type(type_name, method));
        let value_method_id = value_receiver_type
            .as_ref()
            .and_then(|type_name| self.value_method_id_for_type(type_name, method));
        let value_receiver_methods_known = value_receiver_type
            .as_ref()
            .is_some_and(|receiver_type| self.registry_value_type_id(receiver_type).is_some());
        let receiver = self.compile_path_expr(callee.span, receiver_path)?;
        let dst = self.alloc_register()?;
        if let Some(method_id) = method_id {
            let arg_registers = self.compile_script_method_call_args(
                MethodCallFacts {
                    receiver_type: receiver_type.as_deref(),
                    value_receiver_type: value_receiver_type.as_ref(),
                    receiver_shape: receiver_shape.as_ref(),
                },
                method,
                args,
                expr.span,
                arg_syntax,
            )?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallMethodId {
                    dst,
                    receiver,
                    method: method.to_owned(),
                    method_id,
                    args: arg_registers,
                },
                expr.span,
            );
        } else if let Some(method_id) = value_method_id {
            let arg_registers = self.compile_script_method_call_args(
                MethodCallFacts {
                    receiver_type: receiver_type.as_deref(),
                    value_receiver_type: value_receiver_type.as_ref(),
                    receiver_shape: receiver_shape.as_ref(),
                },
                method,
                args,
                expr.span,
                arg_syntax,
            )?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallMethodId {
                    dst,
                    receiver,
                    method: method.to_owned(),
                    method_id,
                    args: arg_registers,
                },
                expr.span,
            );
        } else if receiver_type.is_some() || value_receiver_methods_known {
            return Err(unresolved_static_method_error(method, expr.span));
        } else {
            let args = self.compile_dynamic_method_call_args(args, arg_syntax)?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallDynamicMethod {
                    dst,
                    receiver,
                    method: method.to_owned(),
                    args,
                },
                expr.span,
            );
        }
        Ok(dst)
    }

    fn compile_script_method_call_args(
        &mut self,
        facts: MethodCallFacts<'_>,
        method: &str,
        args: &[Argument],
        call_span: vela_common::Span,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<Vec<CallArgument>> {
        let Some(receiver_type) = facts.receiver_type else {
            return self.compile_value_method_call_args(
                facts.value_receiver_type,
                facts.receiver_shape,
                method,
                args,
                call_span,
                arg_syntax,
            );
        };
        let Some(params) = self.script_method_params(receiver_type, method) else {
            return self.compile_value_method_call_args(
                facts.value_receiver_type,
                facts.receiver_shape,
                method,
                args,
                call_span,
                arg_syntax,
            );
        };
        let params = params.into_iter().skip(1).collect::<Vec<_>>();
        let slots = resolve_script_call_arguments(&params, args, call_span, arg_syntax).map_err(
            |diagnostics| CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics)),
        )?;

        slots
            .into_iter()
            .zip(params)
            .map(|(slot, param)| {
                if let Some(arg) = slot {
                    self.compile_argument_for_param_with_payloads(arg, &param, arg_syntax)
                        .map(|(register, _)| CallArgument::Register(register))
                } else if param.default_value_span.is_some() {
                    Ok(CallArgument::Missing)
                } else {
                    unreachable!("call argument resolver rejects missing required arguments")
                }
            })
            .collect()
    }

    fn compile_dynamic_method_call_args(
        &mut self,
        args: &[Argument],
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<Vec<DynamicCallArgument>> {
        args.iter()
            .map(|arg| {
                Ok(DynamicCallArgument {
                    name: arg_syntax.name_for(arg),
                    value: self.compile_call_argument_value(arg, arg_syntax)?,
                })
            })
            .collect()
    }

    fn compile_value_method_call_args(
        &mut self,
        receiver_type: Option<&RuntimeTypeFact>,
        receiver_shape: Option<&ValueShape>,
        method: &str,
        args: &[Argument],
        call_span: Span,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<Vec<CallArgument>> {
        let registry_params = self.registry_value_method_params(receiver_type, method);
        let Some(params) = registry_params else {
            reject_named_call_args(arg_syntax, "script method call")?;
            return self.compile_positional_method_args(
                receiver_type,
                receiver_shape,
                method,
                args,
                arg_syntax,
            );
        };
        if !arg_syntax.has_named_args() {
            return self.compile_positional_method_args(
                receiver_type,
                receiver_shape,
                method,
                args,
                arg_syntax,
            );
        }
        let params = registry_param_hints(params, call_span);
        let slots = resolve_script_call_arguments(&params, args, call_span, arg_syntax).map_err(
            |diagnostics| CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics)),
        )?;

        let mut registers = Vec::new();
        for (slot, param) in slots.into_iter().zip(params) {
            if let Some(arg) = slot {
                registers.push(CallArgument::Register(self.compile_method_arg(
                    MethodArgContext {
                        receiver_type,
                        receiver_shape,
                        method,
                        param_name: param.name.as_str(),
                        position: registers.len(),
                    },
                    arg,
                    arg_syntax,
                )?));
            } else if param.default_value_span.is_none() {
                unreachable!("call argument resolver rejects missing required arguments");
            }
        }
        Ok(registers)
    }

    fn compile_positional_method_args(
        &mut self,
        receiver_type: Option<&RuntimeTypeFact>,
        receiver_shape: Option<&ValueShape>,
        method: &str,
        args: &[Argument],
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<Vec<CallArgument>> {
        args.iter()
            .enumerate()
            .map(|(index, arg)| {
                self.compile_method_arg(
                    MethodArgContext {
                        receiver_type,
                        receiver_shape,
                        method,
                        param_name: "",
                        position: index,
                    },
                    arg,
                    arg_syntax,
                )
                .map(CallArgument::Register)
            })
            .collect()
    }

    fn compile_method_arg(
        &mut self,
        context: MethodArgContext<'_>,
        arg: &Argument,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<crate::Register> {
        if let Some(expected) = typed_container_mutation_arg_contract(
            context.receiver_type,
            context.method,
            context.param_name,
            context.position,
        ) {
            if arg_syntax.has_missing_value_payload(arg) {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST call argument value",
                )));
            }
            let payload = arg_syntax.value_expression_payload_for(arg);
            return self.compile_expr_with_expected_type_and_payload(
                &arg.value,
                expected,
                TypeContractContext::NativeParameter {
                    function: context.method.to_owned(),
                    name: mutation_arg_debug_name(
                        context.method,
                        context.param_name,
                        context.position,
                    ),
                    index: u16::try_from(context.position).unwrap_or(u16::MAX),
                },
                payload.as_ref(),
            );
        }
        let arg_payload = arg_syntax.value_expression_payload_for(arg);
        if !callback_lambda_payload_is_authoritative(
            arg_payload.as_ref(),
            arg.value.span,
            arg_payload
                .as_ref()
                .and_then(|payload| payload.syntax_kind()),
        ) {
            return self.compile_call_argument_value(arg, arg_syntax);
        }
        let Some(payload) = arg_payload.as_ref() else {
            return self.compile_call_argument_value(arg, arg_syntax);
        };
        let Some(source) = payload.source() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST callback lambda payload",
            )));
        };
        let Some(expression) = payload.syntax_expression() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST callback lambda payload",
            )));
        };
        let Some(lambda) = expression.as_lambda() else {
            return self.compile_call_argument_value(arg, arg_syntax);
        };
        let Some(param_list) = lambda.param_list() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST callback lambda parameters",
            )));
        };
        if lambda.body().is_none() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST lambda body",
            )));
        }
        let Some(receiver_shape) = context.receiver_shape else {
            return self.compile_call_argument_value(arg, arg_syntax);
        };
        let param_count = param_list.params().count();
        let Some(param_shapes) = callback_param_shapes(receiver_shape, context.method, param_count)
        else {
            return self.compile_call_argument_value(arg, arg_syntax);
        };
        self.compile_syntax_lambda_with_callback_shapes(source, expression, &param_shapes)?
            .ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "mismatched CST callback lambda payload",
                ))
            })
    }

    fn compile_metadata_register_args(
        &mut self,
        params: &[ParamHint],
        args: &[Argument],
        call_span: Span,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<Vec<crate::Register>> {
        let slots = resolve_script_call_arguments(params, args, call_span, arg_syntax).map_err(
            |diagnostics| CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics)),
        )?;

        let mut registers = Vec::new();
        for (slot, param) in slots.into_iter().zip(params) {
            if let Some(arg) = slot {
                registers.push(
                    self.compile_argument_for_param_with_payloads(arg, param, arg_syntax)?
                        .0,
                );
            } else if param.default_value_span.is_none() {
                unreachable!("call argument resolver rejects missing required arguments");
            }
        }
        Ok(registers)
    }

    fn compile_native_call_args(
        &mut self,
        name: &str,
        native: FunctionId,
        args: &[Argument],
        call_span: vela_common::Span,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<Vec<crate::Register>> {
        let registry_params = self
            .facts
            .registry
            .and_then(|registry| registry.function_params(native));
        let Some(params) = registry_params else {
            reject_named_call_args(arg_syntax, "native call")?;
            return args
                .iter()
                .map(|arg| self.compile_call_argument_value(arg, arg_syntax))
                .collect();
        };
        let params = params
            .iter()
            .map(|param| ParamHint {
                name: param.name.clone(),
                span: call_span,
                type_hint: param
                    .type_hint
                    .as_ref()
                    .map(|hint| registry_type_hint(hint, call_span)),
                default_value_span: None,
            })
            .collect::<Vec<_>>();
        if !arg_syntax.has_named_args() {
            return args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    if let Some(param) = params.get(index) {
                        self.compile_native_argument_for_param(
                            name,
                            u16::try_from(index).unwrap_or(u16::MAX),
                            arg,
                            param,
                            arg_syntax,
                        )
                    } else {
                        self.compile_call_argument_value(arg, arg_syntax)
                    }
                })
                .collect();
        }

        let slots = resolve_script_call_arguments(&params, args, call_span, arg_syntax).map_err(
            |diagnostics| CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics)),
        )?;

        let mut registers = Vec::new();
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            if let Some(arg) = slot {
                registers.push(self.compile_native_argument_for_param(
                    name,
                    u16::try_from(index).unwrap_or(u16::MAX),
                    arg,
                    param,
                    arg_syntax,
                )?);
            } else {
                unreachable!("native call argument resolver rejects missing required arguments");
            }
        }
        Ok(registers)
    }

    fn compile_native_argument_for_param(
        &mut self,
        function: &str,
        index: u16,
        arg: &Argument,
        param: &ParamHint,
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<crate::Register> {
        let value = &arg.value;
        let Some(expected) = param.type_hint.as_ref().and_then(type_hint_value_type) else {
            return self.compile_call_argument_value(arg, arg_syntax);
        };
        if arg_syntax.has_missing_value_payload(arg) {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST call argument value",
            )));
        }
        let payload = arg_syntax.value_expression_payload_for(arg);
        self.compile_expr_with_expected_type_and_payload(
            value,
            expected,
            TypeContractContext::NativeParameter {
                function: function.to_owned(),
                name: param.name.clone(),
                index,
            },
            payload.as_ref(),
        )
    }

    pub(in crate::compiler) fn resolve_native_function_id(
        &self,
        name: &str,
        call_span: Span,
    ) -> CompileResult<FunctionId> {
        let Some(registry) = self.facts.registry else {
            return Ok(function_id_for_native_name(name));
        };
        if let Some(id) = registry.resolve_native_function_name(name) {
            return Ok(id);
        }

        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            vec![
                Diagnostic::error(format!("unresolved native function `{name}`"))
                    .with_code("compiler::unresolved_native_function")
                    .with_span(call_span)
                    .with_label(call_span, "native function is not registered"),
            ],
        )))
    }

    pub(in crate::compiler) fn value_method_id_for_type(
        &self,
        receiver_type: &RuntimeTypeFact,
        method: &str,
    ) -> Option<MethodId> {
        if let Some(registry) = self.facts.registry {
            let owner = self.registry_value_type_id(receiver_type)?;
            return registry.resolve_value_method(owner, method);
        }
        None
    }

    pub(in crate::compiler) fn registry_value_method_params(
        &self,
        receiver_type: Option<&RuntimeTypeFact>,
        method: &str,
    ) -> Option<&[ParamDef]> {
        let registry = self.facts.registry?;
        let owner = self.registry_value_type_id(receiver_type?)?;
        let method = registry.resolve_value_method(owner, method)?;
        registry.method_params(method)
    }

    pub(in crate::compiler) fn value_methods_known_for_type(
        &self,
        receiver_type: &RuntimeTypeFact,
    ) -> bool {
        self.registry_value_type_id(receiver_type).is_some()
    }

    fn registry_value_type_id(&self, receiver_type: &RuntimeTypeFact) -> Option<TypeId> {
        let registry = self.facts.registry?;
        if let RuntimeTypeFact::Primitive(primitive) = receiver_type
            && let Some(id) = registry.primitive_type_id(*primitive)
        {
            return Some(id);
        }
        let type_name = receiver_type.std_type_name();
        registry.resolve_type(&DefPath::ty("std", std::iter::empty::<&str>(), type_name))
    }
}

impl Compiler<'_, '_> {
    fn reject_static_array_ordering_method_without_ord(
        &self,
        method: &str,
        arg_syntax: CallArgumentSyntax<'_, '_>,
        receiver_type: Option<&RuntimeTypeFact>,
        receiver_shape: Option<&ValueShape>,
        span: Span,
    ) -> CompileResult<()> {
        if !matches!(method, "sort" | "sort_by" | "min" | "max") {
            return Ok(());
        }
        if method == "sort_by" {
            let Some(receiver_shape) = receiver_shape else {
                return Ok(());
            };
            let Some((source, args)) = arg_syntax.syntax_arguments() else {
                return Ok(());
            };
            let Some(key_shape) =
                self.syntax_callback_return_shape(receiver_shape, method, &args, Some(source))
            else {
                return Ok(());
            };
            return self.reject_static_ord_shape(method, &key_shape, span);
        }
        if let Some(RuntimeTypeFact::Array(element)) = receiver_type
            && !runtime_type_satisfies_ord(element)
        {
            return Err(missing_array_ord_error(
                method,
                "element",
                &element.source_type_display(),
                span,
            ));
        }
        let Some(ValueShape::Array(element)) = receiver_shape else {
            return Ok(());
        };
        let Some(type_name) = element.as_record().and_then(|record| record.type_name()) else {
            return Ok(());
        };
        if !self.is_declared_script_type(type_name)
            || self.type_implements_builtin_trait_method(type_name, "Ord", "cmp")
        {
            return Ok(());
        }
        Err(missing_array_ord_error(method, "element", type_name, span))
    }

    fn reject_static_ord_shape(
        &self,
        method: &str,
        shape: &ValueShape,
        span: Span,
    ) -> CompileResult<()> {
        if let Some(value_type) = shape.value_type() {
            if !runtime_type_satisfies_ord(&value_type) {
                return Err(missing_array_ord_error(
                    method,
                    "key",
                    &value_type.source_type_display(),
                    span,
                ));
            }
            return Ok(());
        }
        let Some(type_name) = shape.as_record().and_then(|record| record.type_name()) else {
            return Ok(());
        };
        if !self.is_declared_script_type(type_name)
            || self.type_implements_builtin_trait_method(type_name, "Ord", "cmp")
        {
            return Ok(());
        }
        Err(missing_array_ord_error(method, "key", type_name, span))
    }

    pub(in crate::compiler) fn reject_static_syntax_array_ordering_method_without_ord(
        &self,
        source: vela_common::SourceId,
        method: &str,
        args: &[SyntaxArgument],
        receiver_type: Option<&RuntimeTypeFact>,
        receiver_shape: Option<&ValueShape>,
        span: Span,
    ) -> CompileResult<()> {
        if !matches!(method, "sort" | "sort_by" | "min" | "max") {
            return Ok(());
        }
        if method == "sort_by" {
            let Some(receiver_shape) = receiver_shape else {
                return Ok(());
            };
            let Some(key_shape) =
                self.syntax_callback_return_shape(receiver_shape, method, args, Some(source))
            else {
                return Ok(());
            };
            return self.reject_static_ord_shape(method, &key_shape, span);
        }
        if let Some(RuntimeTypeFact::Array(element)) = receiver_type
            && !runtime_type_satisfies_ord(element)
        {
            return Err(missing_array_ord_error(
                method,
                "element",
                &element.source_type_display(),
                span,
            ));
        }
        let Some(ValueShape::Array(element)) = receiver_shape else {
            return Ok(());
        };
        let Some(type_name) = element.as_record().and_then(|record| record.type_name()) else {
            return Ok(());
        };
        if !self.is_declared_script_type(type_name)
            || self.type_implements_builtin_trait_method(type_name, "Ord", "cmp")
        {
            return Ok(());
        }
        Err(missing_array_ord_error(method, "element", type_name, span))
    }
}

fn runtime_type_satisfies_ord(fact: &RuntimeTypeFact) -> bool {
    matches!(
        fact,
        RuntimeTypeFact::Primitive(
            PrimitiveTag::Bool
                | PrimitiveTag::Char
                | PrimitiveTag::I8
                | PrimitiveTag::I16
                | PrimitiveTag::I32
                | PrimitiveTag::I64
                | PrimitiveTag::U8
                | PrimitiveTag::U16
                | PrimitiveTag::U32
                | PrimitiveTag::U64
                | PrimitiveTag::String
                | PrimitiveTag::Bytes
        )
    )
}

fn missing_array_ord_error(
    method: &str,
    value_kind: &str,
    value_type: &str,
    span: Span,
) -> CompileError {
    CompileError::new(CompileErrorKind::SemanticDiagnostics(vec![
        Diagnostic::error(format!(
            "`Array.{method}` requires an `Ord` {value_kind}, but `{value_type}` does not implement `Ord`"
        ))
        .with_code("compiler::missing_ord_for_array_ordering")
        .with_span(span)
        .with_label(span, format!("static `Array.{method}` requires `Ord`"))
        .with_label(
            span,
            format!("add `impl Ord for {value_type}` or use a dynamic value"),
        ),
    ]))
}

pub(in crate::compiler) fn typed_container_mutation_arg_contract(
    receiver_type: Option<&RuntimeTypeFact>,
    method: &str,
    param_name: &str,
    position: usize,
) -> Option<RuntimeTypeFact> {
    match receiver_type? {
        RuntimeTypeFact::Array(element) => {
            match (method, mutation_arg_role(method, param_name, position)) {
                ("push" | "insert", MutationArgRole::Value) => Some((**element).clone()),
                ("extend", MutationArgRole::Values) => {
                    Some(RuntimeTypeFact::array((**element).clone()))
                }
                _ => None,
            }
        }
        RuntimeTypeFact::Map { key, value } => {
            match (method, mutation_arg_role(method, param_name, position)) {
                ("set", MutationArgRole::Key)
                    if !matches!(
                        key.as_ref(),
                        RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::String)
                    ) =>
                {
                    Some((**key).clone())
                }
                ("set", MutationArgRole::Value) => Some((**value).clone()),
                ("extend", MutationArgRole::Values) => {
                    Some(RuntimeTypeFact::map((**key).clone(), (**value).clone()))
                }
                _ => None,
            }
        }
        RuntimeTypeFact::Set(element) => {
            match (method, mutation_arg_role(method, param_name, position)) {
                ("add", MutationArgRole::Value) => Some((**element).clone()),
                ("extend", MutationArgRole::Values) => {
                    Some(RuntimeTypeFact::set((**element).clone()))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MutationArgRole {
    Key,
    Value,
    Values,
    Other,
}

fn mutation_arg_role(method: &str, param_name: &str, position: usize) -> MutationArgRole {
    match param_name {
        "key" => MutationArgRole::Key,
        "value" => MutationArgRole::Value,
        "values" => MutationArgRole::Values,
        _ => match (method, position) {
            ("set", 0) => MutationArgRole::Key,
            ("set", 1) | ("insert", 1) | ("push", 0) | ("add", 0) => MutationArgRole::Value,
            ("extend", 0) => MutationArgRole::Values,
            _ => MutationArgRole::Other,
        },
    }
}

pub(in crate::compiler) fn mutation_arg_debug_name(
    method: &str,
    param_name: &str,
    position: usize,
) -> String {
    if param_name.is_empty() {
        match mutation_arg_role(method, param_name, position) {
            MutationArgRole::Key => "key",
            MutationArgRole::Value => "value",
            MutationArgRole::Values => "values",
            _ => "argument",
        }
        .to_owned()
    } else {
        param_name.to_owned()
    }
}

fn function_id_for_native_name(name: &str) -> FunctionId {
    if let Some((module, function)) = name.rsplit_once("::")
        && let Some(id) = vela_stdlib::std_function_id(module, function)
    {
        return id;
    }
    function_id_for_path("host", name)
}

fn function_id_for_path(package: &str, name: &str) -> FunctionId {
    let mut segments = name.split("::").collect::<Vec<_>>();
    let function = segments.pop().unwrap_or(name);
    FunctionId::from_def_id(DefPath::function(package, segments, function).id())
}

fn reject_named_call_args(
    arg_syntax: CallArgumentSyntax<'_, '_>,
    context: &'static str,
) -> CompileResult<()> {
    if arg_syntax.has_named_args() {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            context,
        )));
    }
    Ok(())
}
