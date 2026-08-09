//! Independent verification of selected physical coverage against sealed MIR.
//!
//! This deliberately does not call the selector's coverage-construction
//! helper. A selector bug therefore cannot validate itself by reproducing the
//! same incomplete manifest in the verifier.

use std::collections::BTreeSet;

use vela_mir::{
    MirBackendHandoff, MirBinaryOp, MirBlockId, MirBudgetSite, MirComparisonOp, MirFunction,
    MirFunctionAnalyses, MirFunctionId, MirImmediate, MirLiveValue, MirOperand, MirPlace,
    MirStatementKind, MirTerminatorKind,
};

use super::{
    SelectedBudgetCoverage, SelectedCoverage, SelectedExitCoverage, SelectedFunctionPlan,
    SelectedProgramPlan, SelectedSafepointCoverage, SelectedSourcePoint, SelectedSourcePointKind,
    SelectedStatementLiveness, SelectedUnit, SelectedUnitId, SelectionError, SuperinstructionPlan,
    mir_successors,
};

pub(super) fn verify(
    handoff: MirBackendHandoff<'_>,
    plan: &SelectedProgramPlan,
) -> Result<(), SelectionError> {
    for function_id in plan.functions.keys().copied() {
        if handoff.program().function(function_id).is_none() {
            return Err(SelectionError::UnexpectedFunctionPlan(function_id));
        }
    }
    for (function_id, function) in handoff.program().functions() {
        let analyses = handoff
            .analyses(function_id)
            .ok_or(SelectionError::MissingAnalysis(function_id))?;
        let function_plan = plan
            .functions
            .get(&function_id)
            .ok_or(SelectionError::MissingFunctionPlan(function_id))?;
        verify_function(function_id, function, analyses, function_plan)?;
    }
    Ok(())
}

fn verify_function(
    function_id: MirFunctionId,
    function: &MirFunction,
    analyses: &MirFunctionAnalyses,
    plan: &SelectedFunctionPlan,
) -> Result<(), SelectionError> {
    if plan.function != function_id {
        return Err(SelectionError::FunctionMismatch {
            expected: function_id,
            actual: plan.function,
        });
    }
    if plan.units.len() != plan.coverage.len() {
        return Err(SelectionError::UnitCoverageCountMismatch(function_id));
    }

    let mut covered_blocks = BTreeSet::new();
    for (index, (unit, coverage)) in plan.units.iter().zip(&plan.coverage).enumerate() {
        let block = unit.block();
        if coverage.function != function_id || coverage.block != block {
            return Err(SelectionError::UnitBlockMismatch {
                function: function_id,
                unit: block,
                coverage: coverage.block,
            });
        }
        if !covered_blocks.insert(block) {
            return Err(SelectionError::DuplicateBlock {
                function: function_id,
                block,
            });
        }
        verify_block(function_id, function, analyses, block, coverage)?;
        if let SelectedUnit::Superinstruction(selected) = unit {
            verify_superinstruction(function_id, function, analyses, selected)?;
        }
        verify_block_entry(function_id, plan, block, index)?;
    }

    for (block, _) in function.blocks() {
        if !covered_blocks.contains(&block) {
            return Err(SelectionError::MissingBlock {
                function: function_id,
                block,
            });
        }
    }
    if plan.block_entries.len() != covered_blocks.len() {
        let block = plan
            .block_entries
            .keys()
            .find(|block| !covered_blocks.contains(block))
            .copied()
            .unwrap_or(function.entry_block());
        return Err(SelectionError::InvalidBlockEntry {
            function: function_id,
            block,
        });
    }
    Ok(())
}

fn verify_block_entry(
    function: MirFunctionId,
    plan: &SelectedFunctionPlan,
    block: MirBlockId,
    index: usize,
) -> Result<(), SelectionError> {
    let expected = SelectedUnitId(
        u32::try_from(index).map_err(|_| SelectionError::UnitCountOverflow(function))?,
    );
    match plan.block_entries.get(&block) {
        None => Err(SelectionError::MissingBlockEntry { function, block }),
        Some(actual) if *actual != expected => {
            Err(SelectionError::InvalidBlockEntry { function, block })
        }
        Some(_) => Ok(()),
    }
}

