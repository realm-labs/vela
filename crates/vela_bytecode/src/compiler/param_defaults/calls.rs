use vela_common::{Diagnostic, SourceId, Span};
use vela_hir::binding::BindingResolution;
use vela_hir::ids::HirDeclId;
use vela_hir::type_hint::ParamHint;
use vela_syntax::ast::{AstNode, SyntaxCallExpr, SyntaxExpression};

use crate::{CallArgument, Register, ScriptCallMode, UnlinkedInstructionKind};

use crate::compiler::call_args::{
    ScriptCallArgs, SyntaxCallArgument, resolve_syntax_call_arguments,
};
use crate::compiler::calls::{function_id_for_script_name, registry_param_hints};
use crate::compiler::value_types::{
    ExpectedTypeOutcome, TypeContractContext, check_expected_type, type_hint_value_type,
};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};

use super::{
    param_default_cst_lowering_covers, param_default_unsupported, span_for, span_for_range,
};

impl Compiler<'_, '_> {
    pub(super) fn compile_param_default_call(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        call: &SyntaxCallExpr,
    ) -> CompileResult<Register> {
        let Some(callee) = call.callee() else {
            return Err(param_default_unsupported(source, expression));
        };
        let Some(path) = callee.as_path() else {
            return Err(param_default_unsupported(source, expression));
        };
        let path = path.path_segments();
        if path.is_empty() {
            return Err(param_default_unsupported(source, expression));
        }
        let call_span = span_for(source, expression);
        let callee_span = span_for(source, &callee);
        let args = syntax_call_arguments(source, call)?;
        let dst = self.alloc_register()?;

        if let Some((declaration, name)) = self.script_function_call_at_span(callee_span) {
            let call_args =
                self.compile_param_default_script_call_args(source, declaration, &args, call_span)?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallFunction {
                    dst,
                    target: function_id_for_script_name(&name),
                    name,
                    mode: call_args.mode,
                    args: call_args.args,
                },
                call_span,
            );
            return Ok(dst);
        }

