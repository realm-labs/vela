use super::*;

use crate::{MirLiveValue, MirLiveness};

fn only_function(program: &crate::MirProgram) -> &MirFunction {
    let mut functions = program.functions();
    let (_, function) = functions.next().expect("expected one MIR function");
    assert!(functions.next().is_none(), "expected one MIR function");
    function
}

#[test]
fn mir_liveness_tracks_join_locals_and_early_returns() {
    let program = build(
        "fn main(condition) { let value = if condition { 1 } else { 2 }; if condition { return value; } return value + 1; }",
        &["condition"],
    );
    let function = only_function(&program);
    assert!(function.liveness().is_computed());
    let value = function
        .debug_locals()
        .find_map(|(_, debug)| (debug.name == "value").then_some(debug.storage))
        .expect("value debug local");
    let return_blocks = function
        .blocks()
        .filter_map(|(block, data)| {
            matches!(
                data.terminator().map(|terminator| &terminator.kind),
                Some(MirTerminatorKind::Return(_))
            )
            .then_some(block)
        })
        .collect::<Vec<_>>();
    assert_eq!(return_blocks.len(), 2);
    assert!(return_blocks.iter().all(|block| {
        function
            .liveness()
            .block_live_in
            .get(block)
            .is_some_and(|values| values.contains(&MirLiveValue::Local(value)))
    }));
    let debug = function
        .debug_locals()
        .find_map(|(_, debug)| (debug.storage == value).then_some(debug))
        .expect("value debug metadata");
    assert!(
        return_blocks
            .iter()
            .all(|block| debug.live_region.blocks.contains(block))
    );
    verify_mir(&program).expect("join and early-return liveness verifies");
}

#[test]
fn mir_liveness_uses_operation_live_before_at_safepoints() {
    let program = build(
        "fn main(value) { let items = [value]; return value; }",
        &["value"],
    );
    let function = only_function(&program);
    let value = function.parameters()[0].storage;
    let (destination, safepoint) = function
        .statements()
        .find_map(|(_, statement)| {
            matches!(
                statement.kind,
                MirStatementKind::Allocate(MirAggregate::Array(_))
            )
            .then(|| {
                (
                    statement.destination.expect("allocation destination"),
                    statement.safepoint.expect("allocation safepoint"),
                )
            })
        })
        .expect("array allocation");
    let live = &function
        .safepoint(safepoint)
        .expect("safepoint metadata")
        .live_values;
    assert!(live.contains(&MirLiveValue::Local(value)));
    let destination = match destination {
        MirPlace::Local(local) => MirLiveValue::Local(local),
        MirPlace::Temp(temp) => MirLiveValue::Temp(temp),
    };
    assert!(
        !live.contains(&destination),
        "an operation result is not live before it is produced"
    );
    verify_mir(&program).expect("safepoint liveness verifies");
}

#[test]
fn await_terminator_defines_its_result_on_resume_and_roots_live_parents() {
    let parameter = CompileParameter {
        name: "callback".to_owned(),
        contract: None,
        default: CompileParameterDefault::Required,
        origin: None,
    };
    let mut async_signature = signature(vec![parameter]);
    async_signature.asyncness = vela_common::CallableAsyncness::Async;
    let mut targets = MirTargetTable::default();
    assert!(targets.insert_function(CompileFunctionDescriptor {
        id: FUNCTION,
        class: CompileFunctionClass::Script,
        canonical_symbol: "verifier::main".to_owned(),
        debug_name: "main".to_owned(),
        signature: async_signature,
        access: CompileFunctionAccess::script(false),
    }));
    let mut function = function();
    function.set_asyncness(vela_common::CallableAsyncness::Async);
    let callback = function.add_parameter(crate::MirParameterSpec {
        hir_local: vela_hir::ids::HirLocalId::new(1),
        kind: crate::MirParameterKind::Explicit(vela_hir::ids::HirParamId::new(1)),
        name: "callback".to_owned(),
        value_type: MirValueType::Dynamic,
        contract: None,
        default_body: None,
        origin: origin(),
    });
    let destination = function.add_synthetic_local(MirValueType::Dynamic, origin());
    let source = function.entry_block();
    let resume = function.add_block();
    let safepoint = function.add_safepoint(MirSafepoint::new(origin()));
    function
        .set_terminator(
            source,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::AwaitCall {
                    operation: Box::new(crate::MirAwaitOperation::Call(MirCall::DynamicCallable {
                        callee: MirOperand::Local(callback),
                        arguments: Vec::new(),
                    })),
                    destination: MirPlace::Local(destination),
                    resume,
                },
                MirEffect::dynamic_call(),
                Some(safepoint),
            ),
        )
        .expect("await terminator");
    function
        .set_terminator(
            resume,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(Some(MirOperand::Local(destination))),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("resume return");
    crate::liveness::apply(&mut function);
    let mut program = crate::MirProgram::new(targets);
    program.add_function(function).expect("async test function");
    let function = only_function(&program);
    let destination = MirLiveValue::Local(destination);
    let callback = MirLiveValue::Local(callback);

    assert!(function.liveness().block_live_out[&source].contains(&destination));
    assert!(function.liveness().block_live_in[&resume].contains(&destination));
    let roots = &function
        .safepoint(safepoint)
        .expect("await root map")
        .live_values;
    assert!(roots.contains(&callback));
    assert!(!roots.contains(&destination));
    verify_mir(&program).expect("await MIR and liveness verify");
}

