use vela_common::{Diagnostic, SourceId, Span};
use vela_host::resolved::HostMutationOp;
use vela_syntax::ast::{AssignOp, SyntaxExpression};

use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{Constant, Register};

use super::syntax_statement_values::syntax_expression_span;
use crate::compiler::host_paths::HostIndexAccessKind;

impl Compiler<'_, '_> {
    pub(in crate::compiler::control_flow) fn compile_syntax_host_index_assignment(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        target_expression: &SyntaxExpression,
        op: AssignOp,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let Some(path) = self.syntax_root_host_index_path(source, target_expression) else {
            return Ok(None);
        };
        let access = match op {
            AssignOp::Set => HostIndexAccessKind::Write,
            AssignOp::Add => HostIndexAccessKind::Mutate,
            AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => return Ok(None),
        };
        self.reject_invalid_syntax_host_index_access(
            source,
            expression,
            target_expression,
            access,
        )?;
        let root = self.compile_host_path_root(&path.root)?;
        match op {
            AssignOp::Set => {
                self.emit_host_write(
                    root,
                    path,
                    value,
                    syntax_expression_span(source, expression),
                )?;
                Ok(Some(value))
            }
            AssignOp::Add => {
                self.emit_host_mutate(
                    root,
                    path,
                    HostMutationOp::Add,
                    value,
                    syntax_expression_span(source, expression),
                )?;
                Ok(Some(value))
            }
            AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => Ok(None),
        }
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_host_field_assignment(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        target_expression: &SyntaxExpression,
        op: AssignOp,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let Some(resolved) = self.syntax_host_field_path(source, target_expression) else {
            return Ok(None);
        };
        let path = resolved.path;
        if path.segments.is_empty() {
            return Ok(None);
        }
        self.reject_read_only_syntax_host_field_assignment(source, expression, target_expression)?;
        let root = self.compile_host_path_root(&path.root)?;
        match op {
            AssignOp::Set => {
                self.emit_host_write(
                    root,
                    path,
                    value,
                    syntax_expression_span(source, expression),
                )?;
            }
            AssignOp::Add => {
                self.emit_host_mutate(
                    root,
                    path,
                    HostMutationOp::Add,
                    value,
                    syntax_expression_span(source, expression),
                )?;
            }
            AssignOp::Sub => {
                self.emit_host_mutate(
                    root,
                    path,
                    HostMutationOp::Sub,
                    value,
                    syntax_expression_span(source, expression),
                )?;
            }
            AssignOp::Mul => {
                self.emit_host_mutate(
                    root,
                    path,
                    HostMutationOp::Mul,
                    value,
                    syntax_expression_span(source, expression),
                )?;
            }
            AssignOp::Div => {
                self.emit_host_mutate(
                    root,
                    path,
                    HostMutationOp::Div,
                    value,
                    syntax_expression_span(source, expression),
                )?;
            }
            AssignOp::Rem => {
                self.emit_host_mutate(
                    root,
                    path,
                    HostMutationOp::Rem,
                    value,
                    syntax_expression_span(source, expression),
                )?;
            }
        }
        Ok(Some(value))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_host_index(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(index) = expression.as_index() else {
            return Ok(None);
        };
        let Some(receiver_expression) = index.receiver() else {
            return Ok(None);
        };
        let Some(index_expression) = index.index() else {
            return Ok(None);
        };
        self.reject_invalid_syntax_host_index_read(
            source,
            expression,
            &receiver_expression,
            &index_expression,
        )?;
        let Some(path) = self.syntax_root_host_index_path(source, expression) else {
            return Ok(None);
        };
        let root = self.compile_host_path_root(&path.root)?;
        let dst = self.alloc_register()?;
        self.emit_host_read(dst, root, path, syntax_expression_span(source, expression))?;
        Ok(Some(dst))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_host_field_read(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(resolved) = self.syntax_host_field_path(source, expression) else {
            return Ok(None);
        };
        let path = resolved.path;
        if path.segments.is_empty() {
            return Ok(None);
        }
        let root = self.compile_host_path_root(&path.root)?;
        let dst = self.alloc_register()?;
        self.emit_host_read(dst, root, path, syntax_expression_span(source, expression))?;
        Ok(Some(dst))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_host_index_remove_call(
        &mut self,
        source: SourceId,
        receiver_expression: &SyntaxExpression,
        method: &str,
        arguments_empty: bool,
        call_span: Span,
    ) -> CompileResult<Option<Register>> {
        if method != "remove" || !arguments_empty {
            return Ok(None);
        }
        let Some(path) = self.syntax_root_host_index_path(source, receiver_expression) else {
            return Ok(None);
        };
        self.reject_invalid_syntax_host_index_access(
            source,
            receiver_expression,
            receiver_expression,
            HostIndexAccessKind::Remove,
        )?;
        let root = self.compile_host_path_root(&path.root)?;
        self.emit_host_remove(root, path, call_span)?;
        let dst = self.alloc_register()?;
        self.emit_constant_to(dst, Constant::Null);
        Ok(Some(dst))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_host_path_push_call(
        &mut self,
        source: SourceId,
        receiver_expression: &SyntaxExpression,
        method: &str,
        arguments: &[vela_syntax::ast::SyntaxArgument],
        call_span: Span,
    ) -> CompileResult<Option<Register>> {
        if method != "push" {
            return Ok(None);
        }
        let Some(resolved) = self.syntax_host_field_path(source, receiver_expression) else {
            return Ok(None);
        };
        let path = resolved.path;
        if path.segments.is_empty() {
            return Ok(None);
        }
        self.reject_read_only_syntax_host_field_assignment(
            source,
            receiver_expression,
            receiver_expression,
        )?;
        if arguments
            .iter()
            .any(|argument| argument.name_text().is_some())
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path push",
            )));
        }
        let [argument] = arguments else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path push arity",
            )));
        };
        let Some(expression) = argument.expression() else {
            return Ok(None);
        };
        let Some(value) = self.compile_syntax_expression(source, &expression)? else {
            return Ok(None);
        };
        let root = self.compile_host_path_root(&path.root)?;
        self.emit_host_mutate(root, path, HostMutationOp::Push, value, call_span)?;
        let dst = self.alloc_register()?;
        self.emit_constant_to(dst, Constant::Null);
        Ok(Some(dst))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_host_method_call(
        &mut self,
        source: SourceId,
        receiver_expression: &SyntaxExpression,
        method: &str,
        arguments: &[vela_syntax::ast::SyntaxArgument],
        call_span: Span,
    ) -> CompileResult<Option<Register>> {
        let Some(resolved) = self.syntax_host_field_path(source, receiver_expression) else {
            return Ok(None);
        };
        let Some(method_id) = self.host_method_id(resolved.type_name.as_deref(), method) else {
            return Ok(None);
        };
        let path = resolved.path;
        let root = self.compile_host_path_root(&path.root)?;
        let Some(args) = self
            .compile_syntax_host_method_call_arguments(source, method_id, arguments, call_span)?
        else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        self.emit_host_call(Some(dst), root, path, method_id, args, call_span)?;
        Ok(Some(dst))
    }

    fn reject_read_only_syntax_host_field_assignment(
        &self,
        source: SourceId,
        error_expression: &SyntaxExpression,
        target_expression: &SyntaxExpression,
    ) -> CompileResult<()> {
        let Some((receiver_type, field)) =
            self.syntax_host_assignment_receiver_and_field(source, target_expression)
        else {
            return Ok(());
        };
        let Some(access) = self.host_field_info(Some(receiver_type.as_str()), field.as_str())
        else {
            return Ok(());
        };
        if access.writable {
            return Ok(());
        }
        let span = syntax_expression_span(source, error_expression);
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            vec![
                Diagnostic::error("field is read-only for script writes")
                    .with_code("analysis::field_not_writable")
                    .with_span(span)
                    .with_label(span, "assignment targets a read-only field")
                    .with_label(
                        span,
                        "write through an exposed method or a writable field instead",
                    ),
            ],
        )))
    }

    fn syntax_host_assignment_receiver_and_field(
        &self,
        source: SourceId,
        target_expression: &SyntaxExpression,
    ) -> Option<(String, String)> {
        let field = target_expression.as_field()?;
        let receiver = field.receiver()?;
        let field = field.name_text()?;
        let receiver_type = self
            .script_fact_for_syntax_expression(source, &receiver)
            .map(|fact| fact.type_name)
            .or_else(|| {
                self.syntax_host_field_path(source, &receiver)
                    .and_then(|resolved| resolved.type_name)
            })?;
        Some((receiver_type, field))
    }

    fn reject_invalid_syntax_host_index_access(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
        target_expression: &SyntaxExpression,
        kind: HostIndexAccessKind,
    ) -> CompileResult<()> {
        let Some(index) = target_expression.as_index() else {
            return Ok(());
        };
        let Some(receiver_expression) = index.receiver() else {
            return Ok(());
        };
        let Some(index_expression) = index.index() else {
            return Ok(());
        };
        let Some(receiver_type) = self
            .script_fact_for_syntax_expression(source, &receiver_expression)
            .map(|fact| fact.type_name)
            .filter(|type_name| self.host_runtime_type_id(type_name).is_some())
        else {
            return Ok(());
        };
        let expression_span = syntax_expression_span(source, expression);
        let receiver_span = syntax_expression_span(source, &receiver_expression);
        let Some(capability) = self.facts.options.host_index_capability(&receiver_type) else {
            return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                vec![
                    Diagnostic::error(format!(
                        "type `{receiver_type}` does not support host index access"
                    ))
                    .with_code("analysis::host_index_not_supported")
                    .with_span(expression_span)
                    .with_label(
                        expression_span,
                        "host index access is not registered for this type",
                    )
                    .with_label(
                        receiver_span,
                        "register a host index capability or expose a field/method instead",
                    ),
                ],
            )));
        };
        if !kind.allowed_by(capability) {
            return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                vec![
                    Diagnostic::error(format!(
                        "type `{receiver_type}` does not allow host index {}",
                        kind.access_name()
                    ))
                    .with_code(kind.denied_code())
                    .with_span(expression_span)
                    .with_label(expression_span, kind.capability_label())
                    .with_label(receiver_span, kind.enable_label()),
                ],
            )));
        }
        if let Some(expected) = capability.key_type.as_deref()
            && let Some(actual) =
                self.syntax_value_type_for_expression(Some(source), &index_expression)
            && actual.source_type_name() != expected
            && actual.std_type_name() != expected
        {
            return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                vec![
                    Diagnostic::error(format!(
                        "host index key for `{receiver_type}` must be `{expected}`"
                    ))
                    .with_code("analysis::host_index_key_mismatch")
                    .with_span(expression_span)
                    .with_label(
                        syntax_expression_span(source, &index_expression),
                        format!("index expression has type `{}`", actual.source_type_name()),
                    ),
                ],
            )));
        }
        Ok(())
    }
}
