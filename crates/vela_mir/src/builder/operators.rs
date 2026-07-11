use vela_analysis::literals::{NumericLiteralUse, ResolvedLiteralFact};
use vela_analysis::semantic_facts::OperatorTargetFact;
use vela_analysis::type_fact::TypeFact;
use vela_common::PrimitiveTag;
use vela_hir::body::{HirBinaryOp, HirUnaryOp};
use vela_hir::ids::HirExprId;

use crate::{
    MirBinaryOp, MirBuildError, MirComparisonOp, MirConstantProvenance, MirContextualBinaryOp,
    MirDynamicBinaryOp, MirDynamicUnaryOp, MirEffect, MirIdentityOp, MirImmediate, MirLiteralSide,
    MirNumericBinaryOp, MirOperand, MirPlace, MirSafepoint, MirSourceOrigin, MirStatement,
    MirStatementKind, MirUnaryOp,
};

use super::core::{FunctionBuilder, value_type};

impl FunctionBuilder<'_> {
    pub(super) fn lower_unary(
        &mut self,
        expression: HirExprId,
        operation: Option<HirUnaryOp>,
        operand: Option<HirExprId>,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let operation = operation
            .ok_or_else(|| self.inconsistent(origin, "unary expression has no operator"))?;
        let operand =
            operand.ok_or_else(|| self.inconsistent(origin, "unary expression has no operand"))?;
        let proven = match self.operator_target(expression, origin)? {
            OperatorTargetFact::Unary(target) if target == operation => true,
            OperatorTargetFact::Unary(_) => {
                return Err(self.inconsistent(
                    origin,
                    "analysis unary target disagrees with the HIR operator",
                ));
            }
            OperatorTargetFact::Dynamic => false,
            OperatorTargetFact::Unresolved => {
                return Err(self.inconsistent(origin, "unresolved unary operator reached MIR"));
            }
            OperatorTargetFact::Binary(_) | OperatorTargetFact::Assignment(_) => {
                return Err(self.inconsistent(
                    origin,
                    "analysis operator target has the wrong unary family",
                ));
            }
        };

        if operation == HirUnaryOp::Negate
            && let Some(literal) = self.input.analysis().literal(expression)
        {
            return match literal {
                Ok(ResolvedLiteralFact::Scalar(value)) => self.define_immediate_constant(
                    MirImmediate::Scalar(value.value()),
                    MirConstantProvenance::FoldedLiteral,
                    origin,
                ),
                Ok(ResolvedLiteralFact::Deferred(_)) => Err(self.inconsistent(
                    origin,
                    "negated literal unexpectedly retained dynamic contextualization",
                )),
                Err(error) => Err(self.inconsistent(
                    origin,
                    format!(
                        "invalid negated literal reached MIR after diagnostics: {}",
                        error.detail()
                    ),
                )),
            };
        }

        let operand_fact = self.expression_fact(operand, origin)?;
        let operand = self.lower_expression(operand)?;
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(MirImmediate::Unit));
        }
        let kind = if proven {
            typed_unary(operation, &operand_fact).map_or_else(
                || MirStatementKind::DynamicUnary {
                    operation: dynamic_unary(operation),
                    operand: operand.clone(),
                },
                |operation| MirStatementKind::Unary {
                    operation,
                    operand: operand.clone(),
                },
            )
        } else {
            MirStatementKind::DynamicUnary {
                operation: dynamic_unary(operation),
                operand,
            }
        };
        self.append_operator(expression, origin, kind, MirEffect::may_trap())
    }

    pub(super) fn lower_binary(
        &mut self,
        expression: HirExprId,
        operation: Option<HirBinaryOp>,
        left: Option<HirExprId>,
        right: Option<HirExprId>,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let operation = operation
            .ok_or_else(|| self.inconsistent(origin, "binary expression has no operator"))?;
        let left =
            left.ok_or_else(|| self.inconsistent(origin, "binary expression has no left operand"))?;
        let right = right
            .ok_or_else(|| self.inconsistent(origin, "binary expression has no right operand"))?;
        if matches!(operation, HirBinaryOp::And | HirBinaryOp::Or) {
            return self.lower_short_circuit(expression, operation, left, right, origin);
        }
        let proven = match self.operator_target(expression, origin)? {
            OperatorTargetFact::Binary(target) if target == operation => true,
            OperatorTargetFact::Binary(_) => {
                return Err(self.inconsistent(
                    origin,
                    "analysis binary target disagrees with the HIR operator",
                ));
            }
            OperatorTargetFact::Dynamic => false,
            OperatorTargetFact::Unresolved => {
                return Err(self.inconsistent(origin, "unresolved binary operator reached MIR"));
            }
            OperatorTargetFact::Unary(_) | OperatorTargetFact::Assignment(_) => {
                return Err(self.inconsistent(
                    origin,
                    "analysis operator target has the wrong binary family",
                ));
            }
        };

        if matches!(
            operation,
            HirBinaryOp::IdentityEqual | HirBinaryOp::IdentityNotEqual
        ) {
            let left = self.lower_expression(left)?;
            if self.current_is_terminated()? {
                return Ok(MirOperand::Immediate(MirImmediate::Unit));
            }
            let left = self.capture_operand(left, origin)?;
            let right = self.lower_expression(right)?;
            if self.current_is_terminated()? {
                return Ok(MirOperand::Immediate(MirImmediate::Unit));
            }
            return self.append_operator(
                expression,
                origin,
                MirStatementKind::IdentityCompare {
                    operation: match operation {
                        HirBinaryOp::IdentityEqual => MirIdentityOp::Equal,
                        HirBinaryOp::IdentityNotEqual => MirIdentityOp::NotEqual,
                        _ => unreachable!("identity operation was checked"),
                    },
                    left,
                    right,
                },
                MirEffect::may_trap(),
            );
        }
        if matches!(operation, HirBinaryOp::Range | HirBinaryOp::RangeInclusive) {
            let start = self.lower_expression(left)?;
            if self.current_is_terminated()? {
                return Ok(MirOperand::Immediate(MirImmediate::Unit));
            }
            let start = self.capture_operand(start, origin)?;
            let end = self.lower_expression(right)?;
            if self.current_is_terminated()? {
                return Ok(MirOperand::Immediate(MirImmediate::Unit));
            }
            return self.append_operator(
                expression,
                origin,
                MirStatementKind::MakeRange {
                    start,
                    end,
                    inclusive: operation == HirBinaryOp::RangeInclusive,
                },
                MirEffect::may_trap(),
            );
        }
        if !proven
            && let Some(contextual) =
                self.lower_contextual_numeric(expression, operation, left, right, origin)?
        {
            return Ok(contextual);
        }

        let left_fact = self.expression_fact(left, origin)?;
        let right_fact = self.expression_fact(right, origin)?;
        let left = self.lower_expression(left)?;
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(MirImmediate::Unit));
        }
        let left = self.capture_operand(left, origin)?;
        let right = self.lower_expression(right)?;
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(MirImmediate::Unit));
        }
        if proven && let Some(operation) = typed_binary(operation, &left_fact, &right_fact) {
            return self.append_operator(
                expression,
                origin,
                MirStatementKind::Binary {
                    operation,
                    left,
                    right,
                },
                MirEffect::may_trap(),
            );
        }

        let operation = dynamic_binary(operation);
        let effect = if dynamic_comparison(operation) {
            MirEffect::dynamic_call()
        } else {
            MirEffect::may_trap()
        };
        self.append_operator(
            expression,
            origin,
            MirStatementKind::DynamicBinary {
                operation,
                left,
                right,
            },
            effect,
        )
    }

    fn lower_contextual_numeric(
        &mut self,
        expression: HirExprId,
        operation: HirBinaryOp,
        left: HirExprId,
        right: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<Option<MirOperand>, MirBuildError> {
        let left_literal = NumericLiteralUse::classify(self.body, left);
        let right_literal = NumericLiteralUse::classify(self.body, right);
        let (value, literal, literal_side) = match (left_literal, right_literal) {
            (Some(literal), None) => (right, literal, MirLiteralSide::Left),
            (None, Some(literal)) => (left, literal, MirLiteralSide::Right),
            (Some(_), Some(_)) | (None, None) => return Ok(None),
        };
        let analysis = self.input.analysis();
        let Some(literal_fact) = analysis.literal(literal.resolution_expression()) else {
            return Ok(None);
        };
        let literal = match literal_fact {
            Ok(ResolvedLiteralFact::Scalar(_)) => return Ok(None),
            Ok(ResolvedLiteralFact::Deferred(literal)) => literal.clone(),
            Err(error) => {
                return Err(self.inconsistent(
                    origin,
                    format!(
                        "invalid contextual literal reached MIR after diagnostics: {}",
                        error.detail()
                    ),
                ));
            }
        };
        let operation = contextual_binary(operation).ok_or_else(|| {
            self.inconsistent(
                origin,
                "deferred numeric literal is attached to an unsupported binary operator",
            )
        })?;
        let value = self.lower_expression(value)?;
        if self.current_is_terminated()? {
            return Ok(Some(MirOperand::Immediate(MirImmediate::Unit)));
        }
        self.append_operator(
            expression,
            origin,
            MirStatementKind::ContextualNumericBinary {
                operation,
                value,
                literal: literal.into(),
                literal_side,
            },
            MirEffect::may_trap(),
        )
        .map(Some)
    }

    fn operator_target(
        &self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<OperatorTargetFact, MirBuildError> {
        self.input
            .analysis()
            .operator_target(expression)
            .ok_or_else(|| self.inconsistent(origin, "operator expression has no analysis target"))
    }

    fn expression_fact(
        &self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<TypeFact, MirBuildError> {
        self.input
            .analysis()
            .expression(expression)
            .cloned()
            .ok_or_else(|| self.inconsistent(origin, "operator operand has no analysis type fact"))
    }

    fn append_operator(
        &mut self,
        expression: HirExprId,
        origin: MirSourceOrigin,
        kind: MirStatementKind,
        effect: MirEffect,
    ) -> Result<MirOperand, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(MirImmediate::Unit));
        }
        let result_fact = self.expression_fact(expression, origin)?;
        let destination = self
            .function
            .add_temp(value_type(Some(&result_fact)), origin);
        let safepoint = if effect.requires_safepoint() {
            Some(self.function.add_safepoint(MirSafepoint::new(origin)))
        } else {
            None
        };
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(destination)),
                kind,
                effect,
                safepoint,
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }
}

