use vela_analysis::literals::ResolvedLiteralFact;
use vela_common::PrimitiveTag;
use vela_hir::body::HirLiteral;
use vela_hir::ids::HirExprId;

use crate::{
    MirBuildError, MirEffect, MirEvaluatedConstant, MirImmediate, MirOperand, MirPlace,
    MirSafepoint, MirSourceOrigin, MirStatement, MirStatementKind, MirValueType,
};

use super::core::FunctionBuilder;

impl FunctionBuilder<'_> {
    pub(super) fn lower_literal(
        &mut self,
        expression: HirExprId,
        literal: &HirLiteral,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        match literal {
            HirLiteral::Bool(value) => Ok(MirOperand::Immediate(MirImmediate::Bool(*value))),
            HirLiteral::Char(value) => Ok(MirOperand::Immediate(MirImmediate::Char(*value))),
            HirLiteral::Integer(_) | HirLiteral::Float(_) => {
                let analysis = self.input.analysis();
                let literal = analysis.literal(expression).ok_or_else(|| {
                    self.inconsistent(origin, "numeric literal has no validated analysis fact")
                })?;
                match literal {
                    Ok(ResolvedLiteralFact::Scalar(value)) => {
                        Ok(MirOperand::Immediate(MirImmediate::Scalar(value.value())))
                    }
                    Ok(ResolvedLiteralFact::Deferred(_)) => Err(self.inconsistent(
                        origin,
                        "standalone literal unexpectedly retained dynamic contextualization",
                    )),
                    Err(error) => Err(self.inconsistent(
                        origin,
                        format!(
                            "invalid numeric literal reached MIR after diagnostics: {}",
                            error.detail()
                        ),
                    )),
                }
            }
            HirLiteral::String(value) => self.materialize_constant(
                origin,
                MirEvaluatedConstant::String(value.clone()),
                MirValueType::Primitive(PrimitiveTag::String),
            ),
            HirLiteral::Bytes(value) => self.materialize_constant(
                origin,
                MirEvaluatedConstant::Bytes(value.clone()),
                MirValueType::Primitive(PrimitiveTag::Bytes),
            ),
            HirLiteral::Interpolated { .. } => {
                Err(self.unsupported(origin, "interpolated string expression"))
            }
            HirLiteral::Invalid { .. } => Err(self.inconsistent(
                origin,
                "invalid literal reached MIR after semantic diagnostics",
            )),
        }
    }

    fn materialize_constant(
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
}
