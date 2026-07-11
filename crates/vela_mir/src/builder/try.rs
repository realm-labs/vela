use vela_hir::ids::HirExprId;

use crate::{
    CompileTryLayoutTarget, CompileTryTarget, MirBuildError, MirEffect, MirFieldTarget,
    MirImmediate, MirOperand, MirPlace, MirSourceOrigin, MirStatement, MirStatementKind,
    MirTerminator, MirTerminatorKind, MirTryContinue,
};

use super::core::{FunctionBuilder, value_type};

impl FunctionBuilder<'_> {
    /// Lower `value?` as ordinary variant control flow.
    ///
    /// The operand is evaluated once. Continue variants project their payload
    /// into one mutable join local, break variants return the original enum,
    /// and every other runtime value reaches an explicit type-mismatch trap.
    pub(super) fn lower_try_expression(
        &mut self,
        expression: HirExprId,
        operand: Option<HirExprId>,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let operand =
            operand.ok_or_else(|| self.inconsistent(origin, "try expression has no operand"))?;
        let target = self
            .input
            .targets()
            .try_target(expression)
            .copied()
            .ok_or_else(|| self.inconsistent(origin, "try expression has no compile target"))?;
        let result_type = self
            .input
            .analysis()
            .expression(expression)
            .map(|fact| value_type(Some(fact)))
            .ok_or_else(|| self.inconsistent(origin, "try expression has no type fact"))?;
        let value = self.lower_expression(operand)?;
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(MirImmediate::Unit));
        }
        let result = self.function.add_synthetic_local(result_type, origin);
        let continuation = self.function.add_block();
        let propagate = self.function.add_block();
        let invalid = self.function.add_block();

        let continuations = match target {
            CompileTryTarget::Expected(layout) => {
                let success = self.function.add_block();
                vec![MirTryContinue {
                    layout,
                    block: success,
                }]
            }
            CompileTryTarget::Dynamic { option, result } => {
                let option_success = self.function.add_block();
                let result_success = self.function.add_block();
                vec![
                    MirTryContinue {
                        layout: option,
                        block: option_success,
                    },
                    MirTryContinue {
                        layout: result,
                        block: result_success,
                    },
                ]
            }
        };
        self.function.set_terminator(
            self.current_block,
            MirTerminator::new(
                origin,
                MirTerminatorKind::TrySwitch {
                    value: value.clone(),
                    target,
                    result,
                    continuations: continuations.clone(),
                    propagate,
                    invalid,
                    join: continuation,
                },
                MirEffect::PURE,
                None,
            ),
        )?;

        for next in continuations {
            self.current_block = next.block;
            self.lower_try_continue(value.clone(), result, next.layout, continuation, origin)?;
        }

        self.current_block = propagate;
        self.finish_open_block(Some(value), origin)?;

        self.current_block = invalid;
        self.function.set_terminator(
            invalid,
            MirTerminator::new(
                origin,
                MirTerminatorKind::TryTypeMismatch { target },
                MirEffect::may_trap(),
                None,
            ),
        )?;

        self.current_block = continuation;
        Ok(MirOperand::Local(result))
    }

    fn lower_try_continue(
        &mut self,
        value: MirOperand,
        result: crate::MirLocalId,
        layout: CompileTryLayoutTarget,
        continuation: crate::MirBlockId,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::local(result)),
                MirStatementKind::ReadField {
                    receiver: value,
                    target: MirFieldTarget::VariantSlot {
                        type_id: layout.type_id,
                        variant: layout.continue_variant,
                        field: layout.continue_payload,
                    },
                },
                MirEffect::may_trap(),
                None,
            ),
        )?;
        self.function.set_terminator(
            self.current_block,
            MirTerminator::new(
                origin,
                MirTerminatorKind::Jump(continuation),
                MirEffect::PURE,
                None,
            ),
        )
    }
}
