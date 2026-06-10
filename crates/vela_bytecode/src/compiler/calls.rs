use vela_syntax::ast::{Argument, Expr, ExprKind};

use crate::{CallArgument, InstructionKind};

use super::call_args::resolve_script_call_arguments;
use super::methods::host_method_call;
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
            self.emit(InstructionKind::MakeEnum {
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
            let args = self.compile_script_call_args(declaration, args, callee.span)?;
            self.emit_spanned(InstructionKind::CallFunction { dst, name, args }, expr.span);
        } else if self.local_callee(callee).is_some() || !matches!(callee.kind, ExprKind::Path(_)) {
            reject_named_args(args, "closure call")?;
            let callee = self.compile_expr(callee)?;
            let args = args
                .iter()
                .map(|arg| self.compile_expr(&arg.value))
                .collect::<CompileResult<Vec<_>>>()?;
            self.emit_spanned(
                InstructionKind::CallClosure { dst, callee, args },
                expr.span,
            );
        } else {
            let fallback_name = callable_name(callee)?;
            let native = self.resolve_native_function_id(&fallback_name, callee.span)?;
            let arg_registers =
                self.compile_native_call_args(&fallback_name, native, args, callee.span)?;
            self.emit_spanned(
                InstructionKind::CallNative {
                    dst: Some(dst),
                    name: fallback_name,
                    native,
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
        let value_receiver_type = self.value_type_for_expr(base);
        let method_id = receiver_type
            .as_deref()
            .and_then(|type_name| self.script_method_id_for_type(type_name, name));
        let arg_registers = self.compile_script_method_call_args(
            receiver_type.as_deref(),
            value_receiver_type.as_deref(),
            name,
            args,
            expr.span,
        )?;
        let value_method_id = value_receiver_type
            .as_deref()
            .and_then(|type_name| self.value_method_id_for_type(type_name, name));
        let receiver = self.compile_expr(base)?;
        let dst = self.alloc_register()?;
        if let Some(method_id) = method_id {
            self.emit_spanned(
                InstructionKind::CallMethodId {
                    dst,
                    receiver,
                    method: name.to_owned(),
                    method_id,
                    args: arg_registers,
                },
                expr.span,
            );
        } else {
            self.emit_spanned(
                InstructionKind::CallMethod {
                    dst,
                    receiver,
                    method: name.to_owned(),
                    value_method_id,
                    args: arg_registers,
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
        let value_receiver_type = self.value_type_for_receiver_path(receiver_path);
        let method_id = receiver_type
            .as_deref()
            .and_then(|type_name| self.script_method_id_for_type(type_name, method));
        let arg_registers = self.compile_script_method_call_args(
            receiver_type.as_deref(),
            value_receiver_type.as_deref().or(receiver_type.as_deref()),
            method,
            args,
            expr.span,
        )?;
        let value_method_id = value_receiver_type
            .as_deref()
            .and_then(|type_name| self.value_method_id_for_type(type_name, method));
        let receiver = self.compile_path_expr(callee.span, receiver_path)?;
        let dst = self.alloc_register()?;
        if let Some(method_id) = method_id {
            self.emit_spanned(
                InstructionKind::CallMethodId {
                    dst,
                    receiver,
                    method: method.to_owned(),
                    method_id,
                    args: arg_registers,
                },
                expr.span,
            );
        } else {
            self.emit_spanned(
                InstructionKind::CallMethod {
                    dst,
                    receiver,
                    method: method.to_owned(),
                    value_method_id,
                    args: arg_registers,
                },
                expr.span,
            );
        }
        Ok(dst)
    }

    fn compile_script_method_call_args(
        &mut self,
        receiver_type: Option<&str>,
        value_receiver_type: Option<&str>,
        method: &str,
        args: &[Argument],
        call_span: vela_common::Span,
    ) -> CompileResult<Vec<CallArgument>> {
        let Some(receiver_type) = receiver_type else {
            return self.compile_value_method_call_args(
                value_receiver_type,
                method,
                args,
                call_span,
            );
        };
        let Some(params) = self.script_method_params(receiver_type, method) else {
            return self.compile_value_method_call_args(
                value_receiver_type.or(Some(receiver_type)),
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
                    self.compile_expr(&arg.value).map(CallArgument::Register)
                } else if param.default_value_span.is_some() {
                    Ok(CallArgument::Missing)
                } else {
                    unreachable!("call argument resolver rejects missing required arguments")
                }
            })
            .collect()
    }

    fn compile_value_method_call_args(
        &mut self,
        receiver_type: Option<&str>,
        method: &str,
        args: &[Argument],
        call_span: Span,
    ) -> CompileResult<Vec<CallArgument>> {
        let registry_params = self.registry_value_method_params(receiver_type, method);
        let Some(params) = registry_params else {
            reject_named_args(args, "script method call")?;
            return self.compile_positional_method_args(args);
        };
        if !args.iter().any(|arg| arg.name.is_some()) {
            return self.compile_positional_method_args(args);
        }
        let params = registry_param_hints(params, call_span);
        let slots =
            resolve_script_call_arguments(&params, args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;

        let mut registers = Vec::new();
        for (slot, param) in slots.into_iter().zip(params) {
            if let Some(arg) = slot {
                registers.push(CallArgument::Register(self.compile_expr(&arg.value)?));
            } else if param.default_value_span.is_none() {
                unreachable!("call argument resolver rejects missing required arguments");
            }
        }
        Ok(registers)
    }

    fn compile_positional_method_args(
        &mut self,
        args: &[Argument],
    ) -> CompileResult<Vec<CallArgument>> {
        args.iter()
            .map(|arg| self.compile_expr(&arg.value).map(CallArgument::Register))
            .collect()
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
                registers.push(self.compile_expr(&arg.value)?);
            } else if param.default_value_span.is_none() {
                unreachable!("call argument resolver rejects missing required arguments");
            }
        }
        Ok(registers)
    }

    fn compile_native_call_args(
        &mut self,
        _name: &str,
        native: Option<FunctionId>,
        args: &[Argument],
        call_span: vela_common::Span,
    ) -> CompileResult<Vec<crate::Register>> {
        let has_named_args = args.iter().any(|arg| arg.name.is_some());
        let registry_params = self
            .facts
            .registry
            .and_then(|registry| native.and_then(|id| registry.function_params(id)));
        let Some(params) = registry_params else {
            reject_named_args(args, "native call")?;
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
        let params = params
            .iter()
            .map(|param| ParamHint {
                name: param.name.clone(),
                span: call_span,
                type_hint: None,
                default_value_span: None,
            })
            .collect::<Vec<_>>();
        self.compile_metadata_register_args(&params, args, call_span)
    }

    fn resolve_native_function_id(
        &self,
        name: &str,
        call_span: Span,
    ) -> CompileResult<Option<FunctionId>> {
        let Some(registry) = self.facts.registry else {
            return Ok(None);
        };
        if let Some(id) = registry.resolve_native_function_name(name) {
            return Ok(Some(id));
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

    fn value_method_id_for_type(&self, receiver_type: &str, method: &str) -> Option<MethodId> {
        if let Some(registry) = self.facts.registry {
            let owner = self.registry_value_type_id(receiver_type)?;
            return registry.resolve_value_method(owner, method);
        }
        None
    }

    fn registry_value_method_params(
        &self,
        receiver_type: Option<&str>,
        method: &str,
    ) -> Option<&[ParamDef]> {
        let registry = self.facts.registry?;
        let owner = self.registry_value_type_id(receiver_type?)?;
        let method = registry.resolve_value_method(owner, method)?;
        registry.method_params(method)
    }

    fn registry_value_type_id(&self, receiver_type: &str) -> Option<TypeId> {
        let registry = self.facts.registry?;
        let type_name = standard_value_type_name(receiver_type)?;
        registry.resolve_type(&DefPath::ty("std", std::iter::empty::<&str>(), type_name))
    }
}

fn registry_param_hints(params: &[ParamDef], call_span: Span) -> Vec<ParamHint> {
    params
        .iter()
        .map(|param| ParamHint {
            name: param.name.clone(),
            span: call_span,
            type_hint: None,
            default_value_span: param.has_default.then_some(call_span),
        })
        .collect()
}

fn standard_value_type_name(receiver_type: &str) -> Option<&'static str> {
    match receiver_type {
        "null" => Some("Null"),
        "bool" => Some("Bool"),
        "int" => Some("Int"),
        "float" => Some("Float"),
        "string" => Some("String"),
        "array" => Some("Array"),
        "map" => Some("Map"),
        "set" => Some("Set"),
        "function" => Some("Function"),
        "closure" => Some("Closure"),
        "range" => Some("Range"),
        "Option" => Some("Option"),
        "Result" => Some("Result"),
        _ => None,
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