        let name = path.join("::");
        if name == "set::from_array" {
            reject_named_syntax_call_args(&args, "set::from_array")?;
            let [arg] = args.as_slice() else {
                return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                    vec![
                        Diagnostic::error(format!(
                            "set::from_array expects 1 argument, got {}",
                            args.len()
                        ))
                        .with_code("compiler::arity")
                        .with_span(callee_span),
                    ],
                )));
            };
            let src = self.compile_param_default_expression(source, &arg.value)?;
            self.emit_spanned(
                UnlinkedInstructionKind::MakeSetFromArray { dst, src },
                call_span,
            );
            return Ok(dst);
        }

        let native = self.resolve_native_function_id(&name, callee_span)?;
        let arg_registers =
            self.compile_param_default_native_call_args(&name, native, source, &args, call_span)?;
        self.emit_spanned(
            UnlinkedInstructionKind::CallNative {
                dst: Some(dst),
                name,
                native,
                cache_site: None,
                args: arg_registers,
            },
            call_span,
        );
        Ok(dst)
    }

    fn script_function_call_at_span(&self, span: Span) -> Option<(HirDeclId, String)> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.resolution_at_span(span)
        else {
            return None;
        };
        self.facts
            .script_function_symbols
            .get(declaration)
            .cloned()
            .map(|name| (*declaration, name))
    }

    fn compile_param_default_script_call_args(
        &mut self,
        source: SourceId,
        declaration: HirDeclId,
        args: &[SyntaxCallArgument],
        call_span: Span,
    ) -> CompileResult<ScriptCallArgs> {
        let params = self
            .facts
            .script_function_signatures
            .get(&declaration)
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("script call")))?
            .clone();
        let slots =
            resolve_syntax_call_arguments(&params, args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;

        let mut mode = ScriptCallMode::Unchecked;
        let args = slots
            .into_iter()
            .zip(params)
            .map(|(slot, param)| {
                if let Some(arg) = slot {
                    let (register, requires_guard) =
                        self.compile_param_default_argument_for_param(source, &arg, &param)?;
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

    fn compile_param_default_native_call_args(
        &mut self,
        name: &str,
        native: vela_def::FunctionId,
        source: SourceId,
        args: &[SyntaxCallArgument],
        call_span: Span,
    ) -> CompileResult<Vec<Register>> {
        let registry_params = self
            .facts
            .registry
            .and_then(|registry| registry.function_params(native));
        let Some(params) = registry_params else {
            reject_named_syntax_call_args(args, "native call")?;
            return args
                .iter()
                .map(|arg| self.compile_param_default_expression(source, &arg.value))
                .collect();
        };
        let params = registry_param_hints(params, call_span);
        if !syntax_call_args_have_names(args) {
            return args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    if let Some(param) = params.get(index) {
                        self.compile_param_default_native_argument_for_param(
                            name,
                            u16::try_from(index).unwrap_or(u16::MAX),
                            source,
                            arg,
                            param,
                        )
                    } else {
                        self.compile_param_default_expression(source, &arg.value)
                    }
                })
                .collect();
        }

        let slots =
            resolve_syntax_call_arguments(&params, args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;
        let mut registers = Vec::new();
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            if let Some(arg) = slot {
                registers.push(self.compile_param_default_native_argument_for_param(
                    name,
                    u16::try_from(index).unwrap_or(u16::MAX),
                    source,
                    &arg,
                    param,
                )?);
            } else {
                unreachable!("native call argument resolver rejects missing required arguments");
            }
        }
        Ok(registers)
    }

    fn compile_param_default_argument_for_param(
        &mut self,
        source: SourceId,
        arg: &SyntaxCallArgument,
        param: &ParamHint,
    ) -> CompileResult<(Register, bool)> {
        let Some(expected) = param.type_hint.as_ref().and_then(type_hint_value_type) else {
            return self
                .compile_param_default_expression(source, &arg.value)
                .map(|register| (register, false));
        };
        let context = TypeContractContext::FunctionParameter {
            name: param.name.clone(),
        };
        let outcome = check_expected_type(
            self.param_default_static_type(source, &arg.value),
            expected,
            arg.span,
            context,
        )?;
        let requires_guard = matches!(outcome, ExpectedTypeOutcome::RequiresRuntimeGuard(_));
        self.compile_param_default_initializer(source, &arg.value, Some(&outcome))
            .map(|register| (register, requires_guard))
    }

    fn compile_param_default_native_argument_for_param(
        &mut self,
        function: &str,
        index: u16,
        source: SourceId,
        arg: &SyntaxCallArgument,
        param: &ParamHint,
    ) -> CompileResult<Register> {
        let Some(expected) = param.type_hint.as_ref().and_then(type_hint_value_type) else {
            return self.compile_param_default_expression(source, &arg.value);
        };
        let context = TypeContractContext::NativeParameter {
            function: function.to_owned(),
            name: param.name.clone(),
            index,
        };
        let outcome = check_expected_type(
            self.param_default_static_type(source, &arg.value),
            expected,
            arg.span,
            context,
        )?;
        self.compile_param_default_initializer(source, &arg.value, Some(&outcome))
    }
}

pub(super) fn param_default_call_cst_lowering_covers(expression: &SyntaxExpression) -> bool {
    expression.as_call().is_some_and(|call| {
        call.callee()
            .and_then(|callee| callee.as_path())
            .is_some_and(|path| !path.path_segments().is_empty())
            && call.arguments().into_iter().all(|arg| {
                arg.expression()
                    .is_some_and(|value| param_default_cst_lowering_covers(&value))
            })
    })
}

fn syntax_call_arguments(
    source: SourceId,
    call: &SyntaxCallExpr,
) -> CompileResult<Vec<SyntaxCallArgument>> {
    call.arguments()
        .into_iter()
        .map(|arg| {
            let Some(value) = arg.expression() else {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "call argument",
                ))
                .with_span(span_for_range(source, arg.syntax().text_range())));
            };
            Ok(SyntaxCallArgument {
                name: arg.name_text(),
                span: span_for(source, &value),
                value,
            })
        })
        .collect()
}

fn syntax_call_args_have_names(args: &[SyntaxCallArgument]) -> bool {
    args.iter().any(|arg| arg.name.is_some())
}

fn reject_named_syntax_call_args(
    args: &[SyntaxCallArgument],
    context: &'static str,
) -> CompileResult<()> {
    if syntax_call_args_have_names(args) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            context,
        )));
    }
    Ok(())
}
