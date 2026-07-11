use vela_analysis::literals::ResolvedLiteralFact;
use vela_common::PrimitiveTag;
use vela_hir::body::HirLiteral;
use vela_hir::ids::HirExprId;

use crate::{
    MirBuildError, MirEvaluatedConstant, MirImmediate, MirOperand, MirSourceOrigin, MirValueType,
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
            HirLiteral::String(value) => self.lower_evaluated_constant(
                MirEvaluatedConstant::String(value.clone()),
                MirValueType::Primitive(PrimitiveTag::String),
                origin,
            ),
            HirLiteral::Bytes(value) => self.lower_evaluated_constant(
                MirEvaluatedConstant::Bytes(value.clone()),
                MirValueType::Primitive(PrimitiveTag::Bytes),
                origin,
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
}
