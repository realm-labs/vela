use vela_common::{Diagnostic, SourceId};
use vela_syntax::ast::SyntaxExpression;

use crate::compiler::body_payloads::expression_syntax_path_or_self;
use crate::compiler::calls::metadata::unresolved_static_method_error;
use crate::compiler::patterns::enum_variant_path;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{Register, UnlinkedInstructionKind};

use super::spans::syntax_expression_span;

impl Compiler<'_, '_> {
    pub(super) fn compile_syntax_call(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(call) = expression.as_call() else {
            return Ok(None);
        };
        let Some(callee) = call.callee() else {
            return Ok(None);
        };
        let call_span = syntax_expression_span(source, expression);
        let callee_span = syntax_expression_span(source, &callee);
        let arguments = call.arguments();

        if let Some(field) = callee.as_field() {
            let Some(receiver_expression) = field.receiver() else {
                return Ok(None);
            };
            let Some(method) = field.name_text() else {
                return Ok(None);
            };
            if let Some(register) = self.compile_syntax_host_index_remove_call(
                source,
                &receiver_expression,
                method.as_str(),
                arguments.is_empty(),
                call_span,
            )? {
                return Ok(Some(register));
            }
            if let Some(register) = self.compile_syntax_host_path_push_call(
                source,
                &receiver_expression,
                method.as_str(),
                &arguments,
                call_span,
            )? {
                return Ok(Some(register));
            }
            if let Some(register) = self.compile_syntax_host_method_call(
                source,
                &receiver_expression,
                method.as_str(),
                &arguments,
                call_span,
            )? {
                return Ok(Some(register));
            }
            let receiver_type = self
                .script_fact_for_syntax_expression(source, &receiver_expression)
                .map(|fact| fact.type_name);
            let receiver_shape =
                self.value_shape_for_syntax_expression(Some(source), &receiver_expression);
            let value_receiver_type = self
                .syntax_value_type_for_expression(Some(source), &receiver_expression)
                .or_else(|| receiver_shape.as_ref().and_then(|shape| shape.value_type()));
            let value_receiver_methods_known = value_receiver_type
                .as_ref()
                .is_some_and(|receiver_type| self.value_methods_known_for_type(receiver_type));
            self.reject_static_syntax_array_ordering_method_without_ord(
                source,
                &method,
                &arguments,
                value_receiver_type.as_ref(),
                receiver_shape.as_ref(),
                call_span,
            )?;
            if let Some(method_id) = receiver_type
                .as_deref()
                .and_then(|type_name| self.script_method_id_for_type(type_name, &method))
            {
                let Some(receiver) =
                    self.compile_syntax_expression(source, &receiver_expression)?
                else {
                    return Ok(None);
                };
                let Some(args) = self.compile_syntax_script_method_call_arguments(
                    source,
                    receiver_type
                        .as_deref()
                        .expect("receiver type checked above"),
                    &method,
                    &arguments,
                    call_span,
                )?
                else {
                    return Ok(None);
                };
                let dst = self.alloc_register()?;
                self.emit_spanned(
                    UnlinkedInstructionKind::CallMethodId {
                        dst,
                        receiver,
                        method,
                        method_id,
                        args,
                    },
                    call_span,
                );
                return Ok(Some(dst));
            }
            if let Some(method_id) = value_receiver_type
                .as_ref()
                .and_then(|receiver_type| self.value_method_id_for_type(receiver_type, &method))
            {
                let Some(receiver) =
                    self.compile_syntax_expression(source, &receiver_expression)?
                else {
                    return Ok(None);
                };
                let Some(args) = self.compile_syntax_value_method_call_arguments(
                    source,
                    receiver_shape.as_ref(),
                    value_receiver_type.as_ref(),
                    &method,
                    &arguments,
                    call_span,
                )?
                else {
                    return Ok(None);
                };
                let dst = self.alloc_register()?;
                self.emit_spanned(
                    UnlinkedInstructionKind::CallMethodId {
                        dst,
                        receiver,
                        method,
                        method_id,
                        args,
                    },
                    call_span,
                );
                return Ok(Some(dst));
            }
            if receiver_type.is_some() {
                return Err(unresolved_static_method_error(&method, call_span));
            }
            if value_receiver_methods_known {
                return Err(unresolved_static_method_error(&method, call_span));
            }
            let Some(receiver) = self.compile_syntax_expression(source, &receiver_expression)?
            else {
                return Ok(None);
            };
            let Some(args) = self.compile_syntax_dynamic_call_arguments(source, &arguments)? else {
                return Ok(None);
            };
            let dst = self.alloc_register()?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallDynamicMethod {
                    dst,
                    receiver,
                    method,
                    args,
                },
                call_span,
            );
            return Ok(Some(dst));
        }

