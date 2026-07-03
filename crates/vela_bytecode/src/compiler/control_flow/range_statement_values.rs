use std::collections::BTreeMap;

use vela_common::Span;
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{AstNode, Literal, SyntaxExpression};

use crate::compiler::body_payloads::{
    expression_syntax_negated_number_literal, expression_syntax_path_or_self,
    expression_syntax_range_operands,
};
use crate::compiler::const_eval::{compile_negated_literal_constant, evaluate_syntax_const_expr};
use crate::compiler::record_shapes::ValueShape;
use crate::compiler::script_types::{ScriptTypeFact, type_hint_script_type};
use crate::compiler::value_types::{
    RuntimeTypeFact, StandardRuntimeType, StaticExprType, TypeContractContext, check_expected_type,
    type_hint_value_type,
};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};
use crate::{Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(in crate::compiler::control_flow) fn compile_let_syntax_range(
        &mut self,
        name: String,
        span: Span,
        source: vela_common::SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        let Some((lhs, rhs, inclusive)) = expression_syntax_range_operands(expression) else {
            return Ok(None);
        };
        let local_binding = self
            .bindings
            .local_named_at(&name, LocalBindingKind::Let, span)
            .and_then(|local| {
                self.bindings
                    .local(local)
                    .map(|binding| (local, binding.type_hint.clone()))
            });
        let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
        let hinted_script_fact = hir_type_hint.and_then(|hint| {
            let known_type_names = self.facts.known_type_names();
            type_hint_script_type(hint, known_type_names.iter()).map(ScriptTypeFact::new)
        });
        let hinted_value_type = hir_type_hint.and_then(type_hint_value_type);
        if let Some(expected) = hinted_value_type.clone() {
            check_expected_type(
                StaticExprType::Exact(range_type_fact()),
                expected.clone(),
                span,
                TypeContractContext::TypedLet { name: name.clone() },
            )?;
        }
        let register = self.compile_syntax_range_value(source, &lhs, &rhs, inclusive)?;
        self.locals.insert(name.clone(), register);
        let value_type = hinted_value_type.or_else(|| Some(range_type_fact()));
        let value_shape = Some(ValueShape::Scalar("Range".to_owned()));
        if let Some((local, _)) = local_binding {
            self.hir_locals.insert(local, register);
            self.record_frame_slot(
                name.clone(),
                register,
                frame_slot_kind(LocalBindingKind::Let),
                Some(local),
                Some(span),
            );
            self.script_types
                .set_local_fact(local, name.clone(), hinted_script_fact);
            self.value_types.set_local(local, name.clone(), value_type);
            self.value_shapes.set_local(local, name, value_shape);
        } else {
            self.record_frame_slot(
                name.clone(),
                register,
                frame_slot_kind(LocalBindingKind::Let),
                None,
                Some(span),
            );
            self.script_types
                .set_name_fact(name.clone(), hinted_script_fact);
            self.value_types.set_name(name.clone(), value_type);
            self.value_shapes.set_name(name, value_shape);
        }
        Ok(Some(false))
    }

    pub(in crate::compiler::control_flow) fn compile_return_syntax_range(
        &mut self,
        source: vela_common::SourceId,
        expression: &SyntaxExpression,
        span: Span,
    ) -> CompileResult<Option<bool>> {
        let Some((lhs, rhs, inclusive)) = expression_syntax_range_operands(expression) else {
            return Ok(None);
        };
        if let Some(expected) = self.return_type.clone() {
            check_expected_type(
                StaticExprType::Exact(range_type_fact()),
                expected,
                span,
                TypeContractContext::Return,
            )?;
        }
        let register = self.compile_syntax_range_value(source, &lhs, &rhs, inclusive)?;
        self.emit(UnlinkedInstructionKind::Return { src: register });
        Ok(Some(true))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_range_expr_statement(
        &mut self,
        source: vela_common::SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        let Some((lhs, rhs, inclusive)) = expression_syntax_range_operands(expression) else {
            return Ok(None);
        };
        self.compile_syntax_range_value(source, &lhs, &rhs, inclusive)?;
        Ok(Some(false))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_range_expr_to(
        &mut self,
        source: vela_common::SourceId,
        expression: &SyntaxExpression,
        dst: Register,
    ) -> CompileResult<Option<bool>> {
        let Some((lhs, rhs, inclusive)) = expression_syntax_range_operands(expression) else {
            return Ok(None);
        };
        let value = self.compile_syntax_range_value(source, &lhs, &rhs, inclusive)?;
        if value != dst {
            self.emit(UnlinkedInstructionKind::Move { dst, src: value });
        }
        Ok(Some(false))
    }

    fn compile_syntax_range_value(
        &mut self,
        source: vela_common::SourceId,
        lhs: &SyntaxExpression,
        rhs: &SyntaxExpression,
        inclusive: bool,
    ) -> CompileResult<Register> {
        let start = self.compile_syntax_range_operand(source, lhs)?;
        let end = self.compile_syntax_range_operand(source, rhs)?;
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeRange {
            dst,
            start,
            end,
            inclusive,
        });
        Ok(dst)
    }

    fn compile_syntax_range_operand(
        &mut self,
        source: vela_common::SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Register> {
        if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
            return self.compile_syntax_range_operand(source, &inner);
        }
        let span = syntax_expression_span(source, expression);
        if let Some(literal) = expression
            .as_literal()
            .and_then(|literal| literal.literal())
        {
            return self.compile_literal(Some(span), &literal);
        }
        if let Some(literal) = expression_syntax_negated_number_literal(expression) {
            return self.compile_syntax_negated_range_operand(literal, span);
        }
        if let Some(path) = expression_syntax_path_or_self(expression) {
            return self.compile_path_expr(span, &path);
        }
        if let Some(constant) = evaluate_syntax_const_expr(source, expression, &BTreeMap::new())? {
            return self.emit_constant(constant);
        }
        Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "range operand expression",
        )))
    }

    fn compile_syntax_negated_range_operand(
        &mut self,
        literal: Literal,
        span: Span,
    ) -> CompileResult<Register> {
        let Some(constant) =
            compile_negated_literal_constant(&literal).map_err(|error| error.with_span(span))?
        else {
            return self.compile_literal(Some(span), &literal);
        };
        self.emit_constant(constant)
    }
}

fn range_type_fact() -> RuntimeTypeFact {
    RuntimeTypeFact::standard(StandardRuntimeType::Range)
}

fn syntax_expression_span(source: vela_common::SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}
