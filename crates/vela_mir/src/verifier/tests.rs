use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{PrimitiveTag, ScalarValue, SourceId, Span};
use vela_def::{FunctionId, GlobalId, TypeId};
use vela_hir::ids::{HirBodyId, HirDeclId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use crate::{
    CompileFunctionAccess, CompileFunctionClass, CompileFunctionDescriptor,
    CompileFunctionIdentity, CompileParameter, CompileParameterDefault, CompilePositionalPolicy,
    CompileSignature, CompileTargetSnapshot, HostTypeTarget, MirAggregate, MirCall, MirEffect,
    MirFunction, MirFunctionOwner, MirGlobalOperation, MirGuardId, MirHostOperation, MirHostPath,
    MirImmediate, MirLocalId, MirLoweringConfig, MirLoweringInput, MirOperand, MirPlace,
    MirReflectionOperation, MirRvalue, MirSafepoint, MirSafepointId, MirSourceOrigin, MirStatement,
    MirStatementKind, MirSwitchCase, MirSwitchValue, MirTargetTable, MirTerminator,
    MirTerminatorKind, MirValueType,
};

use super::{MirVerifyErrorKind, MirVerifyTarget, verify_mir};

mod builder_sweep;
mod contracts;
mod liveness;
mod try_regions;

const SOURCE: SourceId = SourceId::new(111);
const BODY: HirBodyId = HirBodyId::new(700);
const FUNCTION: FunctionId = FunctionId::new(7_000);

fn origin() -> MirSourceOrigin {
    MirSourceOrigin::body(BODY, Span::new(SOURCE, 0, 10))
}

fn signature(parameters: Vec<CompileParameter>) -> CompileSignature {
    CompileSignature {
        parameters,
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: None,
        effect: MirEffect::PURE,
    }
}

fn descriptor(parameters: Vec<CompileParameter>) -> CompileFunctionDescriptor {
    CompileFunctionDescriptor {
        id: FUNCTION,
        class: CompileFunctionClass::Script,
        canonical_symbol: "verifier::main".to_owned(),
        debug_name: "main".to_owned(),
        signature: signature(parameters),
        access: CompileFunctionAccess::script(false),
    }
}

fn target_table() -> MirTargetTable {
    let mut table = MirTargetTable::default();
    assert!(table.insert_function(descriptor(Vec::new())));
    table
}

fn function() -> MirFunction {
    MirFunction::new(
        BODY,
        MirFunctionOwner::Function(FUNCTION),
        "verifier::main",
        None,
        origin(),
    )
}

fn program(function: MirFunction) -> crate::MirProgram {
    let mut program = crate::MirProgram::new(target_table());
    program.add_function(function).expect("test MIR function");
    program
}

fn terminated_function() -> MirFunction {
    let mut function = function();
    function
        .set_terminator(
            function.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(None),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return terminator");
    function
}

fn scalar(value: i64) -> MirOperand {
    MirOperand::Immediate(MirImmediate::Scalar(ScalarValue::I64(value)))
}

fn verify_error(program: &crate::MirProgram) -> crate::MirVerifyError {
    verify_mir(program).expect_err("malformed MIR must fail verification")
}

fn build(source: &str, parameters: &[&str]) -> crate::MirProgram {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SOURCE,
        ModulePath::from_qualified("verifier_builder"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main body");
    let function_id = FunctionId::new(7_100);
    let body_origin = MirSourceOrigin::body(body.id, body.origin.span);
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function_id, body.id)],
    )
    .expect("executable analysis");
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_script_function(
            declaration.id,
            body.id,
            CompileFunctionDescriptor {
                id: function_id,
                class: CompileFunctionClass::Script,
                canonical_symbol: "verifier_builder::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: signature(
                    parameters
                        .iter()
                        .map(|name| CompileParameter {
                            name: (*name).to_owned(),
                            contract: None,
                            default: CompileParameterDefault::Required,
                            origin: None,
                        })
                        .collect(),
                ),
                access: CompileFunctionAccess::script(false),
            },
            body_origin,
        )
        .expect("script target");
    let targets = targets.build().expect("closed targets");
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(function_id),
        body.id,
        analysis.view(function_id).expect("analysis view"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: true,
        },
    )
    .expect("MIR lowering input");
    crate::build_mir(input).expect("full builder fixture")
}

