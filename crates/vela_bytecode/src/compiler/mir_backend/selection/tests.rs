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
fn selection_covers_every_function_and_reports_selected_recipes() {
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
        "fixture should report selected short recipes"
    );
}

#[test]
fn coverage_verifier_rejects_corrupted_superinstruction_recipe() {
    let (owner, mut plan) = plan();
    let handoff = owner.backend_handoff().expect("complete analyses");
    let (function_id, block, mut selected) = handoff
        .program()
        .functions()
        .find_map(|(function_id, function)| {
            let analyses = handoff.analyses(function_id)?;
            function.blocks().find_map(|(block, _)| {
                measured_candidate(function, analyses, block)
                    .map(|selected| (function_id, block, selected))
            })
        })
        .expect("fixture has one measured superinstruction recipe");
    selected.immediate = selected.immediate.wrapping_add(1);
    let function = plan.functions.get_mut(&function_id).expect("function plan");
    let unit = function.block_entries[&block];
    function.units[unit.0 as usize] = SelectedUnit::Superinstruction(selected);
    assert!(matches!(
        verify(handoff, &plan),
        Err(SelectionError::InvalidSuperinstruction { .. })
    ));
}

#[test]
fn coverage_verifier_rejects_corrupted_scalar_block_recipe() {
    let compiled = compile_test_program(
            SourceId::new(902),
            "fn main() -> i64 { let total = 0; for outer in 0..8 { for value in 0..128 { total += value + outer - outer; } } return total; }",
        )
        .expect("scalar selection fixture should compile");
    let (_, owner) = compiled
        .verified_mir()
        .roots()
        .next()
        .expect("fixture has one root");
    let owner = Arc::clone(owner);
    let handoff = owner.backend_handoff().expect("complete analyses");
    let mut plan = select(handoff).expect("scalar selection should succeed");
    let selected = plan
        .functions
        .values_mut()
        .flat_map(|function| function.units.iter_mut())
        .find_map(|unit| match unit {
            SelectedUnit::ScalarBlock(selected) => Some(selected),
            SelectedUnit::Ordinary(_) | SelectedUnit::Superinstruction(_) => None,
        })
        .expect("fixture has one selected scalar block");
    selected.statements = selected.statements[1..].into();
    assert!(matches!(
        verify(handoff, &plan),
        Err(SelectionError::InvalidScalarBlock { .. })
    ));

    let mut range_plan = select(handoff).expect("scalar selection should succeed");
    let (latch, range) = range_plan
        .functions
        .values_mut()
        .flat_map(|function| function.units.iter_mut())
        .find_map(|unit| match unit {
            SelectedUnit::ScalarBlock(selected) => {
                let latch = selected.block;
                selected.range_loop.as_mut().map(|range| (latch, range))
            }
            SelectedUnit::Ordinary(_) | SelectedUnit::Superinstruction(_) => None,
        })
        .expect("fixture has one selected scalar range loop");
    range.header = latch;
    assert!(matches!(
        verify(handoff, &range_plan),
        Err(SelectionError::InvalidScalarBlock { .. })
    ));
}

#[test]
fn coverage_verifier_rejects_missing_and_duplicate_blocks() {
    let (owner, original) = plan();
    let handoff = owner.backend_handoff().expect("complete analyses");
    let function = *original.functions.keys().next().expect("one function");

    let mut missing = original.clone();
    let function_plan = missing.functions.get_mut(&function).expect("function plan");
    let removed_unit = function_plan.units.last().expect("one unit").clone();
    let removed_block = removed_unit.block();
    function_plan.units = function_plan.units[..function_plan.units.len() - 1].into();
    function_plan.coverage = function_plan.coverage[..function_plan.coverage.len() - 1].into();
    function_plan.block_entries.remove(&removed_block);
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
    let coverage = first_coverage_with(&mut source_mismatch, |value| value.source_points.len() > 1);
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
