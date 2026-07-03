use vela_common::{SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{AstNode, BinaryOp, SyntaxExpression};

use crate::compiler::body_payloads::{expression_syntax_literal, expression_syntax_path_or_self};
use crate::compiler::operators::non_logical_binary_instruction;
use crate::compiler::{CompileResult, Compiler, frame_slot_kind};
use crate::{Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(in crate::compiler::control_flow) fn compile_let_syntax_expression(
        &mut self,
        source: SourceId,
        name: String,
        span: Span,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        let Some(register) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        self.locals.insert(name.clone(), register);
        let local_binding = self
            .bindings
            .local_named_at(&name, LocalBindingKind::Let, span);
        if let Some(local) = local_binding {
            self.hir_locals.insert(local, register);
            self.script_types.set_local_fact(local, name.clone(), None);
            self.value_types.set_local(local, name.clone(), None);
            self.value_shapes.set_local(local, name.clone(), None);
        } else {
            self.script_types.set_name_fact(name.clone(), None);
            self.value_types.set_name(name.clone(), None);
            self.value_shapes.set_name(name.clone(), None);
        }
        self.record_frame_slot(
            name,
            register,
            frame_slot_kind(LocalBindingKind::Let),
            local_binding,
            Some(span),
        );
        Ok(Some(false))
    }

    pub(in crate::compiler::control_flow) fn compile_return_syntax_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        let Some(register) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        self.emit(UnlinkedInstructionKind::Return { src: register });
        Ok(Some(true))
    }

    fn compile_syntax_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
            return self.compile_syntax_expression(source, &inner);
        }
        if let Some(literal) = expression_syntax_literal(expression) {
            return self
                .compile_literal(Some(syntax_expression_span(source, expression)), &literal)
                .map(Some);
        }
        if let Some(path) = expression_syntax_path_or_self(expression) {
            return self
                .compile_path_expr(syntax_expression_span(source, expression), &path)
                .map(Some);
        }
        let Some(binary) = expression.as_binary() else {
            return Ok(None);
        };
        let Some(op) = binary.operator() else {
            return Ok(None);
        };
        if matches!(
            op,
            BinaryOp::And | BinaryOp::Or | BinaryOp::Range | BinaryOp::RangeInclusive
        ) {
            return Ok(None);
        }
        let Some(lhs_expression) = binary.lhs() else {
            return Ok(None);
        };
        let Some(rhs_expression) = binary.rhs() else {
            return Ok(None);
        };
        let Some(lhs) = self.compile_syntax_expression(source, &lhs_expression)? else {
            return Ok(None);
        };
        let Some(rhs) = self.compile_syntax_expression(source, &rhs_expression)? else {
            return Ok(None);
        };
        let dst = self.alloc_register()?;
        let Some(instruction) = non_logical_binary_instruction(op, dst, lhs, rhs) else {
            return Ok(None);
        };
        self.emit_spanned(instruction, syntax_expression_span(source, expression));
        Ok(Some(dst))
    }
}

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}
