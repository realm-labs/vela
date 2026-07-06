use vela_common::{SourceId, Span};
use vela_syntax::ast::{
    AstNode, Pattern, SyntaxExpression, SyntaxMatchArm, SyntaxMatchArmBody, SyntaxMatchExpr,
    SyntaxPattern, SyntaxPatternKind,
};

use crate::compiler::body_payloads::{CompilerBodyPayload, CompilerPatternPayload};
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
        let mut end_jumps = Vec::new();
        let mut has_catch_all = false;

        for arm in match_expr.arms() {
            let Some(pattern) = arm.pattern() else {
                return Err(syntax_match_error(source, arm.syntax().text_range()));
            };
            let next_arm_jumps = self.compile_syntax_match_pattern(source, scrutinee, &pattern)?;
            self.compile_syntax_match_arm(source, &arm, dst)?;
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
                let payload =
                    CompilerPatternPayload::from_syntax(Some(source), Some(pattern.clone()));
                self.compile_match_pattern(scrutinee, &Pattern::Wildcard, Some(&payload))
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
}

fn syntax_match_value_lowering_covers(match_expr: &SyntaxMatchExpr) -> bool {
    match_expr.attributes().next().is_none()
        && match_expr.guardless_arms_have_covered_patterns()
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
    fn guardless_arms_have_covered_patterns(&self) -> bool;
}

impl SyntaxMatchCoverage for SyntaxMatchExpr {
    fn guardless_arms_have_covered_patterns(&self) -> bool {
        self.arms().into_iter().all(|arm| {
            arm.guard().is_none()
                && arm.pattern().is_some_and(|pattern| {
                    matches!(
                        pattern.pattern_kind(),
                        Some(
                            SyntaxPatternKind::Wildcard
                                | SyntaxPatternKind::Literal
                                | SyntaxPatternKind::Path
                        )
                    )
                })
        })
    }
}

fn syntax_match_error(source: SourceId, range: vela_syntax::TextRange) -> CompileError {
    CompileError::new(CompileErrorKind::UnsupportedSyntax("CST match value")).with_span(Span::new(
        source,
        range.start().into(),
        range.end().into(),
    ))
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
