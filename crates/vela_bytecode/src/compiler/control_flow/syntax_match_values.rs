use vela_common::{SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{
    AstNode, SyntaxExpression, SyntaxMatchArm, SyntaxMatchArmBody, SyntaxMatchExpr, SyntaxPattern,
    SyntaxPatternKind,
};

use crate::compiler::body_payloads::CompilerBodyPayload;
use crate::compiler::patterns::PatternBindingFacts;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{Constant, Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(in crate::compiler::control_flow) fn compile_syntax_match_value(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(match_expr) = expression.as_match() else {
            return Ok(None);
        };
        if !syntax_match_value_lowering_covers(&match_expr) {
            return Ok(None);
        }
        let dst = self.alloc_register()?;
        self.compile_syntax_match_value_to(source, &match_expr, dst)?;
        Ok(Some(dst))
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_match_statement(
        &mut self,
        source: SourceId,
        match_expr: &SyntaxMatchExpr,
    ) -> CompileResult<Option<bool>> {
        let Some(scrutinee_expression) = match_expr.scrutinee() else {
            return Ok(None);
        };
        let Some(scrutinee) = self.compile_syntax_expression(source, &scrutinee_expression)? else {
            return Err(syntax_match_error(
                source,
                scrutinee_expression.syntax().text_range(),
            ));
        };
        let scrutinee_facts = PatternBindingFacts::value_shape(
            self.value_shape_for_syntax_expression(Some(source), &scrutinee_expression),
        )
        .with_script(self.script_fact_for_syntax_expression(source, &scrutinee_expression));
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms().is_empty();

        for arm in match_expr.arms() {
            let Some(pattern) = arm.pattern() else {
                return Err(syntax_match_error(source, arm.syntax().text_range()));
            };
            let mut next_arm_jumps =
                self.compile_syntax_match_pattern(source, scrutinee, &pattern)?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            let previous_value_shapes = self.value_shapes.clone();
            self.bind_syntax_pattern_locals(
                scrutinee,
                &pattern,
                syntax_match_arm_body_span(source, &arm),
                scrutinee_facts.clone(),
                LocalBindingKind::Pattern,
            )?;
            if let Some(guard) = arm.guard() {
                let Some(condition) = self.compile_syntax_expression(source, &guard)? else {
                    return Err(syntax_match_error(source, guard.syntax().text_range()));
                };
                next_arm_jumps.push(self.emit_jump_if_false(condition));
            }
            let arm_returned = self.compile_syntax_match_arm_statement(source, &arm)?;
            self.locals = previous_locals;
            self.hir_locals = previous_hir_locals;
            self.script_types = previous_script_types;
            self.value_types = previous_value_types;
            self.value_shapes = previous_value_shapes;
            all_arms_return &= arm_returned;
            if !arm_returned {
                end_jumps.push(self.emit_jump());
            }
            if next_arm_jumps.is_empty() {
                break;
            }
            for jump in next_arm_jumps {
                self.patch_jump(jump, self.current_offset())?;
            }
        }

        for jump in end_jumps {
            self.patch_jump(jump, self.current_offset())?;
        }
        Ok(Some(all_arms_return))
    }

    fn compile_syntax_match_value_to(
        &mut self,
        source: SourceId,
        match_expr: &SyntaxMatchExpr,
        dst: Register,
    ) -> CompileResult<bool> {
        let Some(scrutinee_expression) = match_expr.scrutinee() else {
            return Err(syntax_match_error(source, match_expr.syntax().text_range()));
        };
        let Some(scrutinee) = self.compile_syntax_expression(source, &scrutinee_expression)? else {
            return Err(syntax_match_error(
                source,
                scrutinee_expression.syntax().text_range(),
            ));
        };
        let scrutinee_facts = PatternBindingFacts::value_shape(
            self.value_shape_for_syntax_expression(Some(source), &scrutinee_expression),
        )
        .with_script(self.script_fact_for_syntax_expression(source, &scrutinee_expression));
        let mut end_jumps = Vec::new();
        let mut has_catch_all = false;

        for arm in match_expr.arms() {
            let Some(pattern) = arm.pattern() else {
                return Err(syntax_match_error(source, arm.syntax().text_range()));
            };
            let mut next_arm_jumps =
                self.compile_syntax_match_pattern(source, scrutinee, &pattern)?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            let previous_value_shapes = self.value_shapes.clone();
            self.bind_syntax_pattern_locals(
                scrutinee,
                &pattern,
                syntax_match_arm_body_span(source, &arm),
                scrutinee_facts.clone(),
                LocalBindingKind::Pattern,
            )?;
            if let Some(guard) = arm.guard() {
                let Some(condition) = self.compile_syntax_expression(source, &guard)? else {
                    return Err(syntax_match_error(source, guard.syntax().text_range()));
                };
                next_arm_jumps.push(self.emit_jump_if_false(condition));
            }
            self.compile_syntax_match_arm(source, &arm, dst)?;
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
            self.emit_constant_to(dst, Constant::Null);
        }
        for jump in end_jumps {
            self.patch_jump(jump, self.current_offset())?;
        }
        Ok(false)
    }

    pub(in crate::compiler::control_flow) fn compile_syntax_match_pattern(
        &mut self,
        source: SourceId,
        scrutinee: Register,
        pattern: &SyntaxPattern,
    ) -> CompileResult<Vec<usize>> {
        let Some(kind) = pattern.pattern_kind() else {
            return Err(syntax_match_error(source, pattern.syntax().text_range()));
        };
        match kind {
            SyntaxPatternKind::Wildcard | SyntaxPatternKind::Binding => Ok(Vec::new()),
            SyntaxPatternKind::Literal | SyntaxPatternKind::Path => {
                if kind == SyntaxPatternKind::Literal {
                    let literal = pattern
                        .literal()
                        .ok_or_else(|| syntax_match_error(source, pattern.syntax().text_range()))?;
                    let pattern = self.compile_literal(None, &literal)?;
                    let condition = self.alloc_register()?;
                    self.emit(UnlinkedInstructionKind::Equal {
                        dst: condition,
                        lhs: scrutinee,
                        rhs: pattern,
                    });
                    Ok(vec![self.emit_jump_if_false(condition)])
                } else {
                    let path = syntax_pattern_path_segments(source, pattern)?;
                    self.compile_variant_tag_pattern(scrutinee, &path)
                }
            }
            SyntaxPatternKind::TupleVariant => {
                let path = syntax_pattern_path_segments(source, pattern)?;
                let mut jumps = self.compile_variant_tag_pattern(scrutinee, &path)?;
                let tuple = pattern
                    .tuple_pattern()
                    .ok_or_else(|| syntax_match_error(source, pattern.syntax().text_range()))?;
                for (index, field) in tuple.patterns().enumerate() {
                    let Some(kind) = field.pattern_kind() else {
                        return Err(syntax_match_error(source, field.syntax().text_range()));
                    };
                    if !syntax_pattern_kind_needs_match_check(kind) {
                        continue;
                    }
                    let field_value = self.emit_enum_pattern_field_read(
                        scrutinee,
                        &path,
                        crate::compiler::patterns::tuple_variant_field_name(index),
                    )?;
                    jumps.extend(self.compile_syntax_match_pattern(source, field_value, &field)?);
                }
                Ok(jumps)
            }
            SyntaxPatternKind::RecordVariant => {
                let path = syntax_pattern_path_segments(source, pattern)?;
                let mut jumps = self.compile_variant_tag_pattern(scrutinee, &path)?;
                let record = pattern
                    .record_pattern()
                    .ok_or_else(|| syntax_match_error(source, pattern.syntax().text_range()))?;
                for field in record.fields() {
                    let Some(field_pattern) = field.pattern() else {
                        if field.is_shorthand() {
                            continue;
                        }
                        return Err(syntax_match_error(source, field.syntax().text_range()));
                    };
                    let Some(kind) = field_pattern.pattern_kind() else {
                        return Err(syntax_match_error(
                            source,
                            field_pattern.syntax().text_range(),
                        ));
                    };
                    if !syntax_pattern_kind_needs_match_check(kind) {
                        continue;
                    }
                    let field_name = field
                        .label_text()
                        .ok_or_else(|| syntax_match_error(source, field.syntax().text_range()))?;
                    let field_value =
                        self.emit_enum_pattern_field_read(scrutinee, &path, field_name)?;
                    jumps.extend(self.compile_syntax_match_pattern(
                        source,
                        field_value,
                        &field_pattern,
                    )?);
                }
                Ok(jumps)
            }
        }
    }

    fn compile_syntax_match_arm(
        &mut self,
        source: SourceId,
        arm: &SyntaxMatchArm,
        dst: Register,
    ) -> CompileResult<()> {
        match arm.body() {
            Some(SyntaxMatchArmBody::Expression(body)) => {
                let Some(value) = self.compile_syntax_expression(source, &body)? else {
                    return Err(syntax_match_error(source, body.syntax().text_range()));
                };
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(())
            }
            Some(SyntaxMatchArmBody::Block(block)) => {
                let body = CompilerBodyPayload::nested_syntax(source, block);
                self.compile_block_payload_value_to(&body, dst)?;
                Ok(())
            }
            None => Err(syntax_match_error(source, arm.syntax().text_range())),
        }
    }

    fn compile_syntax_match_arm_statement(
        &mut self,
        source: SourceId,
        arm: &SyntaxMatchArm,
    ) -> CompileResult<bool> {
        match arm.body() {
            Some(SyntaxMatchArmBody::Expression(body)) => {
                let Some(_value) = self.compile_syntax_expression(source, &body)? else {
                    return Err(syntax_match_error(source, body.syntax().text_range()));
                };
                Ok(false)
            }
            Some(SyntaxMatchArmBody::Block(block)) => {
                let body = CompilerBodyPayload::nested_syntax(source, block);
                self.compile_body_payload_statements(&body)
            }
            None => Err(syntax_match_error(source, arm.syntax().text_range())),
        }
    }
}

fn syntax_match_value_lowering_covers(match_expr: &SyntaxMatchExpr) -> bool {
    match_expr.attributes().next().is_none()
        && match_expr.arms_have_covered_patterns()
        && match_expr.arms().into_iter().all(|arm| {
            arm.body().is_some_and(|body| match body {
                SyntaxMatchArmBody::Expression(expression) => expression
                    .as_match()
                    .is_none_or(|match_expr| syntax_match_value_lowering_covers(&match_expr)),
                SyntaxMatchArmBody::Block(_) => true,
            })
        })
}

trait SyntaxMatchCoverage {
    fn arms_have_covered_patterns(&self) -> bool;
}

impl SyntaxMatchCoverage for SyntaxMatchExpr {
    fn arms_have_covered_patterns(&self) -> bool {
        self.arms().into_iter().all(|arm| {
            arm.pattern().is_some_and(|pattern| {
                matches!(
                    pattern.pattern_kind(),
                    Some(
                        SyntaxPatternKind::Wildcard
                            | SyntaxPatternKind::Binding
                            | SyntaxPatternKind::Literal
                            | SyntaxPatternKind::Path
                            | SyntaxPatternKind::TupleVariant
                            | SyntaxPatternKind::RecordVariant
                    )
                )
            })
        })
    }
}

fn syntax_match_error(source: SourceId, range: vela_syntax::TextRange) -> CompileError {
    CompileError::new(CompileErrorKind::UnsupportedSyntax("match expression")).with_span(Span::new(
        source,
        range.start().into(),
        range.end().into(),
    ))
}

fn syntax_match_arm_body_span(source: SourceId, arm: &SyntaxMatchArm) -> Span {
    let range = arm
        .body()
        .map(|body| match body {
            SyntaxMatchArmBody::Expression(expression) => expression.syntax().text_range(),
            SyntaxMatchArmBody::Block(block) => block.syntax().text_range(),
        })
        .unwrap_or_else(|| arm.syntax().text_range());
    Span::new(source, range.start().into(), range.end().into())
}

fn syntax_pattern_path_segments(
    source: SourceId,
    pattern: &SyntaxPattern,
) -> CompileResult<Vec<String>> {
    let path = pattern.path_segments();
    if path.is_empty() {
        return Err(syntax_match_error(source, pattern.syntax().text_range()));
    }
    Ok(path)
}

const fn syntax_pattern_kind_needs_match_check(kind: SyntaxPatternKind) -> bool {
    matches!(
        kind,
        SyntaxPatternKind::Literal
            | SyntaxPatternKind::Path
            | SyntaxPatternKind::TupleVariant
            | SyntaxPatternKind::RecordVariant
    )
}
