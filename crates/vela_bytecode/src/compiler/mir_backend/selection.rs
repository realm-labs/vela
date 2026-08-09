use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use vela_mir::{
    MirBackendHandoff, MirBinaryOp, MirBlockId, MirBudgetPoint, MirBudgetSite, MirDebugLocalId,
    MirFunction, MirFunctionAnalyses, MirFunctionId, MirLiveValue, MirPlace, MirSafepointId,
    MirSourceOrigin, MirStatementId, MirStatementKind, MirTerminatorKind,
};

mod verify;

pub(super) fn verify(
    handoff: MirBackendHandoff<'_>,
    plan: &SelectedProgramPlan,
) -> Result<(), SelectionError> {
    verify::verify(handoff, plan)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SelectedProgramPlan {
    functions: BTreeMap<MirFunctionId, SelectedFunctionPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedFunctionPlan {
    function: MirFunctionId,
    units: Box<[SelectedUnit]>,
    block_entries: BTreeMap<MirBlockId, SelectedUnitId>,
    coverage: Box<[SelectedCoverage]>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SelectedUnitId(u32);

#[derive(Clone, Debug, Eq, PartialEq)]
enum SelectedUnit {
    Ordinary(MirUnitRange),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MirUnitRange {
    block: MirBlockId,
    reason: OrdinaryReason,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum OrdinaryReason {
    CandidateDeferred(SelectionCandidate),
    NoApprovedRecipe,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SelectionCandidate {
    I64CompareImmediateBranch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedCoverage {
    function: MirFunctionId,
    block: MirBlockId,
    statements: Box<[MirStatementId]>,
    terminator: MirBlockId,
    budget: Box<[SelectedBudgetCoverage]>,
    safepoints: Box<[SelectedSafepointCoverage]>,
    exits: Box<[SelectedExitCoverage]>,
    source_points: Box<[SelectedSourcePoint]>,
    live_in: BTreeSet<MirLiveValue>,
    live_out: BTreeSet<MirLiveValue>,
    debug_at_entry: BTreeSet<MirDebugLocalId>,
    statement_liveness: Box<[SelectedStatementLiveness]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedBudgetCoverage {
    site: MirBudgetSite,
    point: MirBudgetPoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedSafepointCoverage {
    safepoint: MirSafepointId,
    live_roots: BTreeSet<MirLiveValue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedExitCoverage {
    target: MirBlockId,
    budget: Option<MirBudgetPoint>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectedSourcePointKind {
    Statement(MirStatementId),
    Terminator(MirBlockId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedSourcePoint {
    kind: SelectedSourcePointKind,
    origin: MirSourceOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedStatementLiveness {
    statement: MirStatementId,
    live_before: BTreeSet<MirLiveValue>,
    live_after: BTreeSet<MirLiveValue>,
    debug_before: BTreeSet<MirDebugLocalId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SelectionError {
    MissingAnalysis(MirFunctionId),
    MissingFunctionPlan(MirFunctionId),
    UnexpectedFunctionPlan(MirFunctionId),
    FunctionMismatch {
        expected: MirFunctionId,
        actual: MirFunctionId,
    },
    UnitCountOverflow(MirFunctionId),
    UnitCoverageCountMismatch(MirFunctionId),
    MissingBlockEntry {
        function: MirFunctionId,
        block: MirBlockId,
    },
    InvalidBlockEntry {
        function: MirFunctionId,
        block: MirBlockId,
    },
    MissingBlock {
        function: MirFunctionId,
        block: MirBlockId,
    },
    DuplicateBlock {
        function: MirFunctionId,
        block: MirBlockId,
    },
    UnitBlockMismatch {
        function: MirFunctionId,
        unit: MirBlockId,
        coverage: MirBlockId,
    },
    StatementCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    TerminatorCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    BudgetCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    SafepointCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    ExitCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    SourceCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    LivenessCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    MissingAnalysisFact {
        function: MirFunctionId,
        block: MirBlockId,
        fact: &'static str,
    },
}

impl SelectionError {
    pub(crate) const fn function(&self) -> Option<MirFunctionId> {
        match *self {
            Self::MissingAnalysis(function)
            | Self::MissingFunctionPlan(function)
            | Self::UnexpectedFunctionPlan(function)
            | Self::UnitCountOverflow(function)
            | Self::UnitCoverageCountMismatch(function) => Some(function),
            Self::FunctionMismatch { expected, .. }
            | Self::MissingBlockEntry {
                function: expected, ..
            }
            | Self::InvalidBlockEntry {
                function: expected, ..
            }
            | Self::MissingBlock {
                function: expected, ..
            }
            | Self::DuplicateBlock {
                function: expected, ..
            }
            | Self::UnitBlockMismatch {
                function: expected, ..
            }
            | Self::StatementCoverageMismatch {
                function: expected, ..
            }
            | Self::TerminatorCoverageMismatch {
                function: expected, ..
            }
            | Self::BudgetCoverageMismatch {
                function: expected, ..
            }
            | Self::SafepointCoverageMismatch {
                function: expected, ..
            }
            | Self::ExitCoverageMismatch {
                function: expected, ..
            }
            | Self::SourceCoverageMismatch {
                function: expected, ..
            }
            | Self::LivenessCoverageMismatch {
                function: expected, ..
            }
            | Self::MissingAnalysisFact {
                function: expected, ..
            } => Some(expected),
        }
    }
}

impl fmt::Display for SelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid verified-MIR selection coverage: {self:?}"
        )
    }
}

pub(super) fn select(
    handoff: MirBackendHandoff<'_>,
) -> Result<SelectedProgramPlan, SelectionError> {
    let mut functions = BTreeMap::new();
    for (function_id, function) in handoff.program().functions() {
        let analyses = handoff
            .analyses(function_id)
            .ok_or(SelectionError::MissingAnalysis(function_id))?;
        let mut units = Vec::new();
        let mut coverage = Vec::new();
        let mut block_entries = BTreeMap::new();
        for (block, _) in function.blocks() {
            let unit = SelectedUnitId(
                u32::try_from(units.len())
                    .map_err(|_| SelectionError::UnitCountOverflow(function_id))?,
            );
            units.push(SelectedUnit::Ordinary(MirUnitRange {
                block,
                reason: measured_candidate(function, analyses, block).map_or(
                    OrdinaryReason::NoApprovedRecipe,
                    OrdinaryReason::CandidateDeferred,
                ),
            }));
            coverage.push(select_coverage(function_id, function, analyses, block)?);
            block_entries.insert(block, unit);
        }
        functions.insert(
            function_id,
            SelectedFunctionPlan {
                function: function_id,
                units: units.into_boxed_slice(),
                block_entries,
                coverage: coverage.into_boxed_slice(),
            },
        );
    }
    Ok(SelectedProgramPlan { functions })
}

fn measured_candidate(
    function: &MirFunction,
    analyses: &MirFunctionAnalyses,
    block: MirBlockId,
) -> Option<SelectionCandidate> {
    let block = function.block(block)?;
    let last_statement_id = block.statements().last().copied()?;
    let last_statement = function.statement(last_statement_id)?;
    let MirStatementKind::Binary {
        operation:
            MirBinaryOp::Compare {
                kind: vela_common::PrimitiveTag::I64,
                ..
            },
        left,
        right,
    } = &last_statement.kind
    else {
        return None;
    };
    let MirPlace::Temp(destination) = last_statement.destination? else {
        return None;
    };
    let MirTerminatorKind::Branch {
        condition: vela_mir::MirOperand::Temp(condition),
        ..
    } = &block.terminator()?.kind
    else {
        return None;
    };
    if destination != *condition {
        return None;
    }
    let has_immediate = analyses
        .facts
        .operand_before(last_statement_id, left)
        .is_some_and(|fact| fact.immediate.is_some())
        || analyses
            .facts
            .operand_before(last_statement_id, right)
            .is_some_and(|fact| fact.immediate.is_some());
    has_immediate.then_some(SelectionCandidate::I64CompareImmediateBranch)
}

fn select_coverage(
    function_id: MirFunctionId,
    function: &MirFunction,
    analyses: &MirFunctionAnalyses,
    block_id: MirBlockId,
) -> Result<SelectedCoverage, SelectionError> {
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
    let mut budget = Vec::new();
    let mut safepoints = Vec::new();
    let mut source_points = Vec::new();
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
            safepoints.push(safepoint_coverage(
                function_id,
                block_id,
                analyses,
                safepoint,
            )?);
        }
        source_points.push(SelectedSourcePoint {
            kind: SelectedSourcePointKind::Statement(statement_id),
            origin: statement.origin,
        });
        statement_liveness.push(SelectedStatementLiveness {
            statement: statement_id,
            live_before: analysis_set(
                function_id,
                block_id,
                analyses
                    .value_liveness
                    .statement_live_before
                    .get(&statement_id),
                "statement live-before",
            )?,
            live_after: analysis_set(
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
        safepoints.push(safepoint_coverage(
            function_id,
            block_id,
            analyses,
            safepoint,
        )?);
    }
    source_points.push(SelectedSourcePoint {
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
    Ok(SelectedCoverage {
        function: function_id,
        block: block_id,
        statements: block.statements().to_vec().into_boxed_slice(),
        terminator: block_id,
        budget: budget.into_boxed_slice(),
        safepoints: safepoints.into_boxed_slice(),
        exits: exits.into_boxed_slice(),
        source_points: source_points.into_boxed_slice(),
        live_in: analysis_set(
            function_id,
            block_id,
            analyses.value_liveness.block_live_in.get(&block_id),
            "block live-in",
        )?,
        live_out: analysis_set(
            function_id,
            block_id,
            analyses.value_liveness.block_live_out.get(&block_id),
            "block live-out",
        )?,
        debug_at_entry: analyses
            .debug_availability
            .block_entry
            .get(&block_id)
            .cloned()
            .unwrap_or_default(),
        statement_liveness: statement_liveness.into_boxed_slice(),
    })
}

fn safepoint_coverage(
    function: MirFunctionId,
    block: MirBlockId,
    analyses: &MirFunctionAnalyses,
    safepoint: MirSafepointId,
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

fn analysis_set<T: Ord + Clone>(
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

pub(super) fn mir_successors(kind: &MirTerminatorKind) -> Vec<MirBlockId> {
    match kind {
        MirTerminatorKind::Jump(target) => vec![*target],
        MirTerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        MirTerminatorKind::Switch {
            cases, otherwise, ..
        } => cases
            .iter()
            .map(|case| case.target)
            .chain(std::iter::once(*otherwise))
            .collect(),
        MirTerminatorKind::GuardBranch { passed, slow, .. } => vec![*passed, *slow],
        MirTerminatorKind::TrySwitch {
            continuations,
            propagate,
            invalid,
            join,
            ..
        } => continuations
            .iter()
            .map(|continuation| continuation.block)
            .chain([*propagate, *invalid, *join])
            .collect(),
        MirTerminatorKind::IteratorNext { next, done, .. }
        | MirTerminatorKind::RangeNext { next, done, .. } => vec![*next, *done],
        MirTerminatorKind::AwaitCall { resume, .. } => vec![*resume],
        MirTerminatorKind::Return(_)
        | MirTerminatorKind::TryTypeMismatch { .. }
        | MirTerminatorKind::Unreachable => Vec::new(),
    }
}

#[cfg(test)]
#[derive(Debug, Eq, PartialEq)]
struct SelectionReport {
    functions: usize,
    ordinary_units: usize,
    candidates: BTreeMap<SelectionCandidate, usize>,
    rejection_reasons: BTreeMap<OrdinaryReason, usize>,
}

#[cfg(test)]
fn selection_report(plan: &SelectedProgramPlan) -> SelectionReport {
    let mut ordinary_units = 0;
    let mut candidates = BTreeMap::new();
    let mut rejection_reasons = BTreeMap::new();
    for unit in plan
        .functions
        .values()
        .flat_map(|function| function.units.iter())
    {
        let SelectedUnit::Ordinary(range) = unit;
        ordinary_units += 1;
        *rejection_reasons.entry(range.reason).or_default() += 1;
        if let OrdinaryReason::CandidateDeferred(candidate) = range.reason {
            *candidates.entry(candidate).or_default() += 1;
        }
    }
    SelectionReport {
        functions: plan.functions.len(),
        ordinary_units,
        candidates,
        rejection_reasons,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use vela_common::SourceId;
    use vela_mir::OwnedVerifiedMirProgram;

    use super::*;
    use crate::compiler::compile_test_program;

    const SOURCE: &str = r#"
fn main(limit: i64) -> i64 {
    let bump = |value| value + 1;
    let total = 0;
    for value in 0..200 {
        if value % 3 == 0 {
            total += bump(value);
            continue;
        }
        if value > 180 {
            break;
        }
        total += (value * 5) % 17;
    }
    return total + limit - limit;
}
"#;

    fn fixture() -> Arc<OwnedVerifiedMirProgram> {
        static OWNER: OnceLock<Arc<OwnedVerifiedMirProgram>> = OnceLock::new();
        Arc::clone(OWNER.get_or_init(|| {
            let compiled = compile_test_program(SourceId::new(900), SOURCE)
                .expect("selection fixture should compile");
            let (_, owner) = compiled
                .verified_mir()
                .roots()
                .next()
                .expect("selection fixture has one root");
            Arc::clone(owner)
        }))
    }

    fn plan() -> (Arc<OwnedVerifiedMirProgram>, SelectedProgramPlan) {
        let owner = fixture();
        let plan = select(
            owner
                .backend_handoff()
                .expect("selection fixture has complete analyses"),
        )
        .expect("ordinary selection should succeed");
        (owner, plan)
    }

    #[test]
    fn ordinary_selection_covers_every_function_and_reports_rejections() {
        let (owner, plan) = plan();
        let handoff = owner
            .backend_handoff()
            .expect("selection fixture has complete analyses");
        verify(handoff, &plan).expect("ordinary coverage should verify");
        assert_eq!(
            select(handoff).expect("repeated selection should succeed"),
            plan,
            "selection must be deterministic for one sealed handoff"
        );

        let report = selection_report(&plan);
        assert_eq!(report.functions, plan.functions.len());
        assert!(report.functions >= 2, "fixture should retain its lambda");
        assert!(report.ordinary_units > 0);
        assert_eq!(
            report.rejection_reasons.values().sum::<usize>(),
            report.ordinary_units
        );
        assert!(
            report
                .candidates
                .get(&SelectionCandidate::I64CompareImmediateBranch)
                .is_some_and(|count| *count > 0),
            "fixture should report the deferred measured branch recipe"
        );
    }

    #[test]
    fn coverage_verifier_rejects_missing_and_duplicate_blocks() {
        let (owner, original) = plan();
        let handoff = owner.backend_handoff().expect("complete analyses");
        let function = *original.functions.keys().next().expect("one function");

        let mut missing = original.clone();
        let function_plan = missing.functions.get_mut(&function).expect("function plan");
        let removed_unit = function_plan.units.last().expect("one unit").clone();
        let SelectedUnit::Ordinary(removed_range) = removed_unit;
        function_plan.units = function_plan.units[..function_plan.units.len() - 1].into();
        function_plan.coverage = function_plan.coverage[..function_plan.coverage.len() - 1].into();
        function_plan.block_entries.remove(&removed_range.block);
        assert!(matches!(
            verify(handoff, &missing),
            Err(SelectionError::MissingBlock { .. })
        ));

        let mut duplicate = original.clone();
        let function_plan = duplicate
            .functions
            .get_mut(&function)
            .expect("function plan");
        let mut units = function_plan.units.to_vec();
        units.push(units[0].clone());
        function_plan.units = units.into_boxed_slice();
        let mut coverage = function_plan.coverage.to_vec();
        coverage.push(coverage[0].clone());
        function_plan.coverage = coverage.into_boxed_slice();
        assert!(matches!(
            verify(handoff, &duplicate),
            Err(SelectionError::DuplicateBlock { .. })
        ));
    }

    #[test]
    fn coverage_verifier_rejects_wrong_function_and_statement_coverage() {
        let (owner, original) = plan();
        let handoff = owner.backend_handoff().expect("complete analyses");
        let functions = original.functions.keys().copied().collect::<Vec<_>>();
        assert!(functions.len() >= 2, "fixture should retain its lambda");

        let mut wrong_function = original.clone();
        wrong_function
            .functions
            .get_mut(&functions[0])
            .expect("function plan")
            .function = functions[1];
        assert!(matches!(
            verify(handoff, &wrong_function),
            Err(SelectionError::FunctionMismatch { .. })
        ));

        let mut missing_statement = original.clone();
        let coverage =
            first_coverage_with(&mut missing_statement, |value| !value.statements.is_empty());
        coverage.statements = coverage.statements[1..].into();
        assert!(matches!(
            verify(handoff, &missing_statement),
            Err(SelectionError::StatementCoverageMismatch { .. })
        ));

        let mut duplicate_statement = original.clone();
        let coverage = first_coverage_with(&mut duplicate_statement, |value| {
            !value.statements.is_empty()
        });
        let mut statements = coverage.statements.to_vec();
        statements.push(statements[0]);
        coverage.statements = statements.into_boxed_slice();
        assert!(matches!(
            verify(handoff, &duplicate_statement),
            Err(SelectionError::StatementCoverageMismatch { .. })
        ));

        let mut wrong_terminator = original.clone();
        let coverage = first_coverage_with(&mut wrong_terminator, |value| {
            value.exits.iter().any(|exit| exit.target != value.block)
        });
        coverage.terminator = coverage
            .exits
            .iter()
            .find(|exit| exit.target != coverage.block)
            .expect("different successor")
            .target;
        assert!(matches!(
            verify(handoff, &wrong_terminator),
            Err(SelectionError::TerminatorCoverageMismatch { .. })
        ));
    }

    #[test]
    fn coverage_verifier_rejects_exit_budget_and_safepoint_corruption() {
        let (owner, original) = plan();
        let handoff = owner.backend_handoff().expect("complete analyses");

        let mut invalid_exit = original.clone();
        let coverage = first_coverage_with(&mut invalid_exit, |value| !value.exits.is_empty());
        coverage.exits[0].target = coverage.block;
        assert!(matches!(
            verify(handoff, &invalid_exit),
            Err(SelectionError::ExitCoverageMismatch { .. })
        ));

        let mut moved_budget = original.clone();
        let coverage = first_coverage_with(&mut moved_budget, |value| value.budget.len() > 1);
        coverage.budget.swap(0, 1);
        assert!(matches!(
            verify(handoff, &moved_budget),
            Err(SelectionError::BudgetCoverageMismatch { .. })
        ));

        let mut swallowed_safepoint = original.clone();
        let coverage = first_coverage_with(&mut swallowed_safepoint, |value| {
            !value.safepoints.is_empty()
        });
        coverage.safepoints = Box::new([]);
        assert!(matches!(
            verify(handoff, &swallowed_safepoint),
            Err(SelectionError::SafepointCoverageMismatch { .. })
        ));
    }

    #[test]
    fn coverage_verifier_rejects_source_liveness_and_entry_corruption() {
        let (owner, original) = plan();
        let handoff = owner.backend_handoff().expect("complete analyses");

        let mut source_mismatch = original.clone();
        let coverage =
            first_coverage_with(&mut source_mismatch, |value| value.source_points.len() > 1);
        coverage.source_points = coverage.source_points[1..].into();
        assert!(matches!(
            verify(handoff, &source_mismatch),
            Err(SelectionError::SourceCoverageMismatch { .. })
        ));

        let mut liveness_mismatch = original.clone();
        let coverage = first_coverage_with(&mut liveness_mismatch, |value| {
            !value.statement_liveness.is_empty()
        });
        coverage.statement_liveness = Box::new([]);
        assert!(matches!(
            verify(handoff, &liveness_mismatch),
            Err(SelectionError::LivenessCoverageMismatch { .. })
        ));

        let mut invalid_entry = original.clone();
        let function_plan = invalid_entry
            .functions
            .values_mut()
            .find(|plan| plan.units.len() > 1)
            .expect("fixture has a multi-block function");
        let block = function_plan
            .block_entries
            .keys()
            .next()
            .copied()
            .expect("one block entry");
        function_plan
            .block_entries
            .insert(block, SelectedUnitId(u32::MAX));
        assert!(matches!(
            verify(handoff, &invalid_entry),
            Err(SelectionError::InvalidBlockEntry { .. })
        ));
    }

    fn first_coverage_with(
        plan: &mut SelectedProgramPlan,
        predicate: impl Fn(&SelectedCoverage) -> bool,
    ) -> &mut SelectedCoverage {
        plan.functions
            .values_mut()
            .flat_map(|function| function.coverage.iter_mut())
            .find(|coverage| predicate(coverage))
            .expect("fixture should contain matching coverage")
    }
}
