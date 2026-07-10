use vela_analysis::facts::AnalysisFacts;
use vela_common::{PrimitiveTag, ScalarValue, SourceId, Span};
use vela_def::{FunctionId, MethodId, TypeId};
use vela_hir::ids::{HirBodyId, HirCaptureId, HirExprId, HirLocalId, HirNodeId, HirParamId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

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

fn script_method_descriptor(
    executable: MethodExecutableTarget,
    owner: TypeId,
) -> CompileMethodDescriptor {
    CompileMethodDescriptor {
        id: executable.method,
        owner,
        member_name: "main".to_owned(),
        debug_name: "Player::main".to_owned(),
        class: CompileMethodClass::Script {
            executable,
            owner_name: "game::Player".to_owned(),
            code_symbol: "game::__impl.Player.main".to_owned(),
        },
        signature: CompileSignature {
            parameters: Vec::new(),
            positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
            return_contract: None,
            effect: MirEffect::PURE,
        },
        access: CompileMethodAccess::script(),
    }
}

#[test]
fn mir_model_preserves_stable_function_method_and_body_mappings() {
    let body = HirBodyId::new(3);
    let origin = origin(body);
    let function_id = FunctionId::new(30);
    let method = MethodExecutableTarget {
        method: MethodId::new(31),
        function: FunctionId::new(32),
        owner: TypeId::new(34),
        node: HirNodeId::new(33),
    };
    let mut program = MirProgram::new(MirTargetTable::default());
    let function = program
        .add_function(test_function(
            body,
            MirFunctionOwner::Function(function_id),
            origin,
        ))
        .expect("script function should be inserted");
    let method_function = program
        .add_function(test_function(
            body,
            MirFunctionOwner::Method(method),
            origin,
        ))
        .expect("a method instantiation may share its HIR body");
    let second_method = MethodExecutableTarget {
        method: method.method,
        function: FunctionId::new(35),
        owner: TypeId::new(36),
        node: HirNodeId::new(37),
    };
    let second_method_function = program
        .add_function(test_function(
            body,
            MirFunctionOwner::Method(second_method),
            origin,
        ))
        .expect("trait MethodId may be shared by a distinct receiver owner");
    let lambda = program
        .add_function(test_function(
            HirBodyId::new(4),
            MirFunctionOwner::Lambda {
                parent: function,
                expression: HirExprId::new(40),
            },
            origin,
        ))
        .expect("nested lambda should reference its generation-local parent");

    assert_eq!(program.function_by_id(function_id), Some(function));
    let stored = program.function(function).expect("stored function");
    assert_eq!(stored.body(), body);
    assert_eq!(stored.code_symbol(), "test::body_3");
    assert!(matches!(
        stored.owner(),
        MirFunctionOwner::Function(actual) if *actual == function_id
    ));
    assert_eq!(
        program.function_by_id(method.function),
        Some(method_function)
    );
    assert_eq!(
        program.method_by_id(method.owner, method.method),
        Some(method_function)
    );
    assert_eq!(
        program.method_by_id(second_method.owner, second_method.method),
        Some(second_method_function)
    );
    assert_eq!(
        program.functions_for_body(body),
        &[function, method_function, second_method_function]
    );
    assert!(program.function(lambda).is_some());
    assert!(matches!(
        program.add_function(test_function(
            HirBodyId::new(5),
            MirFunctionOwner::Function(function_id),
            origin,
        )),
        Err(MirBuildError::DuplicateMirFunctionId { .. })
    ));
}

#[test]
fn mir_model_preserves_ordered_function_abi_metadata() {
    let body = HirBodyId::new(27);
    let origin = origin(body);
    let method = MethodExecutableTarget {
        method: MethodId::new(271),
        function: FunctionId::new(272),
        owner: TypeId::new(273),
        node: HirNodeId::new(274),
    };
    let mut function = MirFunction::new(
        body,
        MirFunctionOwner::Method(method),
        "game::__impl.Player.level_up",
        Some(MirFunctionReturn {
            contract: MirTypeContract::Primitive(PrimitiveTag::I64),
            origin,
        }),
        origin,
    );
    let receiver = function.add_parameter(MirParameterSpec {
        kind: MirParameterKind::Receiver,
        hir_local: HirLocalId::new(275),
        name: "self".to_owned(),
        value_type: MirValueType::ScriptType {
            type_id: method.owner,
            shape: vela_common::ShapeId::new(276),
        },
        contract: Some(MirTypeContract::Definition(method.owner)),
        default_body: None,
        origin,
    });
    let amount = function.add_parameter(MirParameterSpec {
        kind: MirParameterKind::Explicit(HirParamId::new(277)),
        hir_local: HirLocalId::new(278),
        name: "amount".to_owned(),
        value_type: MirValueType::Primitive(PrimitiveTag::I64),
        contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
        default_body: Some(HirBodyId::new(279)),
        origin,
    });
    let capture = function.add_capture(
        HirCaptureId::new(280),
        HirLocalId::new(281),
        "bonus",
        MirValueType::Primitive(PrimitiveTag::I64),
        origin,
    );

    assert_eq!(function.code_symbol(), "game::__impl.Player.level_up");
    assert_eq!(
        function.return_contract().map(|value| &value.contract),
        Some(&MirTypeContract::Primitive(PrimitiveTag::I64))
    );
    assert_eq!(
        function
            .parameters()
            .iter()
            .map(|parameter| (parameter.storage, parameter.name.as_str()))
            .collect::<Vec<_>>(),
        [(receiver, "self"), (amount, "amount")]
    );
    assert_eq!(
        function.parameters()[1].default_body,
        Some(HirBodyId::new(279))
    );
    assert_eq!(function.captures()[0].storage, capture);
    assert_eq!(function.captures()[0].capture, HirCaptureId::new(280));
}

#[test]
fn mir_model_lowering_input_requires_an_exact_owned_compile_target() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(9),
        ModulePath::from_qualified("game"),
        "fn main(value: i64) -> i64 { return value; }",
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let hir_body = graph.function_body(declaration.id).expect("main HIR body");
    let body = hir_body.id;
    let body_origin = MirSourceOrigin::body(body, hir_body.origin.span);
    let analysis = AnalysisFacts::from_module_graph(&graph);
    let function = FunctionId::new(90);
    let mut non_script_targets = CompileTargetSnapshot::builder();
    non_script_targets
        .insert_function(
            body,
            CompileFunctionIdentity::Function(function),
            body_origin,
        )
        .expect("non-script fixture target should be unique");
    non_script_targets
        .insert_function_descriptor(
            CompileFunctionDescriptor {
                id: function,
                class: CompileFunctionClass::Native,
                canonical_symbol: "game::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
                    parameters: Vec::new(),
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::new(true, true, false),
            },
            body_origin,
        )
        .expect("non-script fixture descriptor should be unique");
    let non_script_targets = non_script_targets.build();
    assert!(matches!(
        MirLoweringInput::new(
            &graph,
            function,
            body,
            &analysis,
            &non_script_targets,
            MirLoweringConfig::default(),
        ),
        Err(MirBuildError::InconsistentInput { message, .. })
            if message.contains("not classified as a script function")
    ));
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_function(
            body,
            CompileFunctionIdentity::Function(function),
            body_origin,
        )
        .expect("function target should be unique");
    targets
        .insert_function_descriptor(
            CompileFunctionDescriptor {
                id: function,
                class: CompileFunctionClass::Script,
                canonical_symbol: "game::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
                    parameters: vec![CompileParameter {
                        name: "value".to_owned(),
                        contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                        default: CompileParameterDefault::Required,
                        origin: Some(body_origin),
                    }],
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::script(true),
            },
            body_origin,
        )
        .expect("function descriptor should be unique");
    let method = MethodExecutableTarget {
        method: MethodId::new(92),
        function: FunctionId::new(93),
        owner: TypeId::new(95),
        node: HirNodeId::new(94),
    };
    targets
        .insert_function(body, CompileFunctionIdentity::Method(method), body_origin)
        .expect("a method instantiation may share its source HIR body");
    targets
        .insert_function_descriptor(
            CompileFunctionDescriptor {
                id: method.function,
                class: CompileFunctionClass::Script,
                canonical_symbol: "game::__impl.Player.main".to_owned(),
                debug_name: "Player::main".to_owned(),
                signature: CompileSignature {
                    parameters: Vec::new(),
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::script(true),
            },
            body_origin,
        )
        .expect("method code descriptor should be unique");
    let missing_method_targets = targets.clone().build();
    assert!(matches!(
        MirLoweringInput::new(
            &graph,
            method.function,
            body,
            &analysis,
            &missing_method_targets,
            MirLoweringConfig::default(),
        ),
        Err(MirBuildError::InconsistentInput { message, .. })
            if message.contains("missing method descriptor")
    ));
    let mut wrong_owner_targets = targets.clone();
    wrong_owner_targets
        .insert_method_descriptor(
            script_method_descriptor(method, TypeId::new(96)),
            body_origin,
        )
        .expect("mismatched fixture descriptor should be unique");
    let wrong_owner_targets = wrong_owner_targets.build();
    assert!(matches!(
        MirLoweringInput::new(
            &graph,
            method.function,
            body,
            &analysis,
            &wrong_owner_targets,
            MirLoweringConfig::default(),
        ),
        Err(MirBuildError::InconsistentInput { message, .. })
            if message.contains("missing method descriptor")
    ));
    let mut wrong_executable_targets = targets.clone();
    wrong_executable_targets
        .insert_method_descriptor(
            script_method_descriptor(
                MethodExecutableTarget {
                    node: HirNodeId::new(97),
                    ..method
                },
                method.owner,
            ),
            body_origin,
        )
        .expect("mismatched fixture descriptor should be unique");
    let wrong_executable_targets = wrong_executable_targets.build();
    assert!(matches!(
        MirLoweringInput::new(
            &graph,
            method.function,
            body,
            &analysis,
            &wrong_executable_targets,
            MirLoweringConfig::default(),
        ),
        Err(MirBuildError::InconsistentInput { message, .. })
            if message.contains("executable does not match")
    ));
    targets
        .insert_method_descriptor(script_method_descriptor(method, method.owner), body_origin)
        .expect("method descriptor should be unique");
    let targets = targets.build();

    let input = MirLoweringInput::new(
        &graph,
        function,
        body,
        &analysis,
        &targets,
        MirLoweringConfig::default(),
    )
    .expect("matching immutable lowering input should be accepted");
    assert_eq!(input.function(), function);
    assert_eq!(input.body(), body);
    let method_input = MirLoweringInput::new(
        &graph,
        method.function,
        body,
        &analysis,
        &targets,
        MirLoweringConfig::default(),
    )
    .expect("matching method lowering input should be accepted");
    assert_eq!(method_input.function(), method.function);
    assert_eq!(
        targets.functions_for_body(body),
        &[function, method.function]
    );
    let error = MirLoweringInput::new(
        &graph,
        FunctionId::new(91),
        body,
        &analysis,
        &targets,
        MirLoweringConfig::default(),
    )
    .map(|_| ())
    .expect_err("missing stable function target should fail");
    assert!(matches!(
        error,
        MirBuildError::MissingFunctionTarget { function, .. }
            if function == FunctionId::new(91)
    ));
}

#[test]
fn mir_model_dump_is_stable_and_human_readable() {
    let body = HirBodyId::new(6);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(60)),
        origin,
    );
    let local = function.add_script_local(
        HirLocalId::new(2),
        MirValueType::Primitive(PrimitiveTag::I64),
        origin,
    );
    let entry = function.entry_block();
    function
        .append_statement(
            entry,
            MirStatement::assign(
                origin,
                MirPlace::local(local),
                MirRvalue::Use(MirOperand::Immediate(MirImmediate::Scalar(
                    ScalarValue::I64(4),
                ))),
            ),
        )
        .expect("assignment should be inserted");
    function
        .set_terminator(
            entry,
            MirTerminator::new(
                origin,
                MirTerminatorKind::Return(Some(MirOperand::Local(local))),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return should terminate the entry block");
    let mut program = MirProgram::new(MirTargetTable::default());
    program
        .add_function(function)
        .expect("function should be inserted");

    assert_eq!(
        program.dump(),
        "mir {\n  fn f0 body h6 owner function#60 symbol=\"test::body_6\" @7:0..5/h6 {\n    local l0: Script(HirLocalId(2)) Primitive(I64) @7:0..5/h6\n    bb0:\n      s0: l0 = 4i64 [pure] @7:0..5/h6\n      -> return l0 [pure] @7:0..5/h6\n  }\n}\n"
    );
}
