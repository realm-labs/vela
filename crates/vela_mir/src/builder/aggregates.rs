use vela_hir::body::{HirExprKind, HirInterpolatedStringPart, HirMapEntry};
use vela_hir::ids::HirExprId;

use crate::{
    CompileCallArguments, CompileCalleeTarget, MirAggregate, MirBuildError, MirEffect,
    MirFormatPart, MirOperand, MirPlace, MirSafepoint, MirSourceOrigin, MirStatement,
    MirStatementKind,
};

use super::core::{FunctionBuilder, value_type};

impl FunctionBuilder<'_> {
    /// Lowers aggregate-shaped expressions owned by this builder module.
    ///
    /// Returning `None` means the expression belongs to another lowering
    /// responsibility. In particular, ordinary calls are never guessed to be
    /// set construction: only the explicit compile-target intrinsic is
    /// accepted here.
    pub(super) fn lower_aggregate_expression(
        &mut self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<Option<MirOperand>, MirBuildError> {
        let kind = self
            .body
            .expression(expression)
            .ok_or_else(|| {
                self.inconsistent(
                    origin,
                    format!("missing HIR expression {expression:?} for aggregate lowering"),
                )
            })?
            .kind
            .clone();
        match kind {
            HirExprKind::Tuple { elements } => {
                self.lower_tuple(expression, &elements, origin).map(Some)
            }
            HirExprKind::Array { elements } => {
                self.lower_array(expression, &elements, origin).map(Some)
            }
            HirExprKind::Map { entries } => self.lower_map(expression, &entries, origin).map(Some),
            HirExprKind::Literal(vela_hir::body::HirLiteral::Interpolated { parts }) => self
                .lower_interpolated_string(expression, &parts, origin)
                .map(Some),
            HirExprKind::Call(_) => self.lower_explicit_set_construction(expression, origin),
            HirExprKind::Literal(_)
            | HirExprKind::Path(_)
            | HirExprKind::Paren { .. }
            | HirExprKind::Unit
            | HirExprKind::Unary { .. }
            | HirExprKind::Binary { .. }
            | HirExprKind::Assign { .. }
            | HirExprKind::Field(_)
            | HirExprKind::Index(_)
            | HirExprKind::Try { .. }
            | HirExprKind::Record { .. }
            | HirExprKind::Lambda { .. }
            | HirExprKind::Block { .. }
            | HirExprKind::If(_)
            | HirExprKind::Match(_)
            | HirExprKind::Missing => Ok(None),
        }
    }

    pub(super) fn lower_tuple(
        &mut self,
        expression: HirExprId,
        elements: &[HirExprId],
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let elements = self.lower_aggregate_operands(elements)?;
        self.append_allocation(expression, origin, MirAggregate::Tuple(elements))
    }

    pub(super) fn lower_array(
        &mut self,
        expression: HirExprId,
        elements: &[HirExprId],
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let elements = self.lower_aggregate_operands(elements)?;
        self.append_allocation(expression, origin, MirAggregate::Array(elements))
    }

    pub(super) fn lower_map(
        &mut self,
        expression: HirExprId,
        entries: &[HirMapEntry],
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let mut values = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            let entry_origin =
                MirSourceOrigin::expression(self.body.id, expression, entry.origin.span);
            let key = entry.logical_key.clone().ok_or_else(|| {
                self.inconsistent(
                    entry_origin,
                    "map entry reached MIR without a validated HIR logical key",
                )
            })?;
            let value = entry.value.ok_or_else(|| {
                self.inconsistent(
                    entry_origin,
                    "map entry reached MIR without a value expression",
                )
            })?;
            let value_origin = self.operand_origin(value)?;
            let value = self.lower_aggregate_operand(value)?;
            if self.current_is_terminated()? {
                return Ok(MirOperand::Immediate(crate::MirImmediate::Unit));
            }
            let value = if index + 1 < entries.len() {
                self.capture_operand(value, value_origin)?
            } else {
                value
            };
            values.push((key, value));
        }
        self.append_allocation(expression, origin, MirAggregate::Map(values))
    }

    pub(super) fn lower_interpolated_string(
        &mut self,
        expression: HirExprId,
        parts: &[HirInterpolatedStringPart],
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if !parts
            .iter()
            .any(|part| matches!(part, HirInterpolatedStringPart::Expr(_)))
        {
            return Err(self.inconsistent(
                origin,
                "interpolated string reached MIR without an expression part",
            ));
        }

        let mut lowered = Vec::with_capacity(parts.len());
        for (index, part) in parts.iter().enumerate() {
            lowered.push(match part {
                HirInterpolatedStringPart::Text(text) => MirFormatPart::Text(text.clone()),
                HirInterpolatedStringPart::Expr(value) => {
                    let value_origin = self.operand_origin(*value)?;
                    let value = self.lower_aggregate_operand(*value)?;
                    if self.current_is_terminated()? {
                        return Ok(MirOperand::Immediate(crate::MirImmediate::Unit));
                    }
                    let has_later_value = parts[index + 1..]
                        .iter()
                        .any(|part| matches!(part, HirInterpolatedStringPart::Expr(_)));
                    let value = if has_later_value {
                        self.capture_operand(value, value_origin)?
                    } else {
                        value
                    };
                    MirFormatPart::Value(value)
                }
            });
        }
        self.append_effectful_result(
            expression,
            origin,
            MirStatementKind::FormatString { parts: lowered },
            MirEffect::allocation(),
        )
    }

    pub(super) fn lower_explicit_set_construction(
        &mut self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<Option<MirOperand>, MirBuildError> {
        let Some(target) = self.input.targets().call(expression).cloned() else {
            return Ok(None);
        };
        if !matches!(target.callee, CompileCalleeTarget::SetFromArray { .. }) {
            return Ok(None);
        }
        let CompileCallArguments::Positional(arguments) = target.arguments else {
            return Err(self.inconsistent(
                origin,
                "set-from-array compile target lost canonical positional arguments",
            ));
        };
        let [source] = arguments.as_slice() else {
            return Err(self.inconsistent(
                origin,
                "set-from-array compile target lost its single source operand",
            ));
        };
        let source = self.lower_aggregate_operand(*source)?;
        if self.current_is_terminated()? {
            return Ok(Some(MirOperand::Immediate(crate::MirImmediate::Unit)));
        }
        self.append_allocation(expression, origin, MirAggregate::SetFromArray { source })
            .map(Some)
    }

    fn lower_aggregate_operands(
        &mut self,
        expressions: &[HirExprId],
    ) -> Result<Vec<MirOperand>, MirBuildError> {
        let mut operands = Vec::with_capacity(expressions.len());
        for (index, expression) in expressions.iter().copied().enumerate() {
            let origin = self.operand_origin(expression)?;
            let operand = self.lower_aggregate_operand(expression)?;
            if self.current_is_terminated()? {
                break;
            }
            let operand = if index + 1 < expressions.len() {
                self.capture_operand(operand, origin)?
            } else {
                operand
            };
            operands.push(operand);
        }
        Ok(operands)
    }

    fn lower_aggregate_operand(
        &mut self,
        expression: HirExprId,
    ) -> Result<MirOperand, MirBuildError> {
        let origin = self.operand_origin(expression)?;
        if let Some(value) = self.lower_aggregate_expression(expression, origin)? {
            return Ok(value);
        }
        self.lower_expression(expression)
    }

    fn operand_origin(&self, expression: HirExprId) -> Result<MirSourceOrigin, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                MirSourceOrigin::body(self.body.id, self.body.origin.span),
                format!("missing aggregate operand expression {expression:?}"),
            )
        })?;
        Ok(MirSourceOrigin::expression(
            self.body.id,
            expression,
            record.origin.span,
        ))
    }

    fn append_allocation(
        &mut self,
        expression: HirExprId,
        origin: MirSourceOrigin,
        aggregate: MirAggregate,
    ) -> Result<MirOperand, MirBuildError> {
        self.append_effectful_result(
            expression,
            origin,
            MirStatementKind::Allocate(aggregate),
            MirEffect::allocation(),
        )
    }

    fn append_effectful_result(
        &mut self,
        expression: HirExprId,
        origin: MirSourceOrigin,
        kind: MirStatementKind,
        effect: MirEffect,
    ) -> Result<MirOperand, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(crate::MirImmediate::Unit));
        }
        let analysis = self.input.analysis();
        let fact = analysis.expression(expression).ok_or_else(|| {
            self.inconsistent(origin, "aggregate expression has no analysis type fact")
        })?;
        let destination = self.function.add_temp(value_type(Some(fact)), origin);
        let safepoint = self.function.add_safepoint(MirSafepoint::new(origin));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(destination)),
                kind,
                effect,
                Some(safepoint),
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }
}

#[cfg(test)]
#[path = "../tests/aggregates_builder.rs"]
mod tests;
