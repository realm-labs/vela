use vela_common::{SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{
    AstNode, SyntaxExpression, SyntaxMatchArm, SyntaxMatchArmBody, SyntaxMatchExpr, SyntaxPattern,
    SyntaxPatternKind,
};

use crate::{Constant, Register, UnlinkedInstructionKind};

use crate::compiler::const_eval::compile_literal_constant;
use crate::compiler::patterns::{PatternBindingFacts, tuple_variant_field_name};
use crate::compiler::value_types::StaticExprType;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};

use super::{
    param_default_expression_supported, param_default_unsupported, span_for, span_for_range,
};

impl Compiler<'_, '_> {
    pub(super) fn compile_param_default_match(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        match_expr: &SyntaxMatchExpr,
    ) -> CompileResult<Register> {
        if !param_default_match_supported(expression) {
            return Err(param_default_unsupported(source, expression));
        }
        let Some(scrutinee_expression) = match_expr.scrutinee() else {
            return Err(param_default_unsupported(source, expression));
        };
        let scrutinee = self.compile_param_default_expression(source, &scrutinee_expression)?;
        let scrutinee_facts = PatternBindingFacts::value(
            match self.param_default_static_type(source, &scrutinee_expression) {
                StaticExprType::Exact(fact) => Some(fact),
                StaticExprType::UnsuffixedIntegerLiteral
                | StaticExprType::UnsuffixedFloatLiteral
                | StaticExprType::Dynamic => None,
            },
        );
        let dst = self.alloc_register()?;
        let mut end_jumps = Vec::new();
        let mut has_catch_all = false;

        for arm in match_expr.arms() {
            let Some(pattern) = arm.pattern() else {
                return Err(param_default_unsupported(source, expression));
            };
            let mut next_arm_jumps =
                self.compile_param_default_match_pattern(source, scrutinee, &pattern)?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            let previous_value_shapes = self.value_shapes.clone();

            let hir_patterns = self.hir_pattern_ids_for_syntax_pattern(source, &pattern)?;
            self.bind_syntax_pattern_locals_from_hir_patterns(
                scrutinee,
                &pattern,
                param_default_match_arm_body_span(source, &arm),
                scrutinee_facts.clone(),
                LocalBindingKind::Pattern,
                &hir_patterns,
            )?;
            if let Some(guard) = arm.guard() {
                let condition = self.compile_param_default_expression(source, &guard)?;
                next_arm_jumps.push(self.emit_jump_if_false(condition));
            }
            self.compile_param_default_match_arm(source, expression, &arm, dst)?;

            self.locals = previous_locals;
            self.hir_locals = previous_hir_locals;
            self.script_types = previous_script_types;
            self.value_types = previous_value_types;
            self.value_shapes = previous_value_shapes;

            end_jumps.push(self.emit_jump());
            if next_arm_jumps.is_empty() {
                has_catch_all = true;
                break;
            }
            for jump in next_arm_jumps {
                self.patch_jump(jump, self.current_offset())?;
            }
        }

        if !has_catch_all {
            self.emit_constant_to(dst, Constant::Unit);
        }
        for jump in end_jumps {
            self.patch_jump(jump, self.current_offset())?;
        }
        Ok(dst)
    }

