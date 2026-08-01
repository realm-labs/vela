use vela_analysis::semantic_facts::OperatorTargetFact;
use vela_analysis::type_fact::TypeFact;
use vela_analysis::validation::{LoopControlKind, LoopControlPlacement};
use vela_common::{NumericTag, PrimitiveTag, ScalarValue};
use vela_hir::body::{HirBinaryOp, HirExprKind};
use vela_hir::ids::{HirBlockId, HirExprId, HirPatternId, HirStmtId};

use crate::{
    MirBinaryOp, MirBuildError, MirEffect, MirImmediate, MirIteratorOperation, MirLocalId,
    MirNumericBinaryOp, MirOperand, MirPlace, MirRangeStepMode, MirRvalue, MirSafepoint,
    MirSourceOrigin, MirStatement, MirStatementKind, MirTerminator, MirTerminatorKind,
    MirValueType,
};

use super::core::{FunctionBuilder, value_type};

/// The two explicit destinations visible to loop-control statements.
///
/// Contexts are stacked by the builder, so a nested loop always resolves to
/// the innermost active pair. No source jump patching is retained in MIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LoopContext {
    continue_block: crate::MirBlockId,
    break_block: crate::MirBlockId,
}

impl LoopContext {
    const fn new(continue_block: crate::MirBlockId, break_block: crate::MirBlockId) -> Self {
        Self {
            continue_block,
            break_block,
        }
    }

