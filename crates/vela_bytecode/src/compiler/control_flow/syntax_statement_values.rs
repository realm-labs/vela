use vela_common::{PrimitiveTag, SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{
    Literal, SyntaxElseBranch, SyntaxExpression, SyntaxIfExpr, SyntaxPattern, SyntaxPatternKind,
};

use crate::Constant;
use crate::compiler::body_payloads::{CompilerBodyPayload, expression_syntax_literal};
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::script_types::{ScriptTypeFact, type_hint_script_type};
use crate::compiler::value_types::{RuntimeTypeFact, type_hint_value_type};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};
use crate::{Register, UnlinkedInstructionKind};

use super::classification::merge_type_hint_and_value_fact;
use super::spans::syntax_expression_span;

use super::syntax_expression_dispatch::syntax_block_expression;

impl Compiler<'_, '_> {
    pub(super) fn compile_let_syntax_expression(
        &mut self,
        source: SourceId,
        name: String,
        span: Span,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        if let Some(block) = syntax_block_expression(expression) {
            let register = self.alloc_register()?;
            let body = CompilerBodyPayload::nested_syntax(source, block);
            let returned = self.compile_block_payload_value_to(&body, register)?;
            self.record_syntax_let_binding(name, span, register, None, None, None);
            return Ok(Some(returned));
        }
        let Some(register) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        let script_fact = self.script_fact_for_syntax_expression(source, expression);
        let value_shape = self.value_shape_for_syntax_expression(Some(source), expression);
        let value_type = self
            .syntax_value_type_for_expression(Some(source), expression)
            .or_else(|| value_shape.as_ref().and_then(|shape| shape.value_type()));
        self.record_syntax_let_binding(name, span, register, script_fact, value_type, value_shape);
        Ok(Some(false))
    }