    fn compile_param_default_match_pattern(
        &mut self,
        source: SourceId,
        scrutinee: Register,
        pattern: &SyntaxPattern,
    ) -> CompileResult<Vec<usize>> {
        match pattern.pattern_kind() {
            Some(SyntaxPatternKind::Wildcard | SyntaxPatternKind::Binding) => Ok(Vec::new()),
            Some(SyntaxPatternKind::Literal) => {
                let Some(literal) = pattern.literal() else {
                    return Err(param_default_pattern_unsupported(source, pattern));
                };
                let span = span_for_range(source, pattern.syntax().text_range());
                let constant =
                    compile_literal_constant(&literal).map_err(|error| error.with_span(span))?;
                let pattern_value = self.emit_constant(constant)?;
                let condition = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::Equal {
                    dst: condition,
                    lhs: scrutinee,
                    rhs: pattern_value,
                });
                Ok(vec![self.emit_jump_if_false(condition)])
            }
            Some(SyntaxPatternKind::Path) => {
                let path = self.required_hir_pattern_path(source, pattern)?;
                self.compile_variant_tag_pattern(scrutinee, &path)
            }
            Some(SyntaxPatternKind::TupleVariant) => {
                let Some(tuple) = pattern.as_tuple_variant() else {
                    return Err(param_default_pattern_unsupported(source, pattern));
                };
                let path = self.required_hir_pattern_path(source, pattern)?;
                let mut jumps = self.compile_variant_tag_pattern(scrutinee, &path)?;
                for (index, field_pattern) in tuple.patterns().enumerate() {
                    if matches!(
                        field_pattern.pattern_kind(),
                        Some(SyntaxPatternKind::Wildcard | SyntaxPatternKind::Binding)
                    ) {
                        continue;
                    }
                    let field_value = self.emit_enum_pattern_field_read(
                        scrutinee,
                        &path,
                        tuple_variant_field_name(index),
                    )?;
                    jumps.extend(self.compile_param_default_match_pattern(
                        source,
                        field_value,
                        &field_pattern,
                    )?);
                }
                Ok(jumps)
            }
            Some(SyntaxPatternKind::RecordVariant) => {
                let Some(record) = pattern.as_record_variant() else {
                    return Err(param_default_pattern_unsupported(source, pattern));
                };
                let path = self.required_hir_pattern_path(source, pattern)?;
                let mut jumps = self.compile_variant_tag_pattern(scrutinee, &path)?;
                for field in record.fields() {
                    let Some(field_pattern) = field.pattern() else {
                        continue;
                    };
                    if matches!(
                        field_pattern.pattern_kind(),
                        Some(SyntaxPatternKind::Wildcard | SyntaxPatternKind::Binding)
                    ) {
                        continue;
                    }
                    let Some(field_name) = field.label_text() else {
                        return Err(param_default_pattern_unsupported(source, pattern));
                    };
                    let field_value =
                        self.emit_enum_pattern_field_read(scrutinee, &path, field_name)?;
                    jumps.extend(self.compile_param_default_match_pattern(
                        source,
                        field_value,
                        &field_pattern,
                    )?);
                }
                Ok(jumps)
            }
            None => Err(param_default_pattern_unsupported(source, pattern)),
        }
    }

    fn compile_param_default_match_arm(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        arm: &SyntaxMatchArm,
        dst: Register,
    ) -> CompileResult<()> {
        let value = match arm.body() {
            Some(SyntaxMatchArmBody::Expression(body)) => {
                self.compile_param_default_expression(source, &body)?
            }
            Some(SyntaxMatchArmBody::Block(block)) => {
                self.compile_param_default_block(source, &block)?
            }
            None => return Err(param_default_unsupported(source, expression)),
        };
        self.emit(UnlinkedInstructionKind::Move { dst, src: value });
        Ok(())
    }
}

pub(super) fn param_default_match_supported(expression: &SyntaxExpression) -> bool {
    expression.as_match().is_some_and(|match_expr| {
        match_expr.attributes().next().is_none()
            && match_expr
                .scrutinee()
                .is_some_and(|scrutinee| param_default_expression_supported(&scrutinee))
            && match_expr.arms().into_iter().all(|arm| {
                arm.pattern()
                    .is_some_and(|pattern| param_default_pattern_supported(&pattern))
                    && arm
                        .guard()
                        .is_none_or(|guard| param_default_expression_supported(&guard))
                    && arm.body().is_some_and(|body| match body {
                        SyntaxMatchArmBody::Expression(expression) => {
                            param_default_expression_supported(&expression)
                        }
                        SyntaxMatchArmBody::Block(block) => {
                            super::param_default_block_supported(&block)
                        }
                    })
            })
    })
}

fn param_default_pattern_supported(pattern: &SyntaxPattern) -> bool {
    match pattern.pattern_kind() {
        Some(SyntaxPatternKind::Wildcard | SyntaxPatternKind::Binding) => true,
        Some(SyntaxPatternKind::Literal) => pattern.literal().is_some(),
        Some(SyntaxPatternKind::Path) => true,
        Some(SyntaxPatternKind::TupleVariant) => pattern.as_tuple_variant().is_some_and(|tuple| {
            tuple
                .patterns()
                .all(|pattern| param_default_pattern_supported(&pattern))
        }),
        Some(SyntaxPatternKind::RecordVariant) => {
            pattern.as_record_variant().is_some_and(|record| {
                record.fields().all(|field| {
                    field.label_text().is_some()
                        && field
                            .pattern()
                            .is_none_or(|pattern| param_default_pattern_supported(&pattern))
                })
            })
        }
        None => false,
    }
}

fn param_default_match_arm_body_span(source: SourceId, arm: &SyntaxMatchArm) -> Span {
    arm.body_as_expression()
        .map(|body| span_for(source, &body))
        .or_else(|| {
            arm.body_block()
                .map(|block| span_for_range(source, block.syntax().text_range()))
        })
        .unwrap_or_else(|| span_for_range(source, arm.syntax().text_range()))
}

fn param_default_pattern_unsupported(source: SourceId, pattern: &SyntaxPattern) -> CompileError {
    CompileError::new(CompileErrorKind::UnsupportedSyntax(
        "parameter default match pattern",
    ))
    .with_span(span_for_range(source, pattern.syntax().text_range()))
}