#[test]
fn mir_verifier_accepts_full_builder_control_flow_and_loop_fixtures() {
    let control = build(
        "fn main(condition) { let value = if condition { 1 } else { 2 }; return value; }",
        &["condition"],
    );
    verify_mir(&control).expect("control-flow builder MIR verifies");

    let range = build(
        "fn main() { for value in 1..=3 { if value { continue; } } return 9; }",
        &[],
    );
    verify_mir(&range).expect("loop builder MIR verifies");

    let destructuring = build(
        "fn main(pair) { let (left, (middle, right)) = pair; return right; }",
        &["pair"],
    );
    assert!(destructuring.functions().any(|(_, function)| {
        function.guards().any(|(_, guard)| {
            matches!(
                guard.assumption,
                crate::MirGuardAssumption::TupleArity { .. }
            ) && guard.context.is_none()
        })
    }));
    verify_mir(&destructuring).expect("trapping tuple-destructuring MIR verifies");
}

#[test]
fn verified_mir_seals_semantic_budget_points_for_iterator_steps() {
    let program = build(
        "fn main() { for value in 1..=3 { if value { continue; } } return 9; }",
        &[],
    );
    let owned = crate::verify_owned_mir(program).expect("budget fixture verifies");
    let (function, _) = owned.program().functions().next().expect("main function");
    let analyses = owned.analyses(function).expect("sealed analyses");
    let statement_classes = analyses
        .budget
        .statement_points()
        .map(|(_, point)| point.class)
        .collect::<Vec<_>>();
    let terminator_classes = analyses
        .budget
        .terminator_points()
        .map(|(_, point)| point.class)
        .collect::<Vec<_>>();

    assert!(statement_classes.is_empty());
    assert!(terminator_classes.contains(&crate::MirBudgetClass::IteratorStep));
    assert!(
        analyses
            .budget
            .statement_points()
            .all(|(_, point)| point.units > 0)
    );
    assert!(
        analyses
            .budget
            .terminator_points()
            .all(|(_, point)| point.units > 0)
    );
}

#[test]
fn loop_backedge_budget_charge_does_not_apply_to_conditional_exit_edge() {
    let mut function = function();
    let entry = function.entry_block();
    let exit = function.add_block();
    function
        .set_terminator(
            entry,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Branch {
                    condition: MirOperand::Immediate(MirImmediate::Bool(true)),
                    then_block: entry,
                    else_block: exit,
                },
                MirEffect::PURE,
                None,
            ),
        )
        .expect("conditional loop terminator");
    function
        .set_terminator(
            exit,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(None),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("loop exit terminator");

    let owned = crate::verify_owned_mir(program(function)).expect("conditional loop verifies");
    let (function_id, _) = owned.program().functions().next().expect("root function");
    let budget = &owned.analyses(function_id).expect("sealed analyses").budget;
    assert_eq!(
        budget.edge(entry, entry).map(|point| point.class),
        Some(crate::MirBudgetClass::LoopBackedge)
    );
    assert_eq!(
        budget.edge(entry, exit),
        None,
        "the untaken loop successor must not charge the exit edge"
    );
    assert_eq!(
        budget.terminator_before(entry),
        None,
        "a successor-specific backedge charge must not move before the branch"
    );
}

#[test]
fn mir_verifier_rejects_undefined_function_reservations() {
    let mut program = crate::MirProgram::new(target_table());
    let reservation = program
        .reserve_function(BODY, MirFunctionOwner::Function(FUNCTION), origin())
        .expect("reservation");
    let error = verify_error(&program);
    assert_eq!(error.function, reservation);
    assert_eq!(
        error.into_kind(),
        MirVerifyErrorKind::UndefinedFunctionReservation
    );
}

#[test]
fn mir_verifier_rejects_unterminated_unreachable_and_missing_successor_blocks() {
    let unterminated = program(function());
    assert_eq!(
        verify_error(&unterminated).into_kind(),
        MirVerifyErrorKind::UnterminatedBlock
    );

    let mut unreachable = terminated_function();
    let dead = unreachable.add_block();
    unreachable
        .set_terminator(
            dead,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(Some(MirOperand::Immediate(MirImmediate::Unit))),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("dead terminator");
    assert_eq!(
        verify_error(&program(unreachable)).into_kind(),
        MirVerifyErrorKind::UnreachableBlock
    );

    let mut missing = function();
    let missing_block = crate::MirBlockId::from_index(99);
    missing.verifier_test_set_terminator_unchecked(
        missing.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::Jump(missing_block),
            MirEffect::PURE,
            None,
        ),
    );
    assert_eq!(
        verify_error(&program(missing)).into_kind(),
        MirVerifyErrorKind::MissingBlock(missing_block)
    );
}