fn typed_unary(operation: HirUnaryOp, operand: &TypeFact) -> Option<MirUnaryOp> {
    match (operation, operand) {
        (HirUnaryOp::Not, TypeFact::Primitive(PrimitiveTag::Bool)) => Some(MirUnaryOp::NotBool),
        (HirUnaryOp::Negate, TypeFact::Primitive(primitive)) => {
            primitive.numeric_tag().map(MirUnaryOp::Negate)
        }
        (HirUnaryOp::Not | HirUnaryOp::Negate, _) => None,
    }
}

const fn dynamic_unary(operation: HirUnaryOp) -> MirDynamicUnaryOp {
    match operation {
        HirUnaryOp::Not => MirDynamicUnaryOp::Not,
        HirUnaryOp::Negate => MirDynamicUnaryOp::Negate,
    }
}

fn typed_binary(operation: HirBinaryOp, left: &TypeFact, right: &TypeFact) -> Option<MirBinaryOp> {
    let (TypeFact::Primitive(left), TypeFact::Primitive(right)) = (left, right) else {
        return None;
    };
    if left != right {
        return None;
    }
    match operation {
        HirBinaryOp::Add
        | HirBinaryOp::Sub
        | HirBinaryOp::Mul
        | HirBinaryOp::Div
        | HirBinaryOp::Rem => Some(MirBinaryOp::Numeric {
            operation: match operation {
                HirBinaryOp::Add => MirNumericBinaryOp::Add,
                HirBinaryOp::Sub => MirNumericBinaryOp::Subtract,
                HirBinaryOp::Mul => MirNumericBinaryOp::Multiply,
                HirBinaryOp::Div => MirNumericBinaryOp::Divide,
                HirBinaryOp::Rem => MirNumericBinaryOp::Remainder,
                _ => unreachable!("numeric operation was checked"),
            },
            kind: left.numeric_tag()?,
        }),
        HirBinaryOp::Equal
        | HirBinaryOp::NotEqual
        | HirBinaryOp::Less
        | HirBinaryOp::LessEqual
        | HirBinaryOp::Greater
        | HirBinaryOp::GreaterEqual => Some(MirBinaryOp::Compare {
            operation: comparison(operation)?,
            kind: *left,
        }),
        HirBinaryOp::IdentityEqual
        | HirBinaryOp::IdentityNotEqual
        | HirBinaryOp::Range
        | HirBinaryOp::RangeInclusive
        | HirBinaryOp::And
        | HirBinaryOp::Or => None,
    }
}

