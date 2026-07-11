use vela_analysis::literals::ResolvedLiteralFact;
use vela_hir::body::{HirLiteral, HirMatch, HirMatchArmBody, HirPatternKind};
use vela_hir::ids::{HirExprId, HirLocalId, HirPatternId};

use crate::{
    CompilePatternConstructorTarget, MirBuildError, MirConstantProvenance, MirDynamicBinaryOp,
    MirEffect, MirEvaluatedConstant, MirFieldTarget, MirGuard, MirGuardAssumption, MirImmediate,
    MirOperand, MirPatternPredicate, MirPlace, MirRvalue, MirSafepoint, MirSourceOrigin,
    MirStatement, MirStatementKind, MirTerminator, MirTerminatorKind, MirValueType,
};

use super::core::{FunctionBuilder, value_type};

#[derive(Clone)]
enum PatternFieldLayout {
    NeverMatchesRecord {
        fields: Vec<vela_def::FieldId>,
    },
    Variant {
        type_id: vela_def::TypeId,
        variant: vela_def::VariantId,
        fields: Vec<vela_def::FieldId>,
    },
    DynamicVariant {
        owner_name: String,
        variant_name: String,
        fields: Vec<String>,
    },
}

#[derive(Clone)]
enum PatternBindingPlan {
    None,
    Bind {
        pattern: HirPatternId,
        local: HirLocalId,
    },
    Tuple {
        pattern: HirPatternId,
        arity: u32,
        fields: Vec<PatternBindingProjection>,
    },
    Constructor {
        layout: PatternFieldLayout,
        fields: Vec<PatternBindingProjection>,
    },
}

#[derive(Clone)]
struct PatternBindingProjection {
    index: usize,
    pattern: HirPatternId,
    binding: PatternBindingPlan,
}

impl PatternBindingPlan {
    fn declares_local(&self) -> bool {
        match self {
            Self::Bind { .. } => true,
            Self::Tuple { fields, .. } | Self::Constructor { fields, .. } => {
                fields.iter().any(|field| field.binding.declares_local())
            }
            Self::None => false,
        }
    }
}