#[test]
fn mir_verifier_checks_every_referenced_identity_family() {
    let mut local = function();
    let destination = local.add_synthetic_local(MirValueType::Dynamic, origin());
    let missing_local = MirLocalId::from_index(99);
    local
        .append_statement(
            local.entry_block(),
            MirStatement::assign(
                origin(),
                MirPlace::local(destination),
                MirRvalue::Use(MirOperand::Local(missing_local)),
            ),
        )
        .expect("unchecked operands are verifier-owned");
    local
        .set_terminator(
            local.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(None),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return");
    assert_eq!(
        verify_error(&program(local)).into_kind(),
        MirVerifyErrorKind::MissingLocal(missing_local)
    );

    let mut guard = function();
    guard.verifier_test_append_statement_unchecked(
        guard.entry_block(),
        MirStatement::new(
            origin(),
            None,
            MirStatementKind::GuardTrap {
                value: scalar(1),
                guard: MirGuardId::from_index(99),
            },
            MirEffect::may_trap(),
            None,
        ),
    );
    guard.verifier_test_set_terminator_unchecked(
        guard.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::Return(None),
            MirEffect::PURE,
            None,
        ),
    );
    assert!(matches!(
        verify_error(&program(guard)).into_kind(),
        MirVerifyErrorKind::MissingGuard(_)
    ));

    let mut global = function();
    let destination = global.add_synthetic_local(MirValueType::Dynamic, origin());
    let global_id = GlobalId::new(91);
    global
        .append_statement(
            global.entry_block(),
            MirStatement::new(
                origin(),
                Some(MirPlace::local(destination)),
                MirStatementKind::Global(MirGlobalOperation::Read { global: global_id }),
                MirEffect::global_read(),
                None,
            ),
        )
        .expect("global statement");
    global
        .set_terminator(
            global.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(None),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return");
    assert_eq!(
        verify_error(&program(global)).into_kind(),
        MirVerifyErrorKind::MissingTarget(MirVerifyTarget::Global(global_id))
    );
}

#[test]
fn mir_verifier_requires_local_initialization_on_every_predecessor() {
    let mut function = function();
    let local = function.add_synthetic_local(MirValueType::Dynamic, origin());
    let left = function.add_block();
    let right = function.add_block();
    let join = function.add_block();
    function
        .set_terminator(
            function.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Branch {
                    condition: MirOperand::Immediate(MirImmediate::Bool(true)),
                    then_block: left,
                    else_block: right,
                },
                MirEffect::PURE,
                None,
            ),
        )
        .expect("branch");
    function
        .append_statement(
            left,
            MirStatement::assign(origin(), MirPlace::local(local), MirRvalue::Use(scalar(1))),
        )
        .expect("left definition");
    for block in [left, right] {
        function
            .set_terminator(
                block,
                MirTerminator::new(
                    origin(),
                    MirTerminatorKind::Jump(join),
                    MirEffect::PURE,
                    None,
                ),
            )
            .expect("join edge");
    }
    function
        .set_terminator(
            join,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(Some(MirOperand::Local(local))),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return");
    assert_eq!(
        verify_error(&program(function)).into_kind(),
        MirVerifyErrorKind::LocalUseBeforeInitialization(local)
    );
}

#[test]
fn mir_verifier_enforces_temp_definition_and_dominance() {
    let mut undefined = function();
    let temp = undefined.add_temp(MirValueType::Dynamic, origin());
    undefined
        .set_terminator(
            undefined.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(Some(MirOperand::Temp(temp))),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return");
    assert_eq!(
        verify_error(&program(undefined)).into_kind(),
        MirVerifyErrorKind::TempHasNoDefinition(temp)
    );

    let mut dominance = function();
    let temp = dominance.add_temp(MirValueType::Primitive(PrimitiveTag::I64), origin());
    let left = dominance.add_block();
    let right = dominance.add_block();
    let join = dominance.add_block();
    dominance
        .set_terminator(
            dominance.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Branch {
                    condition: MirOperand::Immediate(MirImmediate::Bool(true)),
                    then_block: left,
                    else_block: right,
                },
                MirEffect::PURE,
                None,
            ),
        )
        .expect("branch");
    dominance
        .append_statement(
            left,
            MirStatement::assign(origin(), MirPlace::temp(temp), MirRvalue::Use(scalar(1))),
        )
        .expect("temp definition");
    for block in [left, right] {
        dominance
            .set_terminator(
                block,
                MirTerminator::new(
                    origin(),
                    MirTerminatorKind::Jump(join),
                    MirEffect::PURE,
                    None,
                ),
            )
            .expect("join edge");
    }
    dominance
        .set_terminator(
            join,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(Some(MirOperand::Temp(temp))),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return");
    assert!(matches!(
        verify_error(&program(dominance)).into_kind(),
        MirVerifyErrorKind::TempUseNotDominated { temp: actual, .. } if actual == temp
    ));

    let mut reassigned = function();
    let temp = reassigned.add_temp(MirValueType::Primitive(PrimitiveTag::I64), origin());
    reassigned
        .append_statement(
            reassigned.entry_block(),
            MirStatement::assign(origin(), MirPlace::temp(temp), MirRvalue::Use(scalar(1))),
        )
        .expect("first definition");
    reassigned.verifier_test_append_statement_unchecked(
        reassigned.entry_block(),
        MirStatement::assign(origin(), MirPlace::temp(temp), MirRvalue::Use(scalar(2))),
    );
    reassigned.verifier_test_set_terminator_unchecked(
        reassigned.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::Return(None),
            MirEffect::PURE,
            None,
        ),
    );
    assert_eq!(
        verify_error(&program(reassigned)).into_kind(),
        MirVerifyErrorKind::TempHasMultipleDefinitions(temp)
    );
}

#[test]
fn mir_verifier_rechecks_destination_effect_safepoint_and_origin_metadata() {
    let mut destination = function();
    let local = destination.add_synthetic_local(MirValueType::Dynamic, origin());
    destination.verifier_test_append_statement_unchecked(
        destination.entry_block(),
        MirStatement::new(
            origin(),
            Some(MirPlace::local(local)),
            MirStatementKind::GuardTrap {
                value: scalar(1),
                guard: MirGuardId::from_index(0),
            },
            MirEffect::may_trap(),
            None,
        ),
    );
    destination.verifier_test_set_terminator_unchecked(
        destination.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::Return(None),
            MirEffect::PURE,
            None,
        ),
    );
    assert!(matches!(
        verify_error(&program(destination)).into_kind(),
        MirVerifyErrorKind::InvalidDestination {
            expected: super::MirDestinationExpectation::Forbidden
        }
    ));

    let mut effect = function();
    let local = effect.add_synthetic_local(MirValueType::Dynamic, origin());
    effect.verifier_test_append_statement_unchecked(
        effect.entry_block(),
        MirStatement::new(
            origin(),
            Some(MirPlace::local(local)),
            MirStatementKind::MakeRange {
                start: scalar(1),
                end: scalar(2),
                inclusive: false,
            },
            MirEffect::PURE,
            None,
        ),
    );
    effect.verifier_test_set_terminator_unchecked(
        effect.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::Return(None),
            MirEffect::PURE,
            None,
        ),
    );
    assert!(matches!(
        verify_error(&program(effect)).into_kind(),
        MirVerifyErrorKind::IncompleteEffect { .. }
    ));

    let mut safepoint = function();
    let local = safepoint.add_synthetic_local(MirValueType::Dynamic, origin());
    safepoint.verifier_test_append_statement_unchecked(
        safepoint.entry_block(),
        MirStatement::new(
            origin(),
            Some(MirPlace::local(local)),
            MirStatementKind::Allocate(MirAggregate::Array(Vec::new())),
            MirEffect::allocation(),
            None,
        ),
    );
    safepoint.verifier_test_set_terminator_unchecked(
        safepoint.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::Return(None),
            MirEffect::PURE,
            None,
        ),
    );
    assert_eq!(
        verify_error(&program(safepoint)).into_kind(),
        MirVerifyErrorKind::MissingRequiredSafepoint
    );

    let mut source = function();
    let invalid = MirSourceOrigin::declaration(HirDeclId::new(1), origin().span);
    let local = source.add_synthetic_local(MirValueType::Dynamic, origin());
    source
        .append_statement(
            source.entry_block(),
            MirStatement::assign(invalid, MirPlace::local(local), MirRvalue::Use(scalar(1))),
        )
        .expect("model permits verifier fixture origin");
    source
        .set_terminator(
            source.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(None),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return");
    assert!(matches!(
        verify_error(&program(source)).into_kind(),
        MirVerifyErrorKind::InvalidSourceOrigin(_)
    ));
}