const fn comparison(operation: HirBinaryOp) -> Option<MirComparisonOp> {
    match operation {
        HirBinaryOp::Equal => Some(MirComparisonOp::Equal),
        HirBinaryOp::NotEqual => Some(MirComparisonOp::NotEqual),
        HirBinaryOp::Less => Some(MirComparisonOp::Less),
        HirBinaryOp::LessEqual => Some(MirComparisonOp::LessEqual),
        HirBinaryOp::Greater => Some(MirComparisonOp::Greater),
        HirBinaryOp::GreaterEqual => Some(MirComparisonOp::GreaterEqual),
        HirBinaryOp::IdentityEqual
        | HirBinaryOp::IdentityNotEqual
        | HirBinaryOp::Range
        | HirBinaryOp::RangeInclusive
        | HirBinaryOp::Add
        | HirBinaryOp::Sub
        | HirBinaryOp::Mul
        | HirBinaryOp::Div
        | HirBinaryOp::Rem
        | HirBinaryOp::And
        | HirBinaryOp::Or => None,
    }
}

fn dynamic_binary(operation: HirBinaryOp) -> MirDynamicBinaryOp {
    match operation {
        HirBinaryOp::Add => MirDynamicBinaryOp::Add,
        HirBinaryOp::Sub => MirDynamicBinaryOp::Subtract,
        HirBinaryOp::Mul => MirDynamicBinaryOp::Multiply,
        HirBinaryOp::Div => MirDynamicBinaryOp::Divide,
        HirBinaryOp::Rem => MirDynamicBinaryOp::Remainder,
        HirBinaryOp::Equal => MirDynamicBinaryOp::Equal,
        HirBinaryOp::NotEqual => MirDynamicBinaryOp::NotEqual,
        HirBinaryOp::Less => MirDynamicBinaryOp::Less,
        HirBinaryOp::LessEqual => MirDynamicBinaryOp::LessEqual,
        HirBinaryOp::Greater => MirDynamicBinaryOp::Greater,
        HirBinaryOp::GreaterEqual => MirDynamicBinaryOp::GreaterEqual,
        HirBinaryOp::IdentityEqual
        | HirBinaryOp::IdentityNotEqual
        | HirBinaryOp::Range
        | HirBinaryOp::RangeInclusive
        | HirBinaryOp::And
        | HirBinaryOp::Or => unreachable!("dedicated binary operation was lowered earlier"),
    }
}