    pub(super) fn compile_let_syntax_pattern(
        &mut self,
        source: SourceId,
        pattern: &SyntaxPattern,
        span: Span,
        expression: &SyntaxExpression,
    ) -> CompileResult<bool> {
        let Some(value) = self.compile_syntax_expression(source, expression)? else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "let pattern initializer",
            )));
        };
        if !matches!(
            pattern.pattern_kind(),
            Some(SyntaxPatternKind::TupleVariant | SyntaxPatternKind::Wildcard)
        ) {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "let pattern",
            )));
        }
        if let Some(tuple) = pattern.tuple_pattern()
            && !tuple.path_segments().is_empty()
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "let tuple variant pattern",
            )));
        }
        self.bind_syntax_pattern_locals(
            value,
            pattern,
            span,
            crate::compiler::patterns::PatternBindingFacts::default(),
            LocalBindingKind::Let,
        )?;
        Ok(false)
    }

    fn record_syntax_let_binding(
        &mut self,
        name: String,
        span: Span,
        register: Register,
        script_fact: Option<ScriptTypeFact>,
        value_type: Option<RuntimeTypeFact>,
        value_shape: Option<crate::compiler::record_shapes::ValueShape>,
    ) {
        self.locals.insert(name.clone(), register);
        let local_binding = self.let_local_binding_at_statement_span(&name, span);
        let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
        let hinted_script_fact = hir_type_hint.and_then(|hint| {
            let known_type_names = self.facts.known_type_names();
            type_hint_script_type(hint, known_type_names.iter()).map(ScriptTypeFact::new)
        });
        let script_fact = merge_type_hint_and_value_fact(hinted_script_fact, script_fact);
        let value_type = hir_type_hint.and_then(type_hint_value_type).or(value_type);
        let local = local_binding.as_ref().map(|(local, _)| *local);
        if let Some(local) = local {
            self.hir_locals.insert(local, register);
            self.script_types
                .set_local_fact(local, name.clone(), script_fact);
            self.value_types.set_local(local, name.clone(), value_type);
            self.value_shapes
                .set_local(local, name.clone(), value_shape);
        } else {
            self.script_types.set_name_fact(name.clone(), script_fact);
            self.value_types.set_name(name.clone(), value_type);
            self.value_shapes.set_name(name.clone(), value_shape);
        }
        self.record_frame_slot(
            name,
            register,
            frame_slot_kind(LocalBindingKind::Let),
            local,
            Some(span),
        );
    }

    pub(super) fn compile_return_syntax_expression(
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

    pub(super) fn compile_syntax_value_expr_statement(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        let Some(_register) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        Ok(Some(false))
    }

    pub(super) fn compile_syntax_if_statement(
        &mut self,
        source: SourceId,
        if_expr: &SyntaxIfExpr,
    ) -> CompileResult<Option<bool>> {
        let Some(condition_expression) = if_expr.condition() else {
            return Ok(None);
        };
        let Some(then_block) = if_expr.then_block() else {
            return Ok(None);
        };
        let then_body = CompilerBodyPayload::nested_syntax(source, then_block);

        let jump_to_else = if let Some(jump) =
            self.try_emit_syntax_i64_immediate_jump_if_false(source, &condition_expression)?
        {
            jump
        } else {
            let Some(condition) = self.compile_syntax_expression(source, &condition_expression)?
            else {
                return Ok(None);
            };
            self.emit_jump_if_false(condition)
        };
        let then_returned = self.compile_body_payload_statements(&then_body)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;
        let else_returned = match if_expr.else_branch() {
            Some(SyntaxElseBranch::If(else_if)) => {
                let Some(returned) = self.compile_syntax_if_statement(source, &else_if)? else {
                    return Ok(None);
                };
                returned
            }
            Some(SyntaxElseBranch::Block(block)) => {
                let else_body = CompilerBodyPayload::nested_syntax(source, block);
                self.compile_body_payload_statements(&else_body)?
            }
            None => false,
        };

        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }
        Ok(Some(then_returned && else_returned))
    }

    fn try_emit_syntax_i64_immediate_jump_if_false(
        &mut self,
        source: SourceId,
        condition: &SyntaxExpression,
    ) -> CompileResult<Option<usize>> {
        let Some(binary) = condition.as_binary() else {
            return Ok(None);
        };
        let Some(op) = binary
            .operator()
            .and_then(crate::compiler::operators::i64_compare_op)
        else {
            return Ok(None);
        };
        let Some(lhs_expression) = binary.lhs() else {
            return Ok(None);
        };
        let Some(rhs_expression) = binary.rhs() else {
            return Ok(None);
        };
        let lhs_span = syntax_expression_span(source, &lhs_expression);
        let Some(lhs_path) = self.hir_value_path_for_span(lhs_span) else {
            return Ok(None);
        };
        if self.value_type_for_path(lhs_span, &lhs_path)
            != Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
        {
            return Ok(None);
        }
        let Some(Literal::Integer(value)) = expression_syntax_literal(&rhs_expression) else {
            return Ok(None);
        };
        let Some(Constant::Scalar(vela_common::ScalarValue::I64(imm))) =
            compile_literal_constant_for_type(&Literal::Integer(value), PrimitiveTag::I64)
                .map_err(|error| {
                    error.with_span(syntax_expression_span(source, &rhs_expression))
                })?
        else {
            return Ok(None);
        };
        let lhs = self.compile_path_expr(lhs_span, &lhs_path)?;
        let offset = self.current_offset();
        self.emit(UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
            op,
            lhs,
            imm,
            target: crate::InstructionOffset(usize::MAX),
        });
        Ok(Some(offset))
    }

    pub(super) fn compile_syntax_value_expr_to(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        dst: Register,
    ) -> CompileResult<Option<bool>> {
        let Some(value) = self.compile_syntax_expression(source, expression)? else {
            return Ok(None);
        };
        if value != dst {
            self.emit(UnlinkedInstructionKind::Move { dst, src: value });
        }
        Ok(Some(false))
    }
}