fn verify_superinstruction(
    function_id: MirFunctionId,
    function: &MirFunction,
    analyses: &MirFunctionAnalyses,
    selected: &SuperinstructionPlan,
) -> Result<(), SelectionError> {
    let invalid = || SelectionError::InvalidSuperinstruction {
        function: function_id,
        block: selected.block,
    };
    let block = function.block(selected.block).ok_or_else(invalid)?;
    let statement_id = block.statements().last().copied().ok_or_else(invalid)?;
    if statement_id != selected.statement {
        return Err(invalid());
    }
    let statement = function.statement(statement_id).ok_or_else(invalid)?;
    let MirStatementKind::Binary {
        operation:
            MirBinaryOp::Compare {
                operation,
                kind: vela_common::PrimitiveTag::I64,
            },
        left,
        right,
    } = &statement.kind
    else {
        return Err(invalid());
    };
    let MirPlace::Temp(destination) = statement.destination.ok_or_else(invalid)? else {
        return Err(invalid());
    };
    let terminator = block.terminator().ok_or_else(invalid)?;
    let MirTerminatorKind::Branch {
        condition: MirOperand::Temp(condition),
        then_block,
        else_block,
    } = &terminator.kind
    else {
        return Err(invalid());
    };
    if destination != *condition
        || statement.safepoint.is_some()
        || terminator.safepoint.is_some()
        || analyses
            .value_liveness
            .block_live_out
            .get(&selected.block)
            .is_none_or(|live| live.contains(&MirLiveValue::Temp(destination)))
    {
        return Err(invalid());
    }

    let left_immediate = verified_i64_immediate(analyses, statement_id, left);
    let right_immediate = verified_i64_immediate(analyses, statement_id, right);
    let expected = match (left_immediate, right_immediate) {
        (None, Some(immediate)) if !matches!(left, MirOperand::Immediate(_)) => {
            (left, immediate, verified_compare(*operation))
        }
        (Some(immediate), None) if !matches!(right, MirOperand::Immediate(_)) => (
            right,
            immediate,
            verified_compare(verified_reverse_compare(*operation)),
        ),
        _ => return Err(invalid()),
    };
    if selected.value != *expected.0
        || selected.immediate != expected.1
        || selected.op != expected.2
        || selected.then_block != *then_block
        || selected.else_block != *else_block
    {
        return Err(invalid());
    }
    Ok(())
}

fn verified_i64_immediate(
    analyses: &MirFunctionAnalyses,
    statement: vela_mir::MirStatementId,
    operand: &MirOperand,
) -> Option<i64> {
    match analyses
        .facts
        .operand_before(statement, operand)?
        .immediate?
    {
        MirImmediate::Scalar(vela_common::ScalarValue::I64(value)) => Some(value),
        _ => None,
    }
}

const fn verified_reverse_compare(operation: MirComparisonOp) -> MirComparisonOp {
    match operation {
        MirComparisonOp::Equal => MirComparisonOp::Equal,
        MirComparisonOp::NotEqual => MirComparisonOp::NotEqual,
        MirComparisonOp::Less => MirComparisonOp::Greater,
        MirComparisonOp::LessEqual => MirComparisonOp::GreaterEqual,
        MirComparisonOp::Greater => MirComparisonOp::Less,
        MirComparisonOp::GreaterEqual => MirComparisonOp::LessEqual,
    }
}

const fn verified_compare(operation: MirComparisonOp) -> crate::I64CompareOp {
    match operation {
        MirComparisonOp::Equal => crate::I64CompareOp::Equal,
        MirComparisonOp::NotEqual => crate::I64CompareOp::NotEqual,
        MirComparisonOp::Less => crate::I64CompareOp::Less,
        MirComparisonOp::LessEqual => crate::I64CompareOp::LessEqual,
        MirComparisonOp::Greater => crate::I64CompareOp::Greater,
        MirComparisonOp::GreaterEqual => crate::I64CompareOp::GreaterEqual,
    }
}