#[test]
fn mir_liveness_applies_iterator_edge_definitions() {
    let program = build(
        "fn main() { for value in [1] { return value; } return 0; }",
        &[],
    );
    let function = only_function(&program);
    let (header, item, next, safepoint) = function
        .blocks()
        .find_map(|(block, data)| {
            let terminator = data.terminator()?;
            let MirTerminatorKind::IteratorNext { item, next, .. } = terminator.kind else {
                return None;
            };
            Some((
                block,
                item,
                next,
                terminator.safepoint.expect("iterator safepoint"),
            ))
        })
        .expect("iterator header");
    let item = MirLiveValue::Local(item);
    assert!(
        !function.liveness().block_live_in[&header].contains(&item),
        "the yielded item is defined on the next edge"
    );
    assert!(function.liveness().block_live_out[&header].contains(&item));
    assert!(function.liveness().block_live_in[&next].contains(&item));
    assert!(
        !function
            .safepoint(safepoint)
            .expect("iterator safepoint")
            .live_values
            .contains(&item),
        "the item is not live before iterator-next produces it"
    );
    verify_mir(&program).expect("iterator edge liveness verifies");
}

#[test]
fn mir_liveness_applies_range_edge_definitions() {
    let program = build(
        "fn main() { for value in 1..3 { return value; } return 0; }",
        &[],
    );
    let function = only_function(&program);
    let (header, cursor, exhausted, item, next, done) = function
        .blocks()
        .find_map(|(block, data)| {
            let terminator = data.terminator()?;
            let MirTerminatorKind::RangeNext {
                cursor,
                exhausted,
                item,
                next,
                done,
                ..
            } = &terminator.kind
            else {
                return None;
            };
            Some((block, *cursor, *exhausted, *item, *next, *done))
        })
        .expect("range header");
    let cursor = MirLiveValue::Local(cursor);
    let exhausted = MirLiveValue::Local(exhausted);
    let item = MirLiveValue::Local(item);
    let header_live = &function.liveness().block_live_in[&header];
    assert!(header_live.contains(&cursor));
    assert!(header_live.contains(&exhausted));
    assert!(!header_live.contains(&item));
    assert!(function.liveness().block_live_in[&next].contains(&item));
    assert!(!function.liveness().block_live_in[&done].contains(&item));
    verify_mir(&program).expect("range edge liveness verifies");
}

#[test]
fn mir_liveness_verifier_rejects_missing_or_partial_computed_metadata() {
    let mut missing = terminated_function();
    missing.set_liveness(MirLiveness {
        computed: true,
        ..MirLiveness::default()
    });
    assert!(matches!(
        verify_error(&program(missing)).into_kind(),
        MirVerifyErrorKind::InvalidLivenessMetadata(_)
    ));

    let mut partial = terminated_function();
    partial.set_liveness(MirLiveness {
        block_live_in: [(partial.entry_block(), std::collections::BTreeSet::new())]
            .into_iter()
            .collect(),
        ..MirLiveness::default()
    });
    assert!(matches!(
        verify_error(&program(partial)).into_kind(),
        MirVerifyErrorKind::InvalidLivenessMetadata(_)
    ));
}

