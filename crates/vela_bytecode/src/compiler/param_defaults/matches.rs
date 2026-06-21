use vela_common::{SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{
    AstNode, SyntaxExpression, SyntaxMatchArm, SyntaxMatchArmBody, SyntaxMatchExpr, SyntaxPattern,
    SyntaxPatternKind,
};

use crate::{Constant, Register, UnlinkedInstructionKind};

use crate::compiler::const_eval::compile_literal_constant;
use crate::compiler::patterns::{PatternBindingFacts, enum_variant_path, tuple_variant_field_name};
use crate::compiler::value_types::StaticExprType;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};

use super::{
    param_default_cst_lowering_covers, param_default_unsupported, span_for, span_for_range,
};

impl Compiler<'_, '_> {
    pub(super) fn compile_param_default_match(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        match_expr: &SyntaxMatchExpr,
    ) -> CompileResult<Register> {
        if !param_default_match_cst_lowering_covers(expression) {
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

            self.bind_param_default_match_pattern_locals(
                source,
                scrutinee,
                &pattern,
                &arm,
                scrutinee_facts.clone(),
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
            self.emit_constant_to(dst, Constant::Null);
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
                let path = pattern.path_segments();
                if enum_variant_path(&path).is_none() {
                    return Err(param_default_pattern_unsupported(source, pattern));
                }
                self.compile_variant_tag_pattern(scrutinee, &path)
            }
            Some(SyntaxPatternKind::TupleVariant) => {
                let Some(tuple) = pattern.as_tuple_variant() else {
                    return Err(param_default_pattern_unsupported(source, pattern));
                };
                let path = tuple.path_segments();
                if enum_variant_path(&path).is_none() {
                    return Err(param_default_pattern_unsupported(source, pattern));
                }
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
                let path = record.path_segments();
                if enum_variant_path(&path).is_none() {
                    return Err(param_default_pattern_unsupported(source, pattern));
                }
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

    fn bind_param_default_match_pattern_locals(
        &mut self,
        source: SourceId,
        scrutinee: Register,
        pattern: &SyntaxPattern,
        arm: &SyntaxMatchArm,
        facts: PatternBindingFacts,
    ) -> CompileResult<()> {
        let span = arm
            .body_as_expression()
            .map(|body| span_for(source, &body))
            .or_else(|| {
                arm.body_block()
                    .map(|block| span_for_range(source, block.syntax().text_range()))
            })
            .unwrap_or_else(|| span_for_range(source, arm.syntax().text_range()));
        self.bind_param_default_match_pattern_locals_at_span(
            source, scrutinee, pattern, span, facts,
        )
    }

    fn bind_param_default_match_pattern_locals_at_span(
        &mut self,
        source: SourceId,
        scrutinee: Register,
        pattern: &SyntaxPattern,
        span: Span,
        facts: PatternBindingFacts,
    ) -> CompileResult<()> {
        match pattern.pattern_kind() {
            Some(SyntaxPatternKind::Binding) => {
                let Some(name) = pattern.binding_name() else {
                    return Ok(());
                };
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::Move {
                    dst,
                    src: scrutinee,
                });
                self.bind_param_default_pattern_local(&name, dst, span, facts);
            }
            Some(SyntaxPatternKind::TupleVariant) => {
                let Some(tuple) = pattern.as_tuple_variant() else {
                    return Err(param_default_pattern_unsupported(source, pattern));
                };
                let path = tuple.path_segments();
                for (index, field_pattern) in tuple.patterns().enumerate() {
                    if !param_default_pattern_declares_locals(&field_pattern) {
                        continue;
                    }
                    let field_name = tuple_variant_field_name(index);
                    let field_value =
                        self.emit_enum_pattern_field_read(scrutinee, &path, field_name.clone())?;
                    let field_facts = PatternBindingFacts::value(
                        self.enum_variant_field_value_type(&path, &field_name),
                    )
                    .with_script(self.enum_variant_field_fact(&path, &field_name));
                    self.bind_param_default_match_pattern_locals_at_span(
                        source,
                        field_value,
                        &field_pattern,
                        span,
                        field_facts,
                    )?;
                }
            }
            Some(SyntaxPatternKind::RecordVariant) => {
                let Some(record) = pattern.as_record_variant() else {
                    return Err(param_default_pattern_unsupported(source, pattern));
                };
                let path = record.path_segments();
                for field in record.fields() {
                    let Some(field_name) = field.label_text() else {
                        return Err(param_default_pattern_unsupported(source, pattern));
                    };
                    let nested_pattern = field.pattern();
                    if nested_pattern
                        .as_ref()
                        .is_some_and(|pattern| !param_default_pattern_declares_locals(pattern))
                    {
                        continue;
                    }
                    let field_value =
                        self.emit_enum_pattern_field_read(scrutinee, &path, field_name.clone())?;
                    let field_facts = PatternBindingFacts::value(
                        self.enum_variant_field_value_type(&path, &field_name),
                    )
                    .with_script(self.enum_variant_field_fact(&path, &field_name));
                    if let Some(nested_pattern) = nested_pattern {
                        self.bind_param_default_match_pattern_locals_at_span(
                            source,
                            field_value,
                            &nested_pattern,
                            span,
                            field_facts,
                        )?;
                    } else {
                        self.bind_param_default_pattern_local(
                            &field_name,
                            field_value,
                            span,
                            field_facts,
                        );
                    }
                }
            }
            Some(
                SyntaxPatternKind::Wildcard | SyntaxPatternKind::Literal | SyntaxPatternKind::Path,
            )
            | None => {}
        }
        Ok(())
    }

    fn bind_param_default_pattern_local(
        &mut self,
        name: &str,
        register: Register,
        body_span: Span,
        facts: PatternBindingFacts,
    ) {
        self.locals.insert(name.to_owned(), register);
        if let Some(local) =
            self.bindings
                .local_named_at(name, LocalBindingKind::Pattern, body_span)
        {
            self.hir_locals.insert(local, register);
            self.record_frame_slot(
                name.to_owned(),
                register,
                frame_slot_kind(LocalBindingKind::Pattern),
                Some(local),
                Some(body_span),
            );
            self.script_types.set_local_fact(local, name, None);
            self.value_types.set_local(local, name, facts.value_type());
            self.value_shapes
                .set_local(local, name, facts.value_shape_fact());
        } else {
            self.record_frame_slot(
                name.to_owned(),
                register,
                frame_slot_kind(LocalBindingKind::Pattern),
                None,
                Some(body_span),
            );
            self.value_types.set_name(name, facts.value_type());
            self.value_shapes.set_name(name, facts.value_shape_fact());
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

pub(super) fn param_default_match_cst_lowering_covers(expression: &SyntaxExpression) -> bool {
    expression.as_match().is_some_and(|match_expr| {
        match_expr.attributes().next().is_none()
            && match_expr
                .scrutinee()
                .is_some_and(|scrutinee| param_default_cst_lowering_covers(&scrutinee))
            && match_expr.arms().into_iter().all(|arm| {
                arm.pattern()
                    .is_some_and(|pattern| param_default_pattern_cst_lowering_covers(&pattern))
                    && arm
                        .guard()
                        .is_none_or(|guard| param_default_cst_lowering_covers(&guard))
                    && arm.body().is_some_and(|body| match body {
                        SyntaxMatchArmBody::Expression(expression) => {
                            param_default_cst_lowering_covers(&expression)
                        }
                        SyntaxMatchArmBody::Block(block) => {
                            super::param_default_block_cst_lowering_covers(&block)
                        }
                    })
            })
    })
}

fn param_default_pattern_cst_lowering_covers(pattern: &SyntaxPattern) -> bool {
    match pattern.pattern_kind() {
        Some(SyntaxPatternKind::Wildcard | SyntaxPatternKind::Binding) => true,
        Some(SyntaxPatternKind::Literal) => pattern.literal().is_some(),
        Some(SyntaxPatternKind::Path) => enum_variant_path(&pattern.path_segments()).is_some(),
        Some(SyntaxPatternKind::TupleVariant) => pattern.as_tuple_variant().is_some_and(|tuple| {
            enum_variant_path(&tuple.path_segments()).is_some()
                && tuple
                    .patterns()
                    .all(|pattern| param_default_pattern_cst_lowering_covers(&pattern))
        }),
        Some(SyntaxPatternKind::RecordVariant) => {
            pattern.as_record_variant().is_some_and(|record| {
                enum_variant_path(&record.path_segments()).is_some()
                    && record.fields().all(|field| {
                        field.label_text().is_some()
                            && field.pattern().is_none_or(|pattern| {
                                param_default_pattern_cst_lowering_covers(&pattern)
                            })
                    })
            })
        }
        None => false,
    }
}

fn param_default_pattern_declares_locals(pattern: &SyntaxPattern) -> bool {
    match pattern.pattern_kind() {
        Some(SyntaxPatternKind::Binding) => true,
        Some(SyntaxPatternKind::TupleVariant) => pattern.as_tuple_variant().is_some_and(|tuple| {
            tuple
                .patterns()
                .any(|pattern| param_default_pattern_declares_locals(&pattern))
        }),
        Some(SyntaxPatternKind::RecordVariant) => {
            pattern.as_record_variant().is_some_and(|record| {
                record.fields().any(|field| {
                    field
                        .pattern()
                        .is_none_or(|pattern| param_default_pattern_declares_locals(&pattern))
                })
            })
        }
        Some(
            SyntaxPatternKind::Wildcard | SyntaxPatternKind::Literal | SyntaxPatternKind::Path,
        )
        | None => false,
    }
}

fn param_default_pattern_unsupported(source: SourceId, pattern: &SyntaxPattern) -> CompileError {
    CompileError::new(CompileErrorKind::UnsupportedSyntax(
        "parameter default match pattern",
    ))
    .with_span(span_for_range(source, pattern.syntax().text_range()))
}