fn verify_block(
    function_id: MirFunctionId,
    function: &MirFunction,
    analyses: &MirFunctionAnalyses,
    block_id: MirBlockId,
    coverage: &SelectedCoverage,
) -> Result<(), SelectionError> {
    let block = function
        .block(block_id)
        .ok_or(SelectionError::MissingBlock {
            function: function_id,
            block: block_id,
        })?;
    let terminator = block
        .terminator()
        .ok_or(SelectionError::TerminatorCoverageMismatch {
            function: function_id,
            block: block_id,
        })?;

    if coverage.statements.as_ref() != block.statements() {
        return Err(SelectionError::StatementCoverageMismatch {
            function: function_id,
            block: block_id,
        });
    }
    if coverage.terminator != block_id {
        return Err(SelectionError::TerminatorCoverageMismatch {
            function: function_id,
            block: block_id,
        });
    }

    let mut budget = Vec::new();
    let mut safepoints = Vec::new();
    let mut sources = Vec::new();
    let mut statement_liveness = Vec::new();
    for statement_id in block.statements().iter().copied() {
        let statement =
            function
                .statement(statement_id)
                .ok_or(SelectionError::StatementCoverageMismatch {
                    function: function_id,
                    block: block_id,
                })?;
        if let Some(point) = analyses.budget.statement_before(statement_id) {
            budget.push(SelectedBudgetCoverage {
                site: MirBudgetSite::StatementBefore(statement_id),
                point,
            });
        }
        if let Some(safepoint) = statement.safepoint {
            safepoints.push(verified_safepoint(
                function_id,
                block_id,
                analyses,
                safepoint,
            )?);
        }
        sources.push(SelectedSourcePoint {
            kind: SelectedSourcePointKind::Statement(statement_id),
            origin: statement.origin,
        });
        statement_liveness.push(SelectedStatementLiveness {
            statement: statement_id,
            live_before: required_set(
                function_id,
                block_id,
                analyses
                    .value_liveness
                    .statement_live_before
                    .get(&statement_id),
                "statement live-before",
            )?,
            live_after: required_set(
                function_id,
                block_id,
                analyses
                    .value_liveness
                    .statement_live_after
                    .get(&statement_id),
                "statement live-after",
            )?,
            debug_before: analyses
                .debug_availability
                .statement_before
                .get(&statement_id)
                .cloned()
                .unwrap_or_default(),
        });
    }
    if let Some(point) = analyses.budget.terminator_before(block_id) {
        budget.push(SelectedBudgetCoverage {
            site: MirBudgetSite::TerminatorBefore(block_id),
            point,
        });
    }
    if let Some(safepoint) = terminator.safepoint {
        safepoints.push(verified_safepoint(
            function_id,
            block_id,
            analyses,
            safepoint,
        )?);
    }
    sources.push(SelectedSourcePoint {
        kind: SelectedSourcePointKind::Terminator(block_id),
        origin: terminator.origin,
    });

    let exits = mir_successors(&terminator.kind)
        .into_iter()
        .map(|target| {
            let edge_budget = analyses.budget.edge(block_id, target);
            if let Some(point) = edge_budget {
                budget.push(SelectedBudgetCoverage {
                    site: MirBudgetSite::Edge {
                        from: block_id,
                        to: target,
                    },
                    point,
                });
            }
            SelectedExitCoverage {
                target,
                budget: edge_budget,
            }
        })
        .collect::<Vec<_>>();

    if coverage.budget.as_ref() != budget {
        return Err(SelectionError::BudgetCoverageMismatch {
            function: function_id,
            block: block_id,
        });
    }
    if coverage.safepoints.as_ref() != safepoints {
        return Err(SelectionError::SafepointCoverageMismatch {
            function: function_id,
            block: block_id,
        });
    }
    if coverage.exits.as_ref() != exits {
        return Err(SelectionError::ExitCoverageMismatch {
            function: function_id,
            block: block_id,
        });
    }
    if coverage.source_points.as_ref() != sources {
        return Err(SelectionError::SourceCoverageMismatch {
            function: function_id,
            block: block_id,
        });
    }

    let live_in = required_set(
        function_id,
        block_id,
        analyses.value_liveness.block_live_in.get(&block_id),
        "block live-in",
    )?;
    let live_out = required_set(
        function_id,
        block_id,
        analyses.value_liveness.block_live_out.get(&block_id),
        "block live-out",
    )?;
    let debug_at_entry = analyses
        .debug_availability
        .block_entry
        .get(&block_id)
        .cloned()
        .unwrap_or_default();
    if coverage.live_in != live_in
        || coverage.live_out != live_out
        || coverage.debug_at_entry != debug_at_entry
        || coverage.statement_liveness.as_ref() != statement_liveness
    {
        return Err(SelectionError::LivenessCoverageMismatch {
            function: function_id,
            block: block_id,
        });
    }
    Ok(())
}

fn verified_safepoint(
    function: MirFunctionId,
    block: MirBlockId,
    analyses: &MirFunctionAnalyses,
    safepoint: vela_mir::MirSafepointId,
) -> Result<SelectedSafepointCoverage, SelectionError> {
    let live_roots = analyses
        .root_liveness
        .live_before_safepoint
        .get(&safepoint)
        .cloned()
        .ok_or(SelectionError::MissingAnalysisFact {
            function,
            block,
            fact: "safepoint live roots",
        })?;
    Ok(SelectedSafepointCoverage {
        safepoint,
        live_roots,
    })
}

fn required_set<T: Ord + Clone>(
    function: MirFunctionId,
    block: MirBlockId,
    values: Option<&BTreeSet<T>>,
    fact: &'static str,
) -> Result<BTreeSet<T>, SelectionError> {
    values.cloned().ok_or(SelectionError::MissingAnalysisFact {
        function,
        block,
        fact,
    })
}
