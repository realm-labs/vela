use vela_common::{PrimitiveTag, ScalarValue, SourceId, Span};
use vela_def::FunctionId;
use vela_hir::ids::HirBodyId;

use crate::*;

fn origin(body: HirBodyId) -> MirSourceOrigin {
    MirSourceOrigin::body(body, Span::new(SourceId::new(7), 0, 5))
}

fn test_function(body: HirBodyId, owner: MirFunctionOwner, origin: MirSourceOrigin) -> MirFunction {
    MirFunction::new(
        body,
        owner,
        format!("test::body_{}", body.get()),
        None,
        origin,
    )
}

#[test]
fn mir_model_iterator_and_range_steps_validate_boundary_metadata() {
    let body = HirBodyId::new(25);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(250)),
        origin,
    );
    let entry = function.entry_block();
    let next = function.add_block();
    let done = function.add_block();
    let range = function.add_block();
    let iterator = function.add_synthetic_local(MirValueType::Iterator, origin);
    let item = function.add_synthetic_local(MirValueType::Dynamic, origin);
    let iterator_step = MirTerminator::new(
        origin,
        MirTerminatorKind::IteratorNext {
            iterator: MirOperand::Local(iterator),
            item,
            next,
            done,
        },
        MirEffect::PURE,
        None,
    );
    assert_eq!(
        function.set_terminator(entry, iterator_step.clone()),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::dynamic_call(),
            actual: MirEffect::PURE,
        })
    );
    let iterator_step = MirTerminator {
        effect: MirEffect::dynamic_call(),
        ..iterator_step
    };
    assert_eq!(
        function.set_terminator(entry, iterator_step.clone()),
        Err(MirBuildError::MissingSafepoint { origin })
    );
    let safepoint = function.add_safepoint(MirSafepoint::new(origin));
    function
        .set_terminator(
            entry,
            MirTerminator {
                safepoint: Some(safepoint),
                ..iterator_step
            },
        )
        .expect("iterator stepping should expose its callback/allocation boundary");

    let cursor = function.add_synthetic_local(MirValueType::Dynamic, origin);
    let exhausted =
        function.add_synthetic_local(MirValueType::Primitive(PrimitiveTag::Bool), origin);
    let range_step = MirTerminator::new(
        origin,
        MirTerminatorKind::RangeNext {
            cursor,
            end: MirOperand::Immediate(MirImmediate::Scalar(ScalarValue::I64(3))),
            exhausted,
            inclusive: false,
            item,
            mode: MirRangeStepMode::DynamicInteger,
            next,
            done,
        },
        MirEffect::PURE,
        None,
    );
    assert_eq!(
        function.set_terminator(range, range_step.clone()),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::may_trap(),
            actual: MirEffect::PURE,
        })
    );
    function
        .set_terminator(
            range,
            MirTerminator {
                effect: MirEffect::may_trap(),
                ..range_step
            },
        )
        .expect("dynamic range stepping may trap but does not allocate");
}

#[test]
fn mir_model_guards_expose_slow_paths_as_cfg_edges() {
    let body = HirBodyId::new(26);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(260)),
        origin,
    );
    let entry = function.entry_block();
    let guard_block = function.add_block();
    let passed = function.add_block();
    let slow = function.add_block();
    let value = function.add_synthetic_local(MirValueType::Dynamic, origin);
    let missing_guard = MirGuardId::from_index(99);
    let trap = MirStatement::new(
        origin,
        None,
        MirStatementKind::GuardTrap {
            value: MirOperand::Local(value),
            guard: missing_guard,
        },
        MirEffect::may_trap(),
        None,
    );
    assert_eq!(
        function.append_statement(entry, trap),
        Err(MirBuildError::MissingGuard {
            guard: missing_guard,
            origin,
        })
    );
    let guard = function.add_guard(MirGuard {
        assumption: MirGuardAssumption::TruthyBoolean,
        origin,
    });
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                None,
                MirStatementKind::GuardTrap {
                    value: MirOperand::Local(value),
                    guard,
                },
                MirEffect::may_trap(),
                None,
            ),
        )
        .expect("contract guards may trap without introducing a hidden edge");
    let branch = MirTerminator::new(
        origin,
        MirTerminatorKind::GuardBranch {
            value: MirOperand::Local(value),
            guard,
            passed,
            slow,
        },
        MirEffect::PURE,
        None,
    );
    function
        .set_terminator(guard_block, branch)
        .expect("recoverable guard transitions should have two visible CFG successors");
}