#[test]
fn mir_verifier_checks_branch_switch_iterator_and_range_contracts() {
    let mut branch = function();
    let yes = branch.add_block();
    let no = branch.add_block();
    branch
        .set_terminator(
            branch.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Branch {
                    condition: scalar(1),
                    then_block: yes,
                    else_block: no,
                },
                MirEffect::PURE,
                None,
            ),
        )
        .expect("branch");
    for block in [yes, no] {
        branch
            .set_terminator(
                block,
                MirTerminator::new(
                    origin(),
                    MirTerminatorKind::Return(None),
                    MirEffect::PURE,
                    None,
                ),
            )
            .expect("return");
    }
    verify_mir(&program(branch)).expect("truthy branch operands are valid MIR");

    let mut switch = function();
    let case = switch.add_block();
    let otherwise = switch.add_block();
    switch
        .set_terminator(
            switch.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Switch {
                    discriminant: MirOperand::Immediate(MirImmediate::Bool(true)),
                    cases: vec![
                        MirSwitchCase {
                            value: MirSwitchValue::Bool(true),
                            target: case,
                        },
                        MirSwitchCase {
                            value: MirSwitchValue::Bool(true),
                            target: case,
                        },
                    ],
                    otherwise,
                },
                MirEffect::PURE,
                None,
            ),
        )
        .expect("switch");
    for block in [case, otherwise] {
        switch
            .set_terminator(
                block,
                MirTerminator::new(
                    origin(),
                    MirTerminatorKind::Return(None),
                    MirEffect::PURE,
                    None,
                ),
            )
            .expect("return");
    }
    assert!(matches!(
        verify_error(&program(switch)).into_kind(),
        MirVerifyErrorKind::InvalidTerminatorContract(_)
    ));

    let mut iterator = function();
    let iterator_local = iterator.add_synthetic_local(MirValueType::Iterator, origin());
    let next = iterator.add_block();
    let done = iterator.add_block();
    let safepoint = iterator.add_safepoint(MirSafepoint::new(origin()));
    iterator.verifier_test_set_terminator_unchecked(
        iterator.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::IteratorNext {
                iterator: MirOperand::Local(iterator_local),
                item: MirLocalId::from_index(99),
                next,
                done,
            },
            MirEffect::dynamic_call(),
            Some(safepoint),
        ),
    );
    for block in [next, done] {
        iterator
            .set_terminator(
                block,
                MirTerminator::new(
                    origin(),
                    MirTerminatorKind::Return(None),
                    MirEffect::PURE,
                    None,
                ),
            )
            .expect("return");
    }
    assert!(matches!(
        verify_error(&program(iterator)).into_kind(),
        MirVerifyErrorKind::MissingLocal(_)
    ));

    let mut range = function();
    let cursor = range.add_synthetic_local(MirValueType::Primitive(PrimitiveTag::I64), origin());
    let exhausted =
        range.add_synthetic_local(MirValueType::Primitive(PrimitiveTag::Bool), origin());
    let invalid_item =
        range.add_synthetic_local(MirValueType::Primitive(PrimitiveTag::Bool), origin());
    let next = range.add_block();
    let done = range.add_block();
    range
        .set_terminator(
            range.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::RangeNext {
                    cursor,
                    end: scalar(3),
                    exhausted,
                    inclusive: false,
                    item: invalid_item,
                    mode: crate::MirRangeStepMode::I64Proven,
                    next,
                    done,
                },
                MirEffect::PURE,
                None,
            ),
        )
        .expect("range terminator");
    for block in [next, done] {
        range
            .set_terminator(
                block,
                MirTerminator::new(
                    origin(),
                    MirTerminatorKind::Return(None),
                    MirEffect::PURE,
                    None,
                ),
            )
            .expect("return");
    }
    assert!(matches!(
        verify_error(&program(range)).into_kind(),
        MirVerifyErrorKind::InvalidTerminatorContract(_)
    ));
}

