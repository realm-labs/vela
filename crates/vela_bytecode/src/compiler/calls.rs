use vela_syntax::{Argument, Expr, ExprKind};

use crate::InstructionKind;

use super::methods::host_method_call;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, reject_named_args};

impl Compiler<'_> {
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
        if let Some(call) =
            host_method_call(&self.facts.options, callee, host_receiver_type.as_deref())
        {
            reject_named_args(args, "host method call")?;
            let root = self.compile_host_path_root(callee.span, call.receiver)?;
            let segments = self.compile_host_path_segments(call.segments)?;
            let arg_registers = args
                .iter()
                .map(|arg| self.compile_expr(&arg.value))
                .collect::<CompileResult<Vec<_>>>()?;
            let dst = self.alloc_register()?;
            self.emit_spanned(
                InstructionKind::CallHostMethod {
                    dst: Some(dst),
                    root,
                    segments,
                    method: call.method,
                    args: arg_registers,
                },
                expr.span,
            );
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
            reject_named_args(args, "native call")?;
            let arg_registers = args
                .iter()
                .map(|arg| self.compile_expr(&arg.value))
                .collect::<CompileResult<Vec<_>>>()?;
            self.emit_spanned(
                InstructionKind::CallNative {
                    dst: Some(dst),
                    name: fallback_name,
                    args: arg_registers,
                },
                expr.span,
            );
        }
        Ok(dst)
    }

    fn compile_script_method_call(
        &mut self,
        expr: &Expr,
        base: &Expr,
        name: &str,
        args: &[Argument],
    ) -> CompileResult<crate::Register> {
        reject_named_args(args, "script method call")?;
        let method_id = self.script_method_id_for_receiver(base, name);
        let receiver = self.compile_expr(base)?;
        let arg_registers = args
            .iter()
            .map(|arg| self.compile_expr(&arg.value))
            .collect::<CompileResult<Vec<_>>>()?;
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
        reject_named_args(args, "script method call")?;
        let method_id = self.script_method_id_for_receiver_path(receiver_path, method);
        let receiver = self.compile_path_expr(callee.span, receiver_path)?;
        let arg_registers = args
            .iter()
            .map(|arg| self.compile_expr(&arg.value))
            .collect::<CompileResult<Vec<_>>>()?;
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
                    args: arg_registers,
                },
                expr.span,
            );
        }
        Ok(dst)
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

fn callable_name(callee: &Expr) -> CompileResult<String> {
    match &callee.kind {
        ExprKind::Path(path) => Ok(path.join(".")),
        _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "callable expression",
        ))),
    }
}