#[test]
fn mir_model_represents_ranges_formats_defaults_and_iterator_control() {
    let body = HirBodyId::new(22);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(220)),
        origin,
    );
    let entry = function.entry_block();
    let next = function.add_block();
    let done = function.add_block();
    let iterator = function.add_synthetic_local(MirValueType::Iterator, origin);
    let item = function.add_synthetic_local(MirValueType::Dynamic, origin);
    let range = function.add_temp(MirValueType::Range, origin);
    let formatted = function.add_temp(MirValueType::Primitive(PrimitiveTag::String), origin);
    let call_result = function.add_temp(MirValueType::Dynamic, origin);
    let format_safepoint = function.add_safepoint(MirSafepoint::new(origin));
    let call_safepoint = function.add_safepoint(MirSafepoint::new(origin));
    let iterator_safepoint = function.add_safepoint(MirSafepoint::new(origin));

    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(range)),
                MirStatementKind::MakeRange {
                    start: MirOperand::Immediate(MirImmediate::Scalar(ScalarValue::I64(1))),
                    end: MirOperand::Immediate(MirImmediate::Scalar(ScalarValue::I64(3))),
                    inclusive: true,
                },
                MirEffect::may_trap(),
                None,
            ),
        )
        .expect("range construction should be an explicit trapping MIR operation");
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(formatted)),
                MirStatementKind::FormatString {
                    parts: vec![
                        MirFormatPart::Text("range=".to_owned()),
                        MirFormatPart::Value(MirOperand::Temp(range)),
                    ],
                },
                MirEffect::allocation(),
                Some(format_safepoint),
            ),
        )
        .expect("format construction should retain ordered parts");
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(call_result)),
                MirStatementKind::Call(MirCall::ScriptFunction {
                    function: FunctionId::new(221),
                    debug_name: "script::format_range".to_owned(),
                    signature: CompileSignature {
                        parameters: vec![
                            CompileParameter {
                                name: "value".to_owned(),
                                contract: None,
                                default: CompileParameterDefault::Required,
                                origin: None,
                            },
                            CompileParameter {
                                name: "fallback".to_owned(),
                                contract: None,
                                default: CompileParameterDefault::HirBody(HirBodyId::new(222)),
                                origin: Some(origin),
                            },
                        ],
                        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                        return_contract: None,
                        effect: MirEffect::PURE,
                    },
                    arguments: vec![
                        MirScriptArgument::placed(0, MirOperand::Temp(formatted)),
                        MirScriptArgument::missing(1),
                    ],
                }),
                MirEffect::script_call(),
                Some(call_safepoint),
            ),
        )
        .expect("static call should retain placed and missing parameter slots");
    function
        .set_terminator(
            entry,
            MirTerminator::new(
                origin,
                MirTerminatorKind::IteratorNext {
                    iterator: MirOperand::Local(iterator),
                    item,
                    next,
                    done,
                },
                MirEffect::dynamic_call(),
                Some(iterator_safepoint),
            ),
        )
        .expect("iterator exhaustion should be explicit CFG control");
    for block in [next, done] {
        function
            .set_terminator(
                block,
                MirTerminator::new(
                    origin,
                    MirTerminatorKind::Return(None),
                    MirEffect::PURE,
                    None,
                ),
            )
            .expect("iterator successor should terminate");
    }
    let mut program = MirProgram::new(MirTargetTable::default());
    program
        .add_function(function)
        .expect("fixture function should be inserted");
    let dump = program.dump();

    assert!(dump.contains("range.make 1i64, 3i64 inclusive=true"));
    assert!(dump.contains("format[text(\"range=\"), value(t0)]"));
    assert!(dump.contains("p0=t1, p1=<missing>"));
    assert!(dump.contains("iterator.next l0 -> l1, next bb1, done bb2"));
}