#[test]
fn mir_backend_handoff_requires_computed_metadata() {
    let computed = build("fn main() { return 1; }", &[]);
    let computed = crate::verify_owned_mir(computed).expect("computed MIR verifies");
    let handoff = computed
        .backend_handoff()
        .expect("computed MIR reaches backend handoff");
    assert!(std::ptr::eq(handoff.program(), computed.program()));
    let function = handoff
        .program()
        .functions()
        .next()
        .expect("root function")
        .0;
    assert!(handoff.analyses(function).is_some());

    let uncomputed = program(terminated_function());
    let uncomputed =
        crate::verify_owned_mir(uncomputed).expect("explicit test configuration may omit liveness");
    let error = uncomputed
        .backend_handoff()
        .expect_err("a physical backend may not consume uncomputed MIR");
    assert!(matches!(
        error,
        crate::MirBackendHandoffError::MissingLiveness { .. }
    ));
}

#[test]
fn lexical_debug_availability_outlives_value_liveness() {
    let program = build(
        "fn main() { let visible = 1; let used = visible + 1; let after = 2; return after; }",
        &[],
    );
    let owned = crate::verify_owned_mir(program).expect("debug fixture verifies");
    let (function_id, function) = owned.program().functions().next().expect("root function");
    let analyses = owned.analyses(function_id).expect("sealed analyses");
    let (debug_id, debug) = function
        .debug_locals()
        .find(|(_, debug)| debug.name == "visible")
        .expect("visible debug local");
    let last_statement = function
        .blocks()
        .flat_map(|(_, block)| block.statements().iter().copied())
        .last()
        .expect("after assignment statement");
    assert!(
        analyses.debug_availability.statement_before[&last_statement].contains(&debug_id),
        "lexically visible local remains debugger-available after its last value use"
    );
    assert!(
        !analyses.value_liveness.statement_live_before[&last_statement]
            .contains(&crate::MirLiveValue::Local(debug.storage)),
        "register-allocation liveness is intentionally shorter"
    );
}

#[test]
fn lexical_debug_availability_ends_at_nested_scope_exit() {
    let program = build(
        "fn main() { { let scoped = 1; let used = scoped + 1; let after_use = 2; } let outside = 3; return outside; }",
        &[],
    );
    let owned = crate::verify_owned_mir(program).expect("nested debug fixture verifies");
    let (function_id, function) = owned.program().functions().next().expect("root function");
    let analyses = owned.analyses(function_id).expect("sealed analyses");
    let (scoped_id, scoped) = function
        .debug_locals()
        .find(|(_, debug)| debug.name == "scoped")
        .expect("scoped debug local");
    let after_use = function
        .debug_locals()
        .find(|(_, debug)| debug.name == "after_use")
        .expect("after-use debug local")
        .1
        .storage;
    let outside = function
        .debug_locals()
        .find(|(_, debug)| debug.name == "outside")
        .expect("outside debug local")
        .1
        .storage;
    let after_use_statement = function
        .statements()
        .find(|(_, statement)| statement.destination == Some(crate::MirPlace::Local(after_use)))
        .expect("after-use assignment")
        .0;
    let outside_statement = function
        .statements()
        .find(|(_, statement)| statement.destination == Some(crate::MirPlace::Local(outside)))
        .expect("outside assignment")
        .0;

    assert!(
        analyses.debug_availability.statement_before[&after_use_statement].contains(&scoped_id),
        "the local remains visible after its final value use while its scope is active"
    );
    assert!(
        !analyses.value_liveness.statement_live_before[&after_use_statement]
            .contains(&crate::MirLiveValue::Local(scoped.storage)),
        "the final value use precedes the lexical availability endpoint"
    );
    assert!(
        !analyses.debug_availability.statement_before[&outside_statement].contains(&scoped_id),
        "the local disappears at the first statement after lexical scope exit"
    );
}

#[test]
fn root_liveness_filters_non_root_scalars_at_each_safepoint() {
    let program = build(
        "fn main() { let number = 1; let text = \"root\"; let values = [text, number]; return values; }",
        &[],
    );
    let owned = crate::verify_owned_mir(program).expect("root fixture verifies");
    let (function_id, function) = owned.program().functions().next().expect("root function");
    let analyses = owned.analyses(function_id).expect("sealed analyses");
    let number = function
        .debug_locals()
        .find(|(_, debug)| debug.name == "number")
        .expect("number local")
        .1
        .storage;
    let roots = analyses
        .root_liveness
        .live_before_safepoint
        .values()
        .max_by_key(|roots| roots.len())
        .expect("allocation safepoints have root maps");
    assert!(
        !roots.is_empty(),
        "array allocation retains its heap operand"
    );
    assert!(!roots.contains(&crate::MirLiveValue::Local(number)));
}
