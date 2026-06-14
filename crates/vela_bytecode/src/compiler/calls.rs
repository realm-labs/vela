use vela_syntax::ast::{Argument, Expr, ExprKind};

use crate::{CallArgument, DynamicCallArgument, UnlinkedInstructionKind};

use super::call_args::resolve_script_call_arguments;
use super::methods::host_method_call;
use super::record_shapes::{ValueShape, callback_param_shapes};
use super::value_types::{RuntimeTypeFact, TypeContractContext, type_hint_value_type};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, reject_named_args};
use vela_common::{Diagnostic, HostMethodId, Span};
use vela_def::{DefPath, FunctionId, MethodId, TypeId};
use vela_hir::type_hint::ParamHint;
use vela_registry::ParamDef;

impl Compiler<'_, '_> {
    pub(super) fn compile_call_expr(
        &mut self,
        expr: &Expr,
        callee: &Expr,
        args: &[Argument],
    ) -> CompileResult<crate::Register> {
        if let Some((enum_name, variant)) = self.tuple_enum_constructor_call(callee) {
            let fields =
                self.compile_tuple_variant_fields(callee.span, &enum_name, &variant, args)?;
            let dst = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::MakeEnum {
                dst,
                enum_name,
                variant,
                fields,
            });
            return Ok(dst);
        }

