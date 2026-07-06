use vela_common::{SourceId, Span};
use vela_host::resolved::HostMutationOp;
use vela_syntax::ast::{AssignOp, SyntaxExpression};

use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, Register};

use super::syntax_statement_values::syntax_expression_span;

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
        let root = self.compile_host_path_root(&path.root)?;
        self.emit_host_remove(root, path, call_span)?;
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
}