impl FunctionBuilder<'_> {
    pub(super) fn lower_match_expression(
        &mut self,
        expression: HirExprId,
        value: &HirMatch,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let result_type = self
            .input
            .analysis()
            .expression(expression)
            .map(|fact| value_type(Some(fact)))
            .ok_or_else(|| self.inconsistent(origin, "match expression has no type fact"))?;
        let result = self.function.add_synthetic_local(result_type, origin);
        self.lower_match_root(value, Some(result), origin)?;
        Ok(MirOperand::Local(result))
    }

    pub(super) fn lower_match_statement(
        &mut self,
        value: &HirMatch,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.lower_match_root(value, None, origin)
    }

    /// Lower a match while retaining one ordered failure continuation between
    /// arms. Pattern bindings are written only after every structural and
    /// nested test has passed, so a failed arm cannot expose partial bindings.
    pub(super) fn lower_match_root(
        &mut self,
        value: &HirMatch,
        destination: Option<crate::MirLocalId>,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let scrutinee = value
            .scrutinee
            .ok_or_else(|| self.inconsistent(origin, "match has no scrutinee"))?;
        let scrutinee_origin = self.pattern_expression_origin(scrutinee)?;
        let scrutinee = self.lower_expression(scrutinee)?;
        if self.current_is_terminated()? {
            return Ok(());
        }
        // Match evaluates its scrutinee once. A source local must not be read
        // again after a guard or arm mutates that local.
        let scrutinee = self.capture_operand(scrutinee, scrutinee_origin)?;
        let join = self.function.add_block();
        let mut reaches_join = false;
        let mut unmatched_reachable = true;

        for arm_id in &value.arms {
            if !unmatched_reachable {
                break;
            }
            let arm = self.body.match_arms.get(arm_id).cloned().ok_or_else(|| {
                self.inconsistent(origin, format!("missing HIR match arm {arm_id:?}"))
            })?;
            let pattern = arm.pattern.ok_or_else(|| {
                self.inconsistent(
                    MirSourceOrigin::body(self.body.id, arm.origin.span),
                    "match arm has no pattern",
                )
            })?;
            let pattern_refutable = self.pattern_is_refutable(pattern)?;
            let mut next_arm = if pattern_refutable {
                Some(self.function.add_block())
            } else {
                None
            };

            if let Some(failure) = next_arm {
                self.lower_pattern_or_branch(pattern, scrutinee.clone(), failure)?;
            } else {
                self.lower_irrefutable_pattern(pattern, scrutinee.clone())?;
            }

            if let Some(guard) = arm.guard {
                let guard_origin = self.pattern_expression_origin(guard)?;
                let guard = self.lower_expression(guard)?;
                if self.current_is_terminated()? {
                    if let Some(next) = next_arm {
                        self.current_block = next;
                        continue;
                    }
                    unmatched_reachable = false;
                    break;
                }
                let failure = *next_arm.get_or_insert_with(|| self.function.add_block());
                let body = self.function.add_block();
                self.function.set_terminator(
                    self.current_block,
                    MirTerminator::new(
                        guard_origin,
                        MirTerminatorKind::Branch {
                            condition: guard,
                            then_block: body,
                            else_block: failure,
                        },
                        MirEffect::PURE,
                        None,
                    ),
                )?;
                self.current_block = body;
            }

            self.lower_match_arm_body(arm.body, destination, origin)?;
            if !self.current_is_terminated()? {
                self.jump_pattern(join, origin)?;
                reaches_join = true;
            }

            match next_arm {
                Some(next) => self.current_block = next,
                None => unmatched_reachable = false,
            }
        }

        if unmatched_reachable {
            if let Some(destination) = destination {
                self.function.widen_local_to_dynamic(destination);
                self.function.append_statement(
                    self.current_block,
                    MirStatement::assign(
                        origin,
                        MirPlace::local(destination),
                        MirRvalue::Use(MirOperand::Immediate(MirImmediate::Unit)),
                    ),
                )?;
            }
            self.jump_pattern(join, origin)?;
            reaches_join = true;
        }

        self.current_block = join;
        if !reaches_join {
            self.function.set_terminator(
                join,
                MirTerminator::new(
                    origin,
                    MirTerminatorKind::Unreachable,
                    MirEffect::PURE,
                    None,
                ),
            )?;
        }
        Ok(())
    }

    fn lower_match_arm_body(
        &mut self,
        body: Option<HirMatchArmBody>,
        destination: Option<crate::MirLocalId>,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        match body {
            Some(HirMatchArmBody::Expr(expression)) => {
                let expression_origin = self.pattern_expression_origin(expression)?;
                let value = self.lower_expression(expression)?;
                if !self.current_is_terminated()?
                    && let Some(destination) = destination
                {
                    self.function.append_statement(
                        self.current_block,
                        MirStatement::assign(
                            expression_origin,
                            MirPlace::local(destination),
                            MirRvalue::Use(value),
                        ),
                    )?;
                }
            }
            Some(HirMatchArmBody::Block(block)) => match destination {
                Some(destination) => {
                    self.lower_block_value_into(block, destination, origin)?;
                }
                None => self.lower_block(block)?,
            },
            None => return Err(self.inconsistent(origin, "match arm has no body")),
        }
        Ok(())
    }

    /// Match a loop item and continue from a fresh success block when the
    /// pattern is refutable. `failure` is the loop header, so mismatches
    /// advance the source iterator rather than entering the user body.
    pub(super) fn lower_loop_pattern(
        &mut self,
        pattern: HirPatternId,
        value: MirOperand,
        failure: crate::MirBlockId,
    ) -> Result<(), MirBuildError> {
        if self.pattern_is_refutable(pattern)? {
            self.lower_pattern_or_branch(pattern, value, failure)
        } else {
            self.lower_irrefutable_pattern(pattern, value)
        }
    }

    /// Preserve the production let-pattern contract: destructuring binds
    /// values but does not perform match-style refutability checks. Pathless
    /// tuples retain their trapping type/arity guard, while constructor fields
    /// are projected only when their subtree declares a local.
    pub(super) fn lower_let_pattern(
        &mut self,
        pattern: HirPatternId,
        value: MirOperand,
        statement_origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let bindings = self.build_pattern_binding_plan(pattern)?;
        self.materialize_pattern_bindings(&bindings, value, Some(statement_origin))
    }

    pub(super) fn pattern_origin(
        &self,
        pattern: HirPatternId,
    ) -> Result<MirSourceOrigin, MirBuildError> {
        let pattern = self.body.patterns.get(&pattern).ok_or_else(|| {
            self.inconsistent(
                MirSourceOrigin::body(self.body.id, self.body.origin.span),
                format!("missing HIR pattern {pattern:?}"),
            )
        })?;
        Ok(MirSourceOrigin::pattern(
            self.body.id,
            pattern.id,
            pattern.origin.span,
        ))
    }

    fn lower_irrefutable_pattern(
        &mut self,
        pattern: HirPatternId,
        value: MirOperand,
    ) -> Result<(), MirBuildError> {
        let bindings = self.build_pattern_binding_plan(pattern)?;
        self.materialize_pattern_bindings(&bindings, value, None)
    }

    fn build_pattern_binding_plan(
        &self,
        pattern: HirPatternId,
    ) -> Result<PatternBindingPlan, MirBuildError> {
        self.require_pattern_fact(pattern)?;
        let kind = self
            .body
            .patterns
            .get(&pattern)
            .ok_or_else(|| {
                self.inconsistent(
                    self.body_origin_for_patterns(),
                    format!("missing HIR pattern {pattern:?}"),
                )
            })?
            .kind
            .clone();
        match kind {
            HirPatternKind::Binding { local: Some(local) } => {
                Ok(PatternBindingPlan::Bind { pattern, local })
            }
            HirPatternKind::Wildcard
            | HirPatternKind::Literal(Some(_))
            | HirPatternKind::Path { path: Some(_) } => Ok(PatternBindingPlan::None),
            HirPatternKind::TupleVariant { path: None, fields } => {
                let origin = self.pattern_origin(pattern)?;
                let arity = u32::try_from(fields.len())
                    .map_err(|_| self.inconsistent(origin, "tuple pattern arity exceeds u32"))?;
                let mut bindings = Vec::new();
                for (index, field) in fields.into_iter().enumerate() {
                    let binding = self.build_pattern_binding_plan(field)?;
                    if binding.declares_local() {
                        bindings.push(PatternBindingProjection {
                            index,
                            pattern: field,
                            binding,
                        });
                    }
                }
                Ok(PatternBindingPlan::Tuple {
                    pattern,
                    arity,
                    fields: bindings,
                })
            }
            HirPatternKind::TupleVariant {
                path: Some(_),
                fields,
            } => {
                let layout = self.pattern_layout(pattern)?;
                let expected = (0..fields.len())
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>();
                self.require_layout_field_count(pattern, &layout, &expected)?;
                let mut bindings = Vec::new();
                for (index, field) in fields.into_iter().enumerate() {
                    let binding = self.build_pattern_binding_plan(field)?;
                    if binding.declares_local() {
                        bindings.push(PatternBindingProjection {
                            index,
                            pattern: field,
                            binding,
                        });
                    }
                }
                Ok(PatternBindingPlan::Constructor {
                    layout,
                    fields: bindings,
                })
            }
            HirPatternKind::RecordVariant {
                path: Some(_),
                fields,
            } => {
                let layout = self.pattern_layout(pattern)?;
                let expected = fields
                    .iter()
                    .map(|field| field.name.clone())
                    .collect::<Vec<_>>();
                self.require_layout_field_count(pattern, &layout, &expected)?;
                let mut bindings = Vec::new();
                for (index, field) in fields.into_iter().enumerate() {
                    let Some(nested) = field.pattern else {
                        continue;
                    };
                    let binding = self.build_pattern_binding_plan(nested)?;
                    if binding.declares_local() {
                        bindings.push(PatternBindingProjection {
                            index,
                            pattern: nested,
                            binding,
                        });
                    }
                }
                Ok(PatternBindingPlan::Constructor {
                    layout,
                    fields: bindings,
                })
            }
            // The direct production backend does not bind fields from an
            // incomplete/pathless record constructor pattern.
            HirPatternKind::RecordVariant { path: None, .. } => Ok(PatternBindingPlan::None),
            HirPatternKind::Binding { local: None }
            | HirPatternKind::Literal(None)
            | HirPatternKind::Path { path: None }
            | HirPatternKind::Missing => Err(self.inconsistent(
                self.pattern_origin(pattern)?,
                "incomplete let pattern reached MIR",
            )),
        }
    }

    fn materialize_pattern_bindings(
        &mut self,
        bindings: &PatternBindingPlan,
        value: MirOperand,
        binding_origin: Option<MirSourceOrigin>,
    ) -> Result<(), MirBuildError> {
        match bindings {
            PatternBindingPlan::None => Ok(()),
            PatternBindingPlan::Bind { pattern, local } => {
                let origin = match binding_origin {
                    Some(origin) => origin,
                    None => self.pattern_origin(*pattern)?,
                };
                let local = self.local(*local, origin)?;
                self.function.append_statement(
                    self.current_block,
                    MirStatement::assign(origin, MirPlace::local(local), MirRvalue::Use(value)),
                )?;
                Ok(())
            }
            PatternBindingPlan::Tuple {
                pattern,
                arity,
                fields,
            } => {
                self.emit_tuple_arity_guard(value.clone(), *arity, self.pattern_origin(*pattern)?)?;
                for field in fields {
                    let projected = self.project_tuple_pattern_field(
                        value.clone(),
                        field.index,
                        field.pattern,
                    )?;
                    self.materialize_pattern_bindings(&field.binding, projected, binding_origin)?;
                }
                Ok(())
            }
            PatternBindingPlan::Constructor { layout, fields } => {
                for field in fields {
                    let projected = self.project_constructor_field(
                        value.clone(),
                        layout,
                        field.index,
                        field.pattern,
                    )?;
                    self.materialize_pattern_bindings(&field.binding, projected, binding_origin)?;
                }
                Ok(())
            }
        }
    }

    fn lower_pattern_or_branch(
        &mut self,
        pattern: HirPatternId,
        value: MirOperand,
        failure: crate::MirBlockId,
    ) -> Result<(), MirBuildError> {
        let bindings = self.build_pattern_binding_plan(pattern)?;
        self.lower_pattern_checks(pattern, value.clone(), failure)?;
        self.materialize_pattern_bindings(&bindings, value, None)
    }

    fn lower_pattern_checks(
        &mut self,
        pattern: HirPatternId,
        value: MirOperand,
        failure: crate::MirBlockId,
    ) -> Result<(), MirBuildError> {
        self.require_pattern_fact(pattern)?;
        let kind = self
            .body
            .patterns
            .get(&pattern)
            .ok_or_else(|| {
                self.inconsistent(
                    self.body_origin_for_patterns(),
                    format!("missing HIR pattern {pattern:?}"),
                )
            })?
            .kind
            .clone();
        match kind {
            HirPatternKind::Binding { local: Some(_) } => Ok(()),
            HirPatternKind::Wildcard => Ok(()),
            HirPatternKind::Literal(Some(literal)) => {
                self.lower_literal_pattern(pattern, value, &literal, failure)
            }
            HirPatternKind::Path { path: Some(_) } => {
                let layout = self.pattern_layout(pattern)?;
                self.require_layout_field_count(pattern, &layout, &[])?;
                self.emit_constructor_predicate(pattern, value, &layout, failure)
            }
            HirPatternKind::TupleVariant { path: None, fields } => {
                let pattern_origin = self.pattern_origin(pattern)?;
                let arity = u32::try_from(fields.len()).map_err(|_| {
                    self.inconsistent(pattern_origin, "tuple pattern arity exceeds u32")
                })?;
                self.emit_pattern_predicate(
                    pattern,
                    MirPatternPredicate::TupleArity {
                        value: value.clone(),
                        arity,
                    },
                    failure,
                )?;
                for (index, field) in fields.into_iter().enumerate() {
                    if !self.pattern_needs_check(field)? {
                        continue;
                    }
                    let projected =
                        self.project_tuple_pattern_field(value.clone(), index, field)?;
                    self.lower_pattern_checks(field, projected, failure)?;
                }
                Ok(())
            }
            HirPatternKind::TupleVariant {
                path: Some(_),
                fields,
            } => {
                let layout = self.pattern_layout(pattern)?;
                let expected = (0..fields.len())
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>();
                self.require_layout_field_count(pattern, &layout, &expected)?;
                self.emit_constructor_predicate(pattern, value.clone(), &layout, failure)?;
                for (index, field) in fields.into_iter().enumerate() {
                    if !self.pattern_needs_check(field)? {
                        continue;
                    }
                    let projected =
                        self.project_constructor_field(value.clone(), &layout, index, field)?;
                    self.lower_pattern_checks(field, projected, failure)?;
                }
                Ok(())
            }
            HirPatternKind::RecordVariant {
                path: Some(_),
                fields,
            } => {
                let layout = self.pattern_layout(pattern)?;
                let expected = fields
                    .iter()
                    .map(|field| field.name.clone())
                    .collect::<Vec<_>>();
                self.require_layout_field_count(pattern, &layout, &expected)?;
                let pattern_origin = self.pattern_origin(pattern)?;
                self.emit_constructor_predicate(pattern, value.clone(), &layout, failure)?;
                for (index, field) in fields.into_iter().enumerate() {
                    let nested = field.pattern.ok_or_else(|| {
                        self.inconsistent(
                            pattern_origin,
                            format!(
                                "record pattern field {:?} has no nested HIR pattern",
                                field.name
                            ),
                        )
                    })?;
                    if !self.pattern_needs_check(nested)? {
                        continue;
                    }
                    let projected =
                        self.project_constructor_field(value.clone(), &layout, index, nested)?;
                    self.lower_pattern_checks(nested, projected, failure)?;
                }
                Ok(())
            }
            HirPatternKind::Binding { local: None }
            | HirPatternKind::Literal(None)
            | HirPatternKind::Path { path: None }
            | HirPatternKind::RecordVariant { path: None, .. }
            | HirPatternKind::Missing => Err(self.inconsistent(
                self.pattern_origin(pattern)?,
                "incomplete pattern reached MIR",
            )),
        }
    }

    fn lower_literal_pattern(
        &mut self,
        pattern: HirPatternId,
        value: MirOperand,
        literal: &HirLiteral,
        failure: crate::MirBlockId,
    ) -> Result<(), MirBuildError> {
        let origin = self.pattern_origin(pattern)?;
        let literal = self.pattern_literal_operand(pattern, literal, origin)?;
        let result = self.function.add_temp(
            MirValueType::Primitive(vela_common::PrimitiveTag::Bool),
            origin,
        );
        let safepoint = self.function.add_safepoint(MirSafepoint::new(origin));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(result)),
                MirStatementKind::DynamicBinary {
                    operation: MirDynamicBinaryOp::Equal,
                    left: value,
                    right: literal,
                },
                MirEffect::dynamic_call(),
                Some(safepoint),
            ),
        )?;
        self.branch_pattern(MirOperand::Temp(result), failure, origin)
    }

    fn pattern_literal_operand(
        &mut self,
        pattern: HirPatternId,
        literal: &HirLiteral,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        match literal {
            HirLiteral::Bool(value) => self.define_immediate_constant(
                MirImmediate::Bool(*value),
                MirConstantProvenance::PatternLiteral,
                origin,
            ),
            HirLiteral::Char(value) => self.define_immediate_constant(
                MirImmediate::Char(*value),
                MirConstantProvenance::PatternLiteral,
                origin,
            ),
            HirLiteral::Integer(_) | HirLiteral::Float(_) => {
                match self.input.analysis().pattern_literal(pattern) {
                    Some(Ok(ResolvedLiteralFact::Scalar(value))) => self.define_immediate_constant(
                        MirImmediate::Scalar(value.value()),
                        MirConstantProvenance::PatternLiteral,
                        origin,
                    ),
                    Some(Ok(ResolvedLiteralFact::Deferred(_))) => Err(self.inconsistent(
                        origin,
                        "pattern literal unexpectedly retained dynamic contextualization",
                    )),
                    Some(Err(error)) => Err(self.inconsistent(
                        origin,
                        format!(
                            "invalid numeric pattern reached MIR after diagnostics: {}",
                            error.detail()
                        ),
                    )),
                    None => Err(self.inconsistent(
                        origin,
                        "numeric pattern has no validated analysis literal fact",
                    )),
                }
            }
            HirLiteral::String(value) => self.materialize_pattern_constant(
                origin,
                MirEvaluatedConstant::String(value.clone()),
                MirValueType::Primitive(vela_common::PrimitiveTag::String),
            ),
            HirLiteral::Bytes(value) => self.materialize_pattern_constant(
                origin,
                MirEvaluatedConstant::Bytes(value.clone()),
                MirValueType::Primitive(vela_common::PrimitiveTag::Bytes),
            ),
            HirLiteral::Interpolated { .. } | HirLiteral::Invalid { .. } => Err(self.inconsistent(
                origin,
                "invalid literal pattern reached MIR after diagnostics",
            )),
        }
    }

    fn materialize_pattern_constant(
        &mut self,
        origin: MirSourceOrigin,
        value: MirEvaluatedConstant,
        value_type: MirValueType,
    ) -> Result<MirOperand, MirBuildError> {
        let temp = self.function.add_temp(value_type, origin);
        let safepoint = self.function.add_safepoint(MirSafepoint::new(origin));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(temp)),
                MirStatementKind::MaterializeConstant(value),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )?;
        Ok(MirOperand::Temp(temp))
    }

    fn pattern_layout(&self, pattern: HirPatternId) -> Result<PatternFieldLayout, MirBuildError> {
        let origin = self.pattern_origin(pattern)?;
        let target = self
            .input
            .targets()
            .pattern_constructor(pattern)
            .cloned()
            .ok_or_else(|| {
                self.inconsistent(
                    origin,
                    "pattern constructor has no compile-target placement",
                )
            })?;
        Ok(match target {
            CompilePatternConstructorTarget::NeverMatchesRecord { fields, .. } => {
                PatternFieldLayout::NeverMatchesRecord { fields }
            }
            CompilePatternConstructorTarget::Variant {
                type_id,
                variant,
                fields,
            } => PatternFieldLayout::Variant {
                type_id,
                variant,
                fields,
            },
            CompilePatternConstructorTarget::DynamicVariant {
                owner_name,
                variant_name,
                fields,
            } => PatternFieldLayout::DynamicVariant {
                owner_name,
                variant_name,
                fields,
            },
        })
    }

    fn require_layout_field_count(
        &self,
        pattern: HirPatternId,
        layout: &PatternFieldLayout,
        expected_names: &[String],
    ) -> Result<(), MirBuildError> {
        let origin = self.pattern_origin(pattern)?;
        let actual_len = match layout {
            PatternFieldLayout::NeverMatchesRecord { fields, .. }
            | PatternFieldLayout::Variant { fields, .. } => fields.len(),
            PatternFieldLayout::DynamicVariant { fields, .. } => fields.len(),
        };
        if actual_len != expected_names.len() {
            return Err(self.inconsistent(
                origin,
                format!(
                    "pattern compile target has {actual_len} fields but HIR has {}",
                    expected_names.len()
                ),
            ));
        }
        match layout {
            PatternFieldLayout::NeverMatchesRecord { fields, .. }
            | PatternFieldLayout::Variant { fields, .. } => {
                for (field, expected) in fields.iter().zip(expected_names) {
                    let descriptor =
                        self.input
                            .targets()
                            .field_descriptor(*field)
                            .ok_or_else(|| {
                                self.inconsistent(origin, "pattern field descriptor disappeared")
                            })?;
                    if descriptor.name != *expected {
                        return Err(self.inconsistent(
                            origin,
                            format!(
                                "pattern field target {:?} is named {:?}, expected {expected:?}",
                                field, descriptor.name
                            ),
                        ));
                    }
                }
            }
            PatternFieldLayout::DynamicVariant { fields, .. } => {
                if fields != expected_names {
                    return Err(
                        self.inconsistent(origin, "dynamic pattern field order disagrees with HIR")
                    );
                }
            }
        }
        Ok(())
    }

    fn emit_constructor_predicate(
        &mut self,
        pattern: HirPatternId,
        value: MirOperand,
        layout: &PatternFieldLayout,
        failure: crate::MirBlockId,
    ) -> Result<(), MirBuildError> {
        let predicate = match layout {
            PatternFieldLayout::NeverMatchesRecord { .. } => {
                MirPatternPredicate::NeverMatches { value }
            }
            PatternFieldLayout::Variant {
                type_id, variant, ..
            } => MirPatternPredicate::VariantShape {
                value,
                type_id: *type_id,
                variant: *variant,
            },
            PatternFieldLayout::DynamicVariant {
                owner_name,
                variant_name,
                ..
            } => MirPatternPredicate::DynamicVariant {
                value,
                owner_name: owner_name.clone(),
                variant_name: variant_name.clone(),
            },
        };
        self.emit_pattern_predicate(pattern, predicate, failure)
    }

    fn emit_pattern_predicate(
        &mut self,
        pattern: HirPatternId,
        predicate: MirPatternPredicate,
        failure: crate::MirBlockId,
    ) -> Result<(), MirBuildError> {
        let origin = self.pattern_origin(pattern)?;
        let result = self.function.add_temp(
            MirValueType::Primitive(vela_common::PrimitiveTag::Bool),
            origin,
        );
        self.function.append_statement(
            self.current_block,
            MirStatement::assign(
                origin,
                MirPlace::temp(result),
                MirRvalue::PatternPredicate(predicate),
            ),
        )?;
        self.branch_pattern(MirOperand::Temp(result), failure, origin)
    }

    fn emit_tuple_arity_guard(
        &mut self,
        value: MirOperand,
        arity: u32,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let guard = self.function.add_guard(MirGuard {
            kind: crate::MirGuardKind::Contract,
            assumption: MirGuardAssumption::TupleArity { arity },
            context: None,
            origin,
        });
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                None,
                MirStatementKind::GuardTrap { value, guard },
                MirEffect::may_trap(),
                None,
            ),
        )?;
        Ok(())
    }

    fn branch_pattern(
        &mut self,
        condition: MirOperand,
        failure: crate::MirBlockId,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let passed = self.function.add_block();
        self.function.set_terminator(
            self.current_block,
            MirTerminator::new(
                origin,
                MirTerminatorKind::Branch {
                    condition,
                    then_block: passed,
                    else_block: failure,
                },
                MirEffect::PURE,
                None,
            ),
        )?;
        self.current_block = passed;
        Ok(())
    }

    fn project_tuple_pattern_field(
        &mut self,
        value: MirOperand,
        index: usize,
        nested: HirPatternId,
    ) -> Result<MirOperand, MirBuildError> {
        let origin = self.pattern_origin(nested)?;
        let index = u32::try_from(index)
            .map_err(|_| self.inconsistent(origin, "tuple pattern index exceeds u32"))?;
        let temp = self.pattern_projection_temp(nested, origin);
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(temp)),
                MirStatementKind::TupleField {
                    tuple: value,
                    index,
                },
                MirEffect::may_trap(),
                None,
            ),
        )?;
        Ok(MirOperand::Temp(temp))
    }

    fn project_constructor_field(
        &mut self,
        value: MirOperand,
        layout: &PatternFieldLayout,
        index: usize,
        nested: HirPatternId,
    ) -> Result<MirOperand, MirBuildError> {
        let origin = self.pattern_origin(nested)?;
        let target = match layout {
            PatternFieldLayout::NeverMatchesRecord { fields, .. } => {
                let field = fields.get(index).ok_or_else(|| {
                    self.inconsistent(origin, "never-match record field index is out of bounds")
                })?;
                let name = self
                    .input
                    .targets()
                    .field_descriptor(*field)
                    .ok_or_else(|| {
                        self.inconsistent(origin, "never-match record field descriptor disappeared")
                    })?
                    .name
                    .clone();
                MirFieldTarget::DynamicVariant { name }
            }
            PatternFieldLayout::Variant {
                type_id,
                variant,
                fields,
            } => MirFieldTarget::VariantSlot {
                type_id: *type_id,
                variant: *variant,
                field: *fields.get(index).ok_or_else(|| {
                    self.inconsistent(origin, "variant pattern field index is out of bounds")
                })?,
            },
            PatternFieldLayout::DynamicVariant { fields, .. } => MirFieldTarget::DynamicVariant {
                name: fields.get(index).cloned().ok_or_else(|| {
                    self.inconsistent(origin, "dynamic pattern field index is out of bounds")
                })?,
            },
        };
        let temp = self.pattern_projection_temp(nested, origin);
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(temp)),
                MirStatementKind::ReadField {
                    receiver: value,
                    target,
                },
                MirEffect::may_trap(),
                None,
            ),
        )?;
        Ok(MirOperand::Temp(temp))
    }

    fn pattern_projection_temp(
        &mut self,
        pattern: HirPatternId,
        origin: MirSourceOrigin,
    ) -> crate::MirTempId {
        let value_type = value_type(self.input.analysis().pattern(pattern));
        self.function.add_temp(value_type, origin)
    }

    fn pattern_is_refutable(&self, pattern: HirPatternId) -> Result<bool, MirBuildError> {
        self.require_pattern_fact(pattern)?;
        let record = self.body.patterns.get(&pattern).ok_or_else(|| {
            self.inconsistent(
                self.body_origin_for_patterns(),
                format!("missing HIR pattern {pattern:?}"),
            )
        })?;
        match record.kind {
            HirPatternKind::Binding { local: Some(_) } | HirPatternKind::Wildcard => Ok(false),
            HirPatternKind::Binding { local: None }
            | HirPatternKind::Literal(None)
            | HirPatternKind::Path { path: None }
            | HirPatternKind::RecordVariant { path: None, .. }
            | HirPatternKind::Missing => Err(self.inconsistent(
                self.pattern_origin(pattern)?,
                "incomplete pattern reached MIR",
            )),
            HirPatternKind::TupleVariant { .. }
            | HirPatternKind::RecordVariant { .. }
            | HirPatternKind::Path { .. }
            | HirPatternKind::Literal(Some(_)) => Ok(true),
        }
    }

    fn require_pattern_fact(&self, pattern: HirPatternId) -> Result<(), MirBuildError> {
        if self.input.analysis().pattern(pattern).is_none() {
            return Err(self.inconsistent(
                self.pattern_origin(pattern)?,
                "pattern has no analysis type fact",
            ));
        }
        Ok(())
    }

    fn pattern_needs_check(&self, pattern: HirPatternId) -> Result<bool, MirBuildError> {
        let record = self.body.patterns.get(&pattern).ok_or_else(|| {
            self.inconsistent(
                self.body_origin_for_patterns(),
                format!("missing HIR pattern {pattern:?}"),
            )
        })?;
        match record.kind {
            HirPatternKind::Binding { local: Some(_) } | HirPatternKind::Wildcard => Ok(false),
            HirPatternKind::TupleVariant { .. }
            | HirPatternKind::RecordVariant { .. }
            | HirPatternKind::Path { path: Some(_) }
            | HirPatternKind::Literal(Some(_)) => Ok(true),
            HirPatternKind::Binding { local: None }
            | HirPatternKind::Literal(None)
            | HirPatternKind::Path { path: None }
            | HirPatternKind::Missing => Err(self.inconsistent(
                self.pattern_origin(pattern)?,
                "incomplete pattern reached MIR",
            )),
        }
    }

    fn jump_pattern(
        &mut self,
        target: crate::MirBlockId,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.function.set_terminator(
            self.current_block,
            MirTerminator::new(
                origin,
                MirTerminatorKind::Jump(target),
                MirEffect::PURE,
                None,
            ),
        )
    }

    fn pattern_expression_origin(
        &self,
        expression: HirExprId,
    ) -> Result<MirSourceOrigin, MirBuildError> {
        let expression = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                self.body_origin_for_patterns(),
                format!("missing HIR expression {expression:?}"),
            )
        })?;
        Ok(MirSourceOrigin::expression(
            self.body.id,
            expression.id,
            expression.origin.span,
        ))
    }

    fn body_origin_for_patterns(&self) -> MirSourceOrigin {
        MirSourceOrigin::body(self.body.id, self.body.origin.span)
    }
}