    const fn target(self, kind: LoopControlKind) -> crate::MirBlockId {
        match kind {
            LoopControlKind::Break => self.break_block,
            LoopControlKind::Continue => self.continue_block,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectRange {
    start: HirExprId,
    end: HirExprId,
    inclusive: bool,
    mode: MirRangeStepMode,
    cursor_type: MirValueType,
}

enum LoweredLoopIterable {
    Iterator {
        iterator: MirOperand,
    },
    Range {
        cursor: MirLocalId,
        end: MirOperand,
        exhausted: MirLocalId,
        inclusive: bool,
        mode: MirRangeStepMode,
    },
}

impl FunctionBuilder<'_> {
    pub(super) fn lower_for(
        &mut self,
        statement: HirStmtId,
        patterns: &[HirPatternId],
        iterable: Option<HirExprId>,
        body: Option<HirBlockId>,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.validate_for_flow(statement, origin)?;
        let iterable =
            iterable.ok_or_else(|| self.inconsistent(origin, "for statement has no iterable"))?;
        let body = body.ok_or_else(|| self.inconsistent(origin, "for statement has no body"))?;
        if self.input.analysis().block_control_flow(body).is_none() {
            return Err(self.inconsistent(origin, "for body has no analysis control-flow fact"));
        }
        let ([value_pattern] | [_, value_pattern]) = patterns else {
            return Err(self.inconsistent(
                origin,
                format!(
                    "for statement has {} patterns; MIR requires one value pattern or index/value patterns",
                    patterns.len()
                ),
            ));
        };
        let iterable_origin = self.loop_expression_origin(iterable)?;
        let direct_range = self.direct_range(iterable, iterable_origin)?;
        let lowered = match direct_range {
            Some(range) => match self.lower_direct_range(range, iterable_origin)? {
                Some(range) => range,
                None => return Ok(()),
            },
            None => match self.lower_iterator(iterable, iterable_origin)? {
                Some(iterator) => iterator,
                None => return Ok(()),
            },
        };

        // Only allocate CFG blocks after iterable evaluation has completed.
        // A diverging iterable or bound expression therefore leaves no dead,
        // unterminated loop skeleton behind.
        let header = self.function.add_block();
        let body_block = self.function.add_block();
        let done = self.function.add_block();
        let index_counter = if patterns.len() == 2 {
            let counter = self
                .function
                .add_synthetic_local(MirValueType::Primitive(PrimitiveTag::I64), origin);
            self.function.append_statement(
                self.current_block,
                MirStatement::assign(
                    origin,
                    MirPlace::local(counter),
                    MirRvalue::Use(MirOperand::Immediate(MirImmediate::Scalar(
                        ScalarValue::I64(0),
                    ))),
                ),
            )?;
            Some(counter)
        } else {
            None
        };
        self.jump_to_loop_block(header, origin)?;

        let item_type = match direct_range {
            Some(_) => MirValueType::Primitive(PrimitiveTag::I64),
            None => self.loop_pattern_value_type(*value_pattern, origin)?,
        };
        let item = self
            .function
            .add_synthetic_local(item_type, iterable_origin);
        self.current_block = header;
        match lowered {
            LoweredLoopIterable::Iterator { iterator } => {
                let safepoint = self
                    .function
                    .add_safepoint(MirSafepoint::new(iterable_origin));
                self.function.set_terminator(
                    header,
                    MirTerminator::new(
                        iterable_origin,
                        MirTerminatorKind::IteratorNext {
                            iterator,
                            item,
                            next: body_block,
                            done,
                        },
                        MirEffect::dynamic_call(),
                        Some(safepoint),
                    ),
                )?;
            }
            LoweredLoopIterable::Range {
                cursor,
                end,
                exhausted,
                inclusive,
                mode,
            } => {
                let effect = match mode {
                    MirRangeStepMode::I64Proven => MirEffect::PURE,
                    MirRangeStepMode::DynamicInteger => MirEffect::may_trap(),
                };
                self.function.set_terminator(
                    header,
                    MirTerminator::new(
                        iterable_origin,
                        MirTerminatorKind::RangeNext {
                            cursor,
                            end,
                            exhausted,
                            inclusive,
                            item,
                            mode,
                            next: body_block,
                            done,
                        },
                        effect,
                        None,
                    ),
                )?;
            }
        }

        self.current_block = body_block;
        if let ([index_pattern, _], Some(counter)) = (patterns, index_counter) {
            // Snapshot the source index, then advance the counter before any
            // refutable index/value test. A mismatch therefore consumes one
            // source item and the next iteration observes the next index.
            let index_origin = self.pattern_origin(*index_pattern)?;
            let index_value = self.capture_operand(MirOperand::Local(counter), index_origin)?;
            self.function.append_statement(
                self.current_block,
                MirStatement::new(
                    origin,
                    Some(MirPlace::local(counter)),
                    MirStatementKind::Binary {
                        operation: MirBinaryOp::Numeric {
                            operation: MirNumericBinaryOp::Add,
                            kind: NumericTag::I64,
                        },
                        left: MirOperand::Local(counter),
                        right: MirOperand::Immediate(MirImmediate::Scalar(ScalarValue::I64(1))),
                    },
                    MirEffect::may_trap(),
                    None,
                ),
            )?;
            self.lower_loop_pattern(*index_pattern, index_value, header)?;
        }
        self.lower_loop_pattern(*value_pattern, MirOperand::Local(item), header)?;

        self.loop_stack.push(LoopContext::new(header, done));
        self.lower_block(body)?;
        self.loop_stack.pop().ok_or_else(|| {
            self.inconsistent(
                origin,
                "for-loop context stack was lost while lowering its body",
            )
        })?;
        if !self.current_is_terminated()? {
            self.jump_to_loop_block(header, origin)?;
        }
        self.current_block = done;
        Ok(())
    }

    pub(super) fn lower_break(
        &mut self,
        statement: HirStmtId,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.lower_loop_control(statement, LoopControlKind::Break, origin)
    }

    pub(super) fn lower_continue(
        &mut self,
        statement: HirStmtId,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.lower_loop_control(statement, LoopControlKind::Continue, origin)
    }

    fn lower_loop_control(
        &mut self,
        statement: HirStmtId,
        expected: LoopControlKind,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let fact = self
            .input
            .analysis()
            .loop_control(statement)
            .ok_or_else(|| {
                self.inconsistent(origin, "loop-control statement has no placement fact")
            })?;
        if fact.kind != expected {
            return Err(self.inconsistent(
                origin,
                format!(
                    "loop-control placement kind {:?} disagrees with HIR {:?}",
                    fact.kind, expected
                ),
            ));
        }
        if fact.placement != LoopControlPlacement::InsideLoop {
            return Err(self.inconsistent(
                origin,
                format!(
                    "validated loop-control statement reached MIR with {:?} placement",
                    fact.placement
                ),
            ));
        }
        self.validate_loop_control_flow(statement, expected, origin)?;
        let context = self.loop_stack.last().copied().ok_or_else(|| {
            self.inconsistent(origin, "loop-control fact has no active MIR loop context")
        })?;
        self.function.set_terminator(
            self.current_block,
            MirTerminator::new(
                origin,
                MirTerminatorKind::Jump(context.target(expected)),
                MirEffect::PURE,
                None,
            ),
        )
    }

    fn lower_iterator(
        &mut self,
        iterable: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<Option<LoweredLoopIterable>, MirBuildError> {
        let host_collection = match self.input.analysis().expression(iterable) {
            Some(
                TypeFact::Array { .. } | TypeFact::ArrayView { .. } | TypeFact::ArrayMut { .. },
            ) => Some(vela_common::HostCollectionIteration::ArrayValues),
            Some(TypeFact::Map { .. } | TypeFact::MapView { .. } | TypeFact::MapMut { .. }) => {
                Some(vela_common::HostCollectionIteration::MapEntries)
            }
            Some(TypeFact::Set { .. } | TypeFact::SetView { .. } | TypeFact::SetMut { .. }) => {
                Some(vela_common::HostCollectionIteration::SetValues)
            }
            _ => None,
        };
        let iterable = self.lower_expression(iterable)?;
        if self.current_is_terminated()? {
            return Ok(None);
        }
        let iterator = self.function.add_temp(MirValueType::Iterator, origin);
        let safepoint = self.function.add_safepoint(MirSafepoint::new(origin));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(iterator)),
                MirStatementKind::Iterator(MirIteratorOperation::Create {
                    iterable,
                    host_collection,
                }),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )?;
        Ok(Some(LoweredLoopIterable::Iterator {
            iterator: MirOperand::Temp(iterator),
        }))
    }

    fn lower_direct_range(
        &mut self,
        range: DirectRange,
        origin: MirSourceOrigin,
    ) -> Result<Option<LoweredLoopIterable>, MirBuildError> {
        let start_origin = self.loop_expression_origin(range.start)?;
        let start = self.lower_expression(range.start)?;
        if self.current_is_terminated()? {
            return Ok(None);
        }
        let start = self.capture_operand(start, start_origin)?;
        let end_origin = self.loop_expression_origin(range.end)?;
        let end = self.lower_expression(range.end)?;
        if self.current_is_terminated()? {
            return Ok(None);
        }
        // The loop body may mutate a source local used as the range end. Keep
        // the evaluated bound stable for every subsequent step.
        let end = self.capture_operand(end, end_origin)?;
        let cursor = self.function.add_synthetic_local(range.cursor_type, origin);
        let exhausted = self
            .function
            .add_synthetic_local(MirValueType::Primitive(PrimitiveTag::Bool), origin);
        self.function.append_statement(
            self.current_block,
            MirStatement::assign(origin, MirPlace::local(cursor), MirRvalue::Use(start)),
        )?;
        self.function.append_statement(
            self.current_block,
            MirStatement::assign(
                origin,
                MirPlace::local(exhausted),
                MirRvalue::Use(MirOperand::Immediate(MirImmediate::Bool(false))),
            ),
        )?;
        Ok(Some(LoweredLoopIterable::Range {
            cursor,
            end,
            exhausted,
            inclusive: range.inclusive,
            mode: range.mode,
        }))
    }

    fn direct_range(
        &self,
        iterable: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<Option<DirectRange>, MirBuildError> {
        let expression = self.body.expression(iterable).ok_or_else(|| {
            self.inconsistent(
                origin,
                format!("missing HIR iterable expression {iterable:?}"),
            )
        })?;
        let HirExprKind::Binary {
            op: Some(operation @ (HirBinaryOp::Range | HirBinaryOp::RangeInclusive)),
            lhs: Some(start),
            rhs: Some(end),
        } = &expression.kind
        else {
            return Ok(None);
        };
        let analysis = self.input.analysis();
        let target = analysis
            .operator_target(iterable)
            .ok_or_else(|| self.inconsistent(origin, "range iterable has no operator target"))?;
        let target_proven = match target {
            OperatorTargetFact::Binary(target) if target == *operation => true,
            OperatorTargetFact::Dynamic => false,
            OperatorTargetFact::Binary(_) => {
                return Err(
                    self.inconsistent(origin, "range iterable operator target disagrees with HIR")
                );
            }
            OperatorTargetFact::Unresolved => {
                return Err(self.inconsistent(origin, "unresolved range iterable reached MIR"));
            }
            OperatorTargetFact::Unary(_) | OperatorTargetFact::Assignment(_) => {
                return Err(self.inconsistent(
                    origin,
                    "range iterable has the wrong analysis operator family",
                ));
            }
        };
        let start_fact = analysis
            .expression(*start)
            .ok_or_else(|| self.inconsistent(origin, "range start has no analysis type fact"))?;
        let end_fact = analysis
            .expression(*end)
            .ok_or_else(|| self.inconsistent(origin, "range end has no analysis type fact"))?;
        let i64_proven = target_proven
            && matches!(start_fact, TypeFact::Primitive(PrimitiveTag::I64))
            && matches!(end_fact, TypeFact::Primitive(PrimitiveTag::I64));
        Ok(Some(DirectRange {
            start: *start,
            end: *end,
            inclusive: *operation == HirBinaryOp::RangeInclusive,
            mode: if i64_proven {
                MirRangeStepMode::I64Proven
            } else {
                MirRangeStepMode::DynamicInteger
            },
            cursor_type: if i64_proven {
                MirValueType::Primitive(PrimitiveTag::I64)
            } else {
                // Dynamic stepping accepts any runtime integer width and
                // normalizes the advanced cursor to i64.
                MirValueType::Dynamic
            },
        }))
    }

    fn validate_for_flow(
        &self,
        statement: HirStmtId,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let analysis = self.input.analysis();
        let flow = analysis
            .statement_control_flow(statement)
            .ok_or_else(|| self.inconsistent(origin, "for statement has no control-flow fact"))?;
        if !flow.can_fallthrough || flow.may_break || flow.may_continue {
            return Err(self.inconsistent(
                origin,
                "for statement control-flow fact did not consume loop-local exits",
            ));
        }
        Ok(())
    }

    fn validate_loop_control_flow(
        &self,
        statement: HirStmtId,
        expected: LoopControlKind,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let analysis = self.input.analysis();
        let flow = analysis.statement_control_flow(statement).ok_or_else(|| {
            self.inconsistent(origin, "loop-control statement has no control-flow fact")
        })?;
        let valid = match expected {
            LoopControlKind::Break => {
                !flow.can_fallthrough && flow.may_break && !flow.may_continue && !flow.may_return
            }
            LoopControlKind::Continue => {
                !flow.can_fallthrough && !flow.may_break && flow.may_continue && !flow.may_return
            }
        };
        if !valid {
            return Err(self.inconsistent(
                origin,
                format!("analysis control-flow fact disagrees with {expected:?}"),
            ));
        }
        Ok(())
    }

    fn loop_pattern_value_type(
        &self,
        pattern: HirPatternId,
        origin: MirSourceOrigin,
    ) -> Result<MirValueType, MirBuildError> {
        self.input
            .analysis()
            .pattern(pattern)
            .map(|fact| value_type(Some(fact)))
            .ok_or_else(|| self.inconsistent(origin, "for-loop pattern has no analysis type fact"))
    }

    fn jump_to_loop_block(
        &mut self,
        target: crate::MirBlockId,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(());
        }
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

    fn loop_expression_origin(
        &self,
        expression: HirExprId,
    ) -> Result<MirSourceOrigin, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                MirSourceOrigin::body(self.body.id, self.body.origin.span),
                format!("missing HIR loop expression {expression:?}"),
            )
        })?;
        Ok(MirSourceOrigin::expression(
            self.body.id,
            expression,
            record.origin.span,
        ))
    }
}