        let host_receiver_type = self.host_method_receiver_type(callee);
        let path_root_is_local = path_root_is_local(callee, &self.locals);
        if let Some(call) = host_method_call(
            self,
            callee,
            host_receiver_type.as_deref(),
            path_root_is_local,
        ) {
            let root = self.compile_host_path_root(call.receiver)?;
            let arg_registers = self.compile_host_method_call_args(call.method, args, expr.span)?;
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

        if let Some(remove) = self.host_path_remove_call(callee, args)? {
            return Ok(remove);
        }

        if let Some(push) = self.host_path_push_call(callee, args)? {
            return Ok(push);
        }

        if let ExprKind::Field { base, name } = &callee.kind {
            return self.compile_script_method_call(expr, base, name, args);
        }
        if let Some((method, receiver_path)) = local_path_method_call(callee, &self.locals) {
            return self.compile_script_path_method_call(expr, callee, receiver_path, method, args);
        }

        let dst = self.alloc_register()?;
        if let Some((declaration, name)) = self.script_function_call(callee) {
            let call_args = self.compile_script_call_args(declaration, args, callee.span)?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallFunction {
                    dst,
                    target: function_id_for_script_name(&name),
                    name,
                    mode: call_args.mode,
                    args: call_args.args,
                },
                expr.span,
            );
        } else if self.local_callee(callee).is_some() || !matches!(callee.kind, ExprKind::Path(_)) {
            reject_named_args(args, "closure call")?;
            let callee = self.compile_expr(callee)?;
            let args = args
                .iter()
                .map(|arg| self.compile_expr(&arg.value))
                .collect::<CompileResult<Vec<_>>>()?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallClosure { dst, callee, args },
                expr.span,
            );
        } else {
            let fallback_name = callable_name(callee)?;
            let native = self.resolve_native_function_id(&fallback_name, callee.span)?;
            let arg_registers =
                self.compile_native_call_args(&fallback_name, native, args, callee.span)?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallNative {
                    dst: Some(dst),
                    name: fallback_name,
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
    ) -> CompileResult<Vec<crate::Register>> {
        let has_named_args = args.iter().any(|arg| arg.name.is_some());
        let registry_params = self
            .facts
            .registry
            .and_then(|registry| registry.host_method_params_by_runtime_id(method.get()));
        let Some(params) = registry_params else {
            reject_named_args(args, "host method call")?;
            return args
                .iter()
                .map(|arg| self.compile_expr(&arg.value))
                .collect();
        };
        if !has_named_args {
            return args
                .iter()
                .map(|arg| self.compile_expr(&arg.value))
                .collect();
        }
        let params = registry_param_hints(params, call_span);
        self.compile_metadata_register_args(&params, args, call_span)
    }

    fn compile_script_method_call(
        &mut self,
        expr: &Expr,
        base: &Expr,
        name: &str,
        args: &[Argument],
    ) -> CompileResult<crate::Register> {
        let receiver_type = self.script_type_for_expr(base);
        let receiver_shape = self.value_shape_for_expr(base);
        let value_receiver_type = self
            .value_type_for_expr(base)
            .or_else(|| receiver_shape.as_ref().and_then(ValueShape::value_type));
        let method_id = receiver_type
            .as_deref()
            .and_then(|type_name| self.script_method_id_for_type(type_name, name));
        let value_method_id = value_receiver_type
            .as_ref()
            .and_then(|type_name| self.value_method_id_for_type(type_name, name));
        let value_receiver_methods_known = value_receiver_type
            .as_ref()
            .is_some_and(|receiver_type| self.registry_value_type_id(receiver_type).is_some());
        let receiver = self.compile_expr(base)?;
        let dst = self.alloc_register()?;
        if let Some(method_id) = method_id {
            let arg_registers = self.compile_script_method_call_args(
                receiver_type.as_deref(),
                value_receiver_type.as_ref(),
                receiver_shape.as_ref(),
                name,
                args,
                expr.span,
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
                receiver_type.as_deref(),
                value_receiver_type.as_ref(),
                receiver_shape.as_ref(),
                name,
                args,
                expr.span,
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
            let args = self.compile_dynamic_method_call_args(args)?;
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

    fn compile_script_path_method_call(
        &mut self,
        expr: &Expr,
        callee: &Expr,
        receiver_path: &[String],
        method: &str,
        args: &[Argument],
    ) -> CompileResult<crate::Register> {
        let receiver_type = self.script_type_for_receiver_path(receiver_path);
        let receiver_shape = self.value_shape_for_receiver_path(receiver_path);
        let value_receiver_type = self
            .value_type_for_receiver_path(receiver_path)
            .or_else(|| receiver_shape.as_ref().and_then(ValueShape::value_type));
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
                receiver_type.as_deref(),
                value_receiver_type.as_ref(),
                receiver_shape.as_ref(),
                method,
                args,
                expr.span,
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
                receiver_type.as_deref(),
                value_receiver_type.as_ref(),
                receiver_shape.as_ref(),
                method,
                args,
                expr.span,
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
            let args = self.compile_dynamic_method_call_args(args)?;
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
        receiver_type: Option<&str>,
        value_receiver_type: Option<&RuntimeTypeFact>,
        receiver_shape: Option<&ValueShape>,
        method: &str,
        args: &[Argument],
        call_span: vela_common::Span,
    ) -> CompileResult<Vec<CallArgument>> {
        let Some(receiver_type) = receiver_type else {
            return self.compile_value_method_call_args(
                value_receiver_type,
                receiver_shape,
                method,
                args,
                call_span,
            );
        };
        let Some(params) = self.script_method_params(receiver_type, method) else {
            return self.compile_value_method_call_args(
                value_receiver_type,
                receiver_shape,
                method,
                args,
                call_span,
            );
        };
        let params = params.into_iter().skip(1).collect::<Vec<_>>();
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
    ) -> CompileResult<Vec<DynamicCallArgument>> {
        args.iter()
            .map(|arg| {
                Ok(DynamicCallArgument {
                    name: arg.name.clone(),
                    value: self.compile_expr(&arg.value)?,
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
    ) -> CompileResult<Vec<CallArgument>> {
        let registry_params = self.registry_value_method_params(receiver_type, method);
        let Some(params) = registry_params else {
            reject_named_args(args, "script method call")?;
            return self.compile_positional_method_args(receiver_shape, method, args);
        };
        if !args.iter().any(|arg| arg.name.is_some()) {
            return self.compile_positional_method_args(receiver_shape, method, args);
        }
        let params = registry_param_hints(params, call_span);
        let slots =
            resolve_script_call_arguments(&params, args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;

        let mut registers = Vec::new();
        for (slot, param) in slots.into_iter().zip(params) {
            if let Some(arg) = slot {
                registers.push(CallArgument::Register(self.compile_method_arg(
                    receiver_shape,
                    method,
                    arg,
                )?));
            } else if param.default_value_span.is_none() {
                unreachable!("call argument resolver rejects missing required arguments");
            }
        }
        Ok(registers)
    }

    fn compile_positional_method_args(
        &mut self,
        receiver_shape: Option<&ValueShape>,
        method: &str,
        args: &[Argument],
    ) -> CompileResult<Vec<CallArgument>> {
        args.iter()
            .map(|arg| {
                self.compile_method_arg(receiver_shape, method, arg)
                    .map(CallArgument::Register)
            })
            .collect()
    }

    fn compile_method_arg(
        &mut self,
        receiver_shape: Option<&ValueShape>,
        method: &str,
        arg: &Argument,
    ) -> CompileResult<crate::Register> {
        let ExprKind::Lambda { params, body } = &arg.value.kind else {
            return self.compile_expr(&arg.value);
        };
        let Some(receiver_shape) = receiver_shape else {
            return self.compile_expr(&arg.value);
        };
        let Some(param_shapes) = callback_param_shapes(receiver_shape, method, params.len()) else {
            return self.compile_expr(&arg.value);
        };
        self.compile_lambda_with_callback_shapes(&arg.value, params, body, &param_shapes)
    }

    fn compile_metadata_register_args(
        &mut self,
        params: &[ParamHint],
        args: &[Argument],
        call_span: Span,
    ) -> CompileResult<Vec<crate::Register>> {
        let slots =
            resolve_script_call_arguments(params, args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;

        let mut registers = Vec::new();
        for (slot, param) in slots.into_iter().zip(params) {
            if let Some(arg) = slot {
                registers.push(self.compile_argument_for_param(&arg.value, param)?.0);
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
    ) -> CompileResult<Vec<crate::Register>> {
        let registry_params = self
            .facts
            .registry
            .and_then(|registry| registry.function_params(native));
        let Some(params) = registry_params else {
            reject_named_args(args, "native call")?;
            return args
                .iter()
                .map(|arg| self.compile_expr(&arg.value))
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
        if !args.iter().any(|arg| arg.name.is_some()) {
            return args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    if let Some(param) = params.get(index) {
                        self.compile_native_argument_for_param(
                            name,
                            u16::try_from(index).unwrap_or(u16::MAX),
                            &arg.value,
                            param,
                        )
                    } else {
                        self.compile_expr(&arg.value)
                    }
                })
                .collect();
        }

        let slots =
            resolve_script_call_arguments(&params, args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;

        let mut registers = Vec::new();
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            if let Some(arg) = slot {
                registers.push(self.compile_native_argument_for_param(
                    name,
                    u16::try_from(index).unwrap_or(u16::MAX),
                    &arg.value,
                    param,
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
        value: &Expr,
        param: &ParamHint,
    ) -> CompileResult<crate::Register> {
        let Some(expected) = param.type_hint.as_ref().and_then(type_hint_value_type) else {
            return self.compile_expr(value);
        };
        self.compile_expr_with_expected_type(
            value,
            expected,
            TypeContractContext::NativeParameter {
                function: function.to_owned(),
                name: param.name.clone(),
                index,
            },
        )
    }

    fn resolve_native_function_id(&self, name: &str, call_span: Span) -> CompileResult<FunctionId> {
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

    fn value_method_id_for_type(
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

    fn registry_value_method_params(
        &self,
        receiver_type: Option<&RuntimeTypeFact>,
        method: &str,
    ) -> Option<&[ParamDef]> {
        let registry = self.facts.registry?;
        let owner = self.registry_value_type_id(receiver_type?)?;
        let method = registry.resolve_value_method(owner, method)?;
        registry.method_params(method)
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

fn function_id_for_script_name(name: &str) -> FunctionId {
    function_id_for_path("script", name)
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

fn registry_param_hints(params: &[ParamDef], call_span: Span) -> Vec<ParamHint> {
    params
        .iter()
        .map(|param| ParamHint {
            name: param.name.clone(),
            span: call_span,
            type_hint: param
                .type_hint
                .as_ref()
                .map(|hint| registry_type_hint(hint, call_span)),
            default_value_span: param.has_default.then_some(call_span),
        })
        .collect()
}

fn registry_type_hint(hint: &str, span: Span) -> vela_hir::type_hint::HirTypeHint {
    vela_hir::type_hint::HirTypeHint {
        path: hint.split("::").map(str::to_owned).collect(),
        args: Vec::new(),
        span,
    }
}

fn local_path_method_call<'expr>(
    callee: &'expr Expr,
    locals: &std::collections::HashMap<String, crate::Register>,
) -> Option<(&'expr str, &'expr [String])> {
    let ExprKind::Path(path) = &callee.kind else {
        return None;
    };
    let (method, receiver_path) = path.split_last()?;
    (!receiver_path.is_empty() && locals.contains_key(&receiver_path[0]))
        .then_some((method.as_str(), receiver_path))
}

fn path_root_is_local(
    callee: &Expr,
    locals: &std::collections::HashMap<String, crate::Register>,
) -> bool {
    let ExprKind::Path(path) = &callee.kind else {
        return false;
    };
    path.first().is_some_and(|root| locals.contains_key(root))
}

fn callable_name(callee: &Expr) -> CompileResult<String> {
    match &callee.kind {
        ExprKind::Path(path) => Ok(path.join("::")),
        _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "callable expression",
        ))),
    }
}

fn unresolved_static_method_error(method: &str, span: Span) -> CompileError {
    CompileError::new(CompileErrorKind::SemanticDiagnostics(vec![
        Diagnostic::error(format!("unresolved method `{method}`"))
            .with_code("compiler::unresolved_method")
            .with_span(span)
            .with_label(span, "method is not defined for the known receiver type"),
    ]))
}