        let Some(path) = expression_syntax_path_or_self(&callee) else {
            if arguments
                .iter()
                .any(|argument| argument.name_text().is_some())
            {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "closure call",
                )));
            }
            let Some(callee) = self.compile_syntax_expression(source, &callee)? else {
                return Ok(None);
            };
            let Some(args) = self.compile_syntax_call_arguments(source, &arguments)? else {
                return Ok(None);
            };
            let dst = self.alloc_register()?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallClosure { dst, callee, args },
                call_span,
            );
            return Ok(Some(dst));
        };
        if path.is_empty() {
            return Ok(None);
        }
        let dst = self.alloc_register()?;
        let call_expression = self.expression_at_span(call_span);
        if let Some((declaration, name)) =
            call_expression.and_then(|call| self.script_function_call(call))
        {
            let Some(call_args) = self.compile_syntax_script_function_call_arguments(
                source,
                declaration,
                &arguments,
                call_span,
            )?
            else {
                return Ok(None);
            };
            self.emit_spanned(
                UnlinkedInstructionKind::CallFunction {
                    dst,
                    target: crate::function_id_for_script_name(&name),
                    name,
                    mode: call_args.mode,
                    args: call_args.args,
                },
                call_span,
            );
            return Ok(Some(dst));
        }

        if call_expression
            .and_then(|call| self.local_call_callee(call))
            .is_some()
        {
            if arguments
                .iter()
                .any(|argument| argument.name_text().is_some())
            {
                return Ok(None);
            }
            let Some(callee) = self.compile_syntax_expression(source, &callee)? else {
                return Ok(None);
            };
            let Some(args) = self.compile_syntax_call_arguments(source, &arguments)? else {
                return Ok(None);
            };
            self.emit_spanned(
                UnlinkedInstructionKind::CallClosure { dst, callee, args },
                call_span,
            );
            return Ok(Some(dst));
        }

        if let Some((_enum_path, variant)) = enum_variant_path(&path)
            && let Some(enum_name) = self.type_symbol_at_span(callee_span)
        {
            let Some(fields) = self.compile_syntax_tuple_variant_fields(
                source,
                callee_span,
                &enum_name,
                &variant,
                &arguments,
            )?
            else {
                return Ok(None);
            };
            self.emit(UnlinkedInstructionKind::MakeEnum {
                dst,
                enum_name,
                variant,
                fields,
            });
            return Ok(Some(dst));
        }

        let callee_name = path.join("::");
        if callee_name == "set::from_array" {
            if arguments
                .iter()
                .any(|argument| argument.name_text().is_some())
            {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "set::from_array",
                )));
            }
            let [argument] = arguments.as_slice() else {
                return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                    vec![
                        Diagnostic::error(format!(
                            "set::from_array expects 1 argument, got {}",
                            arguments.len()
                        ))
                        .with_code("compiler::arity")
                        .with_span(callee_span),
                    ],
                )));
            };
            let Some(argument_expression) = argument.expression() else {
                return Ok(None);
            };
            let Some(src) = self.compile_syntax_expression(source, &argument_expression)? else {
                return Ok(None);
            };
            self.emit_spanned(
                UnlinkedInstructionKind::MakeSetFromArray { dst, src },
                call_span,
            );
            return Ok(Some(dst));
        }
        let native = self.resolve_native_function_id(&callee_name, callee_span)?;
        let Some(args) = self.compile_syntax_native_call_arguments(
            source,
            &callee_name,
            native,
            &arguments,
            call_span,
        )?
        else {
            return Ok(None);
        };
        self.emit_spanned(
            UnlinkedInstructionKind::CallNative {
                dst: Some(dst),
                name: callee_name,
                native,
                cache_site: None,
                args,
            },
            call_span,
        );
        Ok(Some(dst))
    }
}