#[test]
fn mir_verifier_checks_call_host_and_reflection_descriptors() {
    let missing_function = FunctionId::new(99_001);
    let mut call = function();
    let local = call.add_synthetic_local(MirValueType::Dynamic, origin());
    let safepoint = call.add_safepoint(MirSafepoint::new(origin()));
    call.append_statement(
        call.entry_block(),
        MirStatement::new(
            origin(),
            Some(MirPlace::local(local)),
            MirStatementKind::Call(MirCall::NativeFunction {
                function: missing_function,
                debug_name: "missing".to_owned(),
                signature: CompileSignature {
                    parameters: Vec::new(),
                    positional: CompilePositionalPolicy::RuntimeChecked,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                arguments: Vec::new(),
            }),
            MirEffect::external_call(),
            Some(safepoint),
        ),
    )
    .expect("call statement");
    call.set_terminator(
        call.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::Return(None),
            MirEffect::PURE,
            None,
        ),
    )
    .expect("return");
    assert_eq!(
        verify_error(&program(call)).into_kind(),
        MirVerifyErrorKind::MissingTarget(MirVerifyTarget::Function(missing_function))
    );

    let host_type = HostTypeTarget {
        semantic: TypeId::new(81),
        runtime: vela_common::HostTypeId::new(82),
    };
    let mut host = function();
    let local = host.add_synthetic_local(MirValueType::Dynamic, origin());
    let safepoint = host.add_safepoint(MirSafepoint::new(origin()));
    host.append_statement(
        host.entry_block(),
        MirStatement::new(
            origin(),
            Some(MirPlace::local(local)),
            MirStatementKind::Host(MirHostOperation::Read {
                root: scalar(1),
                path: MirHostPath {
                    root_type: host_type,
                    segments: Vec::new(),
                },
            }),
            MirEffect::host_read(),
            Some(safepoint),
        ),
    )
    .expect("host read");
    host.set_terminator(
        host.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::Return(None),
            MirEffect::PURE,
            None,
        ),
    )
    .expect("return");
    assert_eq!(
        verify_error(&program(host)).into_kind(),
        MirVerifyErrorKind::MissingTarget(MirVerifyTarget::Type(host_type.semantic))
    );

    let mut reflection = function();
    let local = reflection.add_synthetic_local(MirValueType::Dynamic, origin());
    let safepoint = reflection.add_safepoint(MirSafepoint::new(origin()));
    reflection
        .append_statement(
            reflection.entry_block(),
            MirStatement::new(
                origin(),
                Some(MirPlace::local(local)),
                MirStatementKind::Reflect(MirReflectionOperation::Read {
                    function: missing_function,
                    target: scalar(1),
                    member: scalar(2),
                }),
                MirEffect::reflection_read(),
                Some(safepoint),
            ),
        )
        .expect("reflection read");
    reflection
        .set_terminator(
            reflection.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(None),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return");
    assert_eq!(
        verify_error(&program(reflection)).into_kind(),
        MirVerifyErrorKind::MissingTarget(MirVerifyTarget::Function(missing_function))
    );
}

#[test]
fn mir_verifier_rejects_missing_and_mismatched_safepoint_ids() {
    let mut function = function();
    let local = function.add_synthetic_local(MirValueType::Dynamic, origin());
    let missing = MirSafepointId::from_index(99);
    function.verifier_test_append_statement_unchecked(
        function.entry_block(),
        MirStatement::new(
            origin(),
            Some(MirPlace::local(local)),
            MirStatementKind::Allocate(MirAggregate::Array(Vec::new())),
            MirEffect::allocation(),
            Some(missing),
        ),
    );
    function.verifier_test_set_terminator_unchecked(
        function.entry_block(),
        MirTerminator::new(
            origin(),
            MirTerminatorKind::Return(None),
            MirEffect::PURE,
            None,
        ),
    );
    assert_eq!(
        verify_error(&program(function)).into_kind(),
        MirVerifyErrorKind::MissingSafepoint(missing)
    );
}