const fn contextual_binary(operation: HirBinaryOp) -> Option<MirContextualBinaryOp> {
    match operation {
        HirBinaryOp::Add => Some(MirContextualBinaryOp::Add),
        HirBinaryOp::Sub => Some(MirContextualBinaryOp::Subtract),
        HirBinaryOp::Mul => Some(MirContextualBinaryOp::Multiply),
        HirBinaryOp::Div => Some(MirContextualBinaryOp::Divide),
        HirBinaryOp::Rem => Some(MirContextualBinaryOp::Remainder),
        HirBinaryOp::Less => Some(MirContextualBinaryOp::Less),
        HirBinaryOp::LessEqual => Some(MirContextualBinaryOp::LessEqual),
        HirBinaryOp::Greater => Some(MirContextualBinaryOp::Greater),
        HirBinaryOp::GreaterEqual => Some(MirContextualBinaryOp::GreaterEqual),
        HirBinaryOp::Equal
        | HirBinaryOp::NotEqual
        | HirBinaryOp::IdentityEqual
        | HirBinaryOp::IdentityNotEqual
        | HirBinaryOp::Range
        | HirBinaryOp::RangeInclusive
        | HirBinaryOp::And
        | HirBinaryOp::Or => None,
    }
}

const fn dynamic_comparison(operation: MirDynamicBinaryOp) -> bool {
    matches!(
        operation,
        MirDynamicBinaryOp::Equal
            | MirDynamicBinaryOp::NotEqual
            | MirDynamicBinaryOp::Less
            | MirDynamicBinaryOp::LessEqual
            | MirDynamicBinaryOp::Greater
            | MirDynamicBinaryOp::GreaterEqual
    )
}
