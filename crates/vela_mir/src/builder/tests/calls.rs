use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{PrimitiveTag, ScalarValue, SourceId};
use vela_def::{FunctionId, MethodId, TypeId};
use vela_hir::body::{HirBody, HirExprKind};
use vela_hir::ids::{HirExprId, HirNodeId};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

use crate::{
    CompileCallTarget, CompileCalleeTarget, CompileDynamicCallArgument, CompileFunctionAccess,
    CompileFunctionClass, CompileFunctionDescriptor, CompileFunctionIdentity, CompileGuardKey,
    CompileGuardTarget, CompileMethodAccess, CompileMethodClass, CompileMethodDescriptor,
    CompileParameter, CompileParameterDefault, CompilePlacedCallArgument, CompilePositionalPolicy,
    CompileSignature, CompileTargetSnapshot, CompileTargetSnapshotBuilder, CompileTypeClass,
    CompileTypeDescriptor, DynamicMethodTarget, MethodExecutableTarget, MirBuildError, MirCall,
    MirEffect, MirEvaluatedConstant, MirGuardLocation, MirLoweringConfig, MirLoweringInput,
    MirProgram, MirScriptParameterGuardMode, MirSourceOrigin, MirStatementKind, MirTypeContract,
};

const ROOT_FUNCTION: FunctionId = FunctionId::new(700);

fn try_build_calls(
    source: &str,
    configure: impl FnOnce(
        &ModuleGraph,
        &HirBody,
        &mut CompileTargetSnapshotBuilder,
    ) -> Result<(), MirBuildError>,
) -> Result<MirProgram, MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(73),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("calls"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main body");
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(ROOT_FUNCTION, body.id)],
    )
    .expect("call builder analysis");

    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        declaration.id,
        body.id,
        function_descriptor(
            ROOT_FUNCTION,
            CompileFunctionClass::Script,
            "calls::main",
            body_parameters(&graph, body),
        ),
        origin,
    )?;
    configure(&graph, body, &mut targets)?;
    let targets = targets.build()?;
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(ROOT_FUNCTION),
        body.id,
        analysis.view(ROOT_FUNCTION).expect("main analysis"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )?;
    crate::build_mir(input)
}

fn function_descriptor(
    id: FunctionId,
    class: CompileFunctionClass,
    symbol: &str,
    parameters: Vec<CompileParameter>,
) -> CompileFunctionDescriptor {
    CompileFunctionDescriptor {
        id,
        class,
        canonical_symbol: symbol.to_owned(),
        debug_name: symbol.rsplit("::").next().unwrap_or(symbol).to_owned(),
        signature: CompileSignature {
            asyncness: vela_common::CallableAsyncness::Sync,
            parameters,
            positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
            return_contract: None,
            effect: MirEffect::PURE,
        },
        access: if class == CompileFunctionClass::Script {
            CompileFunctionAccess::script(false)
        } else {
            CompileFunctionAccess::new(true, true, false)
        },
    }
}

fn body_parameters(graph: &ModuleGraph, body: &HirBody) -> Vec<CompileParameter> {
    let bindings = graph
        .bindings_for_body(body.id)
        .expect("body binding generation");
    body.params
        .iter()
        .map(|parameter| CompileParameter {
            name: bindings
                .local(parameter.local)
                .expect("parameter binding")
                .name
                .clone(),
            contract: None,
            default: parameter.default_body.map_or(
                CompileParameterDefault::Required,
                CompileParameterDefault::HirBody,
            ),
            origin: None,
        })
        .collect()
}

fn only_call(body: &HirBody) -> (HirExprId, vela_hir::body::HirCall) {
    let calls = body
        .expressions
        .values()
        .filter_map(|expression| match &expression.kind {
            HirExprKind::Call(call) => Some((expression.id, call.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("expected exactly one call, got {calls:?}");
    };
    call.clone()
}

fn argument_values(call: &vela_hir::body::HirCall) -> Vec<HirExprId> {
    call.arguments
        .iter()
        .map(|argument| argument.value.expect("valid call argument"))
        .collect()
}

#[test]
fn source_literal_definition_precedes_a_later_effectful_argument() {
    const EFFECT: FunctionId = FunctionId::new(705);
    const SINK: FunctionId = FunctionId::new(706);

    let source = "fn main() { return sink(1, effect()); }";
    let program = try_build_calls(source, |_graph, body, targets| {
        let mut calls = body
            .expressions
            .values()
            .filter_map(|expression| match &expression.kind {
                HirExprKind::Call(call) => Some((expression.id, call.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        calls.sort_by_key(|(expression, _)| {
            body.expression(*expression)
                .expect("call expression")
                .origin
                .span
                .start
        });
        let [
            (outer_expression, outer_call),
            (effect_expression, effect_call),
        ] = calls.as_slice()
        else {
            panic!("expected outer and nested calls: {calls:?}")
        };
        let effect_origin = MirSourceOrigin::expression(
            body.id,
            *effect_expression,
            call_origin(body, *effect_expression),
        );
        let outer_origin = MirSourceOrigin::expression(
            body.id,
            *outer_expression,
            call_origin(body, *outer_expression),
        );
        let mut effect_descriptor = function_descriptor(
            EFFECT,
            CompileFunctionClass::Native,
            "calls::effect",
            Vec::new(),
        );
        effect_descriptor.signature.effect = MirEffect {
            reads_time: true,
            ..MirEffect::PURE
        };
        targets.insert_function_descriptor(effect_descriptor, effect_origin)?;
        targets.insert_function_descriptor(
            function_descriptor(
                SINK,
                CompileFunctionClass::Native,
                "calls::sink",
                vec![required("first"), required("second")],
            ),
            outer_origin,
        )?;
        targets.insert_call(
            ROOT_FUNCTION,
            *effect_expression,
            CompileCallTarget::positional(
                CompileCalleeTarget::NativeFunction {
                    function: EFFECT,
                    debug_name: "calls::effect".to_owned(),
                },
                argument_values(effect_call),
            ),
            effect_origin,
        )?;
        targets.insert_call(
            ROOT_FUNCTION,
            *outer_expression,
            CompileCallTarget::positional(
                CompileCalleeTarget::NativeFunction {
                    function: SINK,
                    debug_name: "calls::sink".to_owned(),
                },
                argument_values(outer_call),
            ),
            outer_origin,
        )
    })
    .expect("literal followed by nested effectful call");

    let (_, function) = program.functions().next().expect("main function");
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let literal_position = statements
        .iter()
        .position(|statement| {
            matches!(
                statement.kind,
                MirStatementKind::Assign(crate::MirRvalue::Constant {
                    value: crate::MirImmediate::Scalar(ScalarValue::I64(1)),
                    provenance: crate::MirConstantProvenance::Literal,
                })
            )
        })
        .expect("literal definition");
    let effect_position = statements
        .iter()
        .position(|statement| {
            matches!(
                statement.kind,
                MirStatementKind::Call(MirCall::NativeFunction {
                    function: EFFECT,
                    ..
                })
            )
        })
        .expect("effectful nested call");
    let sink_position = statements
        .iter()
        .position(|statement| {
            matches!(
                statement.kind,
                MirStatementKind::Call(MirCall::NativeFunction { function: SINK, .. })
            )
        })
        .expect("outer call");
    assert!(
        literal_position < effect_position && effect_position < sink_position,
        "{}",
        program.dump()
    );
}

#[test]
fn mir_builder_lowers_named_script_arguments_after_source_evaluation() {
    let source = r#"
fn target(first, second, third = 3) {}
fn main() { return target(second = "second", first = "first"); }
"#;
    let program = try_build_calls(source, |graph, body, targets| {
        let target_declaration = graph
            .declarations()
            .find(|declaration| declaration.name == "target")
            .expect("target declaration");
        let target_body = graph
            .function_body(target_declaration.id)
            .expect("target body");
        let target_function = FunctionId::new(701);
        let target_origin = MirSourceOrigin::body(target_body.id, target_body.origin.span);
        targets.insert_script_function_descriptor(
            target_declaration.id,
            function_descriptor(
                target_function,
                CompileFunctionClass::Script,
                "calls::target",
                body_parameters(graph, target_body),
            ),
            target_origin,
        )?;

        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::script(
                CompileCalleeTarget::ScriptFunction {
                    function: target_function,
                    debug_name: "calls::target".to_owned(),
                },
                values.clone(),
                vec![
                    CompilePlacedCallArgument::placed(0, 1, values[1]),
                    CompilePlacedCallArgument::placed(1, 0, values[0]),
                    CompilePlacedCallArgument::missing(2),
                ],
            ),
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression)),
        )
    })
    .expect("named script call should lower");

    let dump = program.dump();
    let second = dump.find("const.materialize \"second\"").expect("second");
    let first = dump.find("const.materialize \"first\"").expect("first");
    let call = dump.find("call script#701").expect("script call");
    assert!(second < first && first < call, "{dump}");
    assert!(dump.contains("(p0=t1, p1=t0, p2=<missing>)"), "{dump}");
    assert!(dump.contains("parameter_guards=ProvenAtCallSite"), "{dump}");
}

#[test]
fn mir_builder_captures_dynamic_method_receiver_before_named_arguments() {
    let source = r#"
fn main(receiver, first, second) {
    return receiver.invoke(second, label = first);
}
"#;
    let program = try_build_calls(source, |_graph, body, targets| {
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::dynamic(
                CompileCalleeTarget::DynamicMethod(DynamicMethodTarget::method(
                    "invoke",
                    1,
                    vec!["label".to_owned()],
                )),
                vec![
                    CompileDynamicCallArgument {
                        name: None,
                        value: values[0],
                    },
                    CompileDynamicCallArgument {
                        name: Some("label".to_owned()),
                        value: values[1],
                    },
                ],
            ),
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression)),
        )
    })
    .expect("dynamic method call should lower");

    let dump = program.dump();
    let receiver = dump.find("t0 = l0").expect("captured receiver");
    let positional = dump.find("t1 = l2").expect("captured positional arg");
    let named = dump.find("t2 = l1").expect("captured named arg");
    let call = dump
        .find("call dynamic-method invoke")
        .expect("dynamic call");
    assert!(
        receiver < positional && positional < named && named < call,
        "{dump}"
    );
    assert!(dump.contains("receiver=t0(t1, label=t2)"), "{dump}");
    assert_eq!(
        complete_function_dump(&program),
        r#"  fn f0 body h0 owner function#700 symbol="calls::main" @73:34..88/h0 {
    param p0: receiver -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @73:9..17/h0
    param p1: first -> l1 kind=Explicit(HirParamId(1)) contract=None default=None hir=l1 @73:19..24/h0
    param p2: second -> l2 kind=Explicit(HirParamId(2)) contract=None default=None hir=l2 @73:26..32/h0
    local l0: Script(HirLocalId(0)) Dynamic @73:9..17/h0
    local l1: Script(HirLocalId(1)) Dynamic @73:19..24/h0
    local l2: Script(HirLocalId(2)) Dynamic @73:26..32/h0
    temp t0: Dynamic def=s0 @73:47..55/e2
    temp t1: Dynamic def=s1 @73:63..69/e3
    temp t2: Dynamic def=s2 @73:79..84/e4
    temp t3: Dynamic def=s3 @73:47..85/e0
    debug dl0: receiver -> l0 kind=Parameter hir=Some(0) scope=h0 live=[] @73:9..17/h0
    debug dl1: first -> l1 kind=Parameter hir=Some(1) scope=h0 live=[] @73:19..24/h0
    debug dl2: second -> l2 kind=Parameter hir=Some(2) scope=h0 live=[] @73:26..32/h0
    safepoint sp0: live={} @73:47..85/e0
    bb0:
      s0: t0 = l0 [pure] @73:47..55/e2
      s1: t1 = l2 [pure] @73:63..69/e3
      s2: t2 = l1 [pure] @73:79..84/e4
      s3: t3 = call dynamic-method invoke receiver=t0(t1, label=t2) [trap|alloc|dynamic-call, sp0] @73:47..85/e0
      -> return t3 [pure] @73:40..86/s0
  }
}
"#
    );
}

#[test]
fn mir_builder_preserves_dynamic_callable_names_after_callee_evaluation() {
    let source = r#"
fn main(choose, left, right, value) {
    return (if choose { left } else { right })(label = value);
}
"#;
    let program = try_build_calls(source, |_graph, body, targets| {
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::dynamic(
                CompileCalleeTarget::DynamicCallable,
                vec![CompileDynamicCallArgument {
                    name: Some("label".to_owned()),
                    value: values[0],
                }],
            ),
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression)),
        )
    })
    .expect("dynamic callable should lower");

    let dump = program.dump();
    assert!(dump.contains("call dynamic("), "{dump}");
    assert!(dump.contains("(label="), "{dump}");
    assert!(dump.contains("branch"), "{dump}");
}

#[test]
fn mir_builder_lowers_native_named_arguments_from_stable_descriptor() {
    let source = "fn main(first, second) { return external(second = second, first = first); }";
    let program = try_build_calls(source, |_graph, body, targets| {
        let external = FunctionId::new(702);
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        targets.insert_function_descriptor(
            function_descriptor(
                external,
                CompileFunctionClass::Native,
                "host::external",
                vec![required("first"), required("second")],
            ),
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression)),
        )?;
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::external_named(
                CompileCalleeTarget::NativeFunction {
                    function: external,
                    debug_name: "host::external".to_owned(),
                },
                values.clone(),
                vec![
                    CompilePlacedCallArgument::placed(0, 1, values[1]),
                    CompilePlacedCallArgument::placed(1, 0, values[0]),
                ],
            ),
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression)),
        )
    })
    .expect("native descriptor call should lower");

    let dump = program.dump();
    assert!(
        dump.contains("call native#702 name=\"host::external\""),
        "{dump}"
    );
    assert!(dump.contains("(t1, t0)"), "{dump}");
}

#[test]
fn native_argument_guard_traps_before_a_later_allocating_argument() {
    let source = r#"fn main(first) { return external(first, "later"); }"#;
    let program = try_build_calls(source, |_graph, body, targets| {
        let external = FunctionId::new(703);
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        let origin =
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression));
        targets.insert_function_descriptor(
            function_descriptor(
                external,
                CompileFunctionClass::Native,
                "host::external",
                vec![required("first"), required("second")],
            ),
            origin,
        )?;
        targets.insert_guard(
            CompileGuardKey::Expression {
                function: ROOT_FUNCTION,
                expression: values[0],
            },
            CompileGuardTarget::new(
                MirTypeContract::Primitive(PrimitiveTag::I64),
                MirGuardLocation::Parameter { index: 0 },
                "first",
            ),
            MirSourceOrigin::expression(body.id, values[0], call_origin(body, values[0])),
        )?;
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::positional(
                CompileCalleeTarget::NativeFunction {
                    function: external,
                    debug_name: "host::external".to_owned(),
                },
                values,
            ),
            origin,
        )
    })
    .expect("guarded native call");
    let (_, function) = program.functions().next().expect("main function");
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let guard = statements
        .iter()
        .position(|statement| matches!(statement.kind, MirStatementKind::GuardTrap { .. }))
        .expect("argument guard");
    let later = statements
        .iter()
        .position(|statement| {
            matches!(
                &statement.kind,
                MirStatementKind::MaterializeConstant(MirEvaluatedConstant::String(value))
                    if value == "later"
            )
        })
        .expect("later allocation");
    let call = statements
        .iter()
        .position(|statement| {
            matches!(
                statement.kind,
                MirStatementKind::Call(MirCall::NativeFunction { .. })
            )
        })
        .expect("native call");
    assert!(guard < later && later < call, "{}", program.dump());
    assert_eq!(function.guards().count(), 1);
}

#[test]
fn guarded_script_argument_keeps_checked_callee_parameter_policy() {
    let source = "fn target(value: i64) {} fn main(value) { return target(value); }";
    let program = try_build_calls(source, |graph, body, targets| {
        let target_declaration = graph
            .declarations()
            .find(|declaration| declaration.name == "target")
            .expect("target declaration");
        let target_body = graph
            .function_body(target_declaration.id)
            .expect("target body");
        let target_function = FunctionId::new(704);
        let target_origin = MirSourceOrigin::body(target_body.id, target_body.origin.span);
        let parameter = CompileParameter {
            name: "value".to_owned(),
            contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
            default: CompileParameterDefault::Required,
            origin: Some(MirSourceOrigin::body(
                target_body.id,
                target_body.params[0].origin.span,
            )),
        };
        targets.insert_script_function_descriptor(
            target_declaration.id,
            function_descriptor(
                target_function,
                CompileFunctionClass::Script,
                "calls::target",
                vec![parameter.clone()],
            ),
            target_origin,
        )?;
        targets.insert_guard(
            CompileGuardKey::Parameter {
                function: target_function,
                parameter: 0,
            },
            CompileGuardTarget::new(
                parameter.contract.clone().expect("parameter contract"),
                MirGuardLocation::Parameter { index: 0 },
                "value",
            ),
            parameter.origin.expect("parameter origin"),
        )?;
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        let origin =
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression));
        targets.insert_guard(
            CompileGuardKey::Expression {
                function: ROOT_FUNCTION,
                expression: values[0],
            },
            CompileGuardTarget::new(
                MirTypeContract::Primitive(PrimitiveTag::I64),
                MirGuardLocation::Parameter { index: 0 },
                "value",
            ),
            MirSourceOrigin::expression(body.id, values[0], call_origin(body, values[0])),
        )?;
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::script(
                CompileCalleeTarget::ScriptFunction {
                    function: target_function,
                    debug_name: "calls::target".to_owned(),
                },
                values.clone(),
                vec![CompilePlacedCallArgument::placed(0, 0, values[0])],
            ),
            origin,
        )
    })
    .expect("guarded script call");
    let (_, function) = program.functions().next().expect("main function");
    assert_eq!(function.guards().count(), 0);
    assert!(function.statements().any(|(_, statement)| matches!(
        statement.kind,
        MirStatementKind::Call(MirCall::ScriptFunction {
            parameter_guards: MirScriptParameterGuardMode::CheckCalleeParameterContracts,
            ..
        })
    )));
}

#[test]
fn mir_builder_lowers_script_method_receiver_and_parameter_slots() {
    let source = r#"
fn main(receiver, first, second) {
    return receiver.apply(second = second, first = first);
}
"#;
    let program = try_build_calls(source, |_graph, body, targets| {
        let owner = TypeId::new(710);
        let method = MethodId::new(711);
        let function = FunctionId::new(712);
        let executable = MethodExecutableTarget {
            method,
            function,
            owner,
            node: HirNodeId::new(713),
        };
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        let origin =
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression));
        targets.insert_type_descriptor(
            CompileTypeDescriptor {
                id: owner,
                canonical_name: "calls::Receiver".to_owned(),
                runtime_name: "calls::Receiver".to_owned(),
                class: CompileTypeClass::OpaqueExternal,
                shape: None,
                fields: Vec::new(),
                variants: Vec::new(),
            },
            origin,
        )?;
        let method_parameters = vec![required("first"), required("second")];
        targets.insert_function_descriptor(
            function_descriptor(
                function,
                CompileFunctionClass::Script,
                "calls::Receiver::apply",
                std::iter::once(required("self"))
                    .chain(method_parameters.clone())
                    .collect(),
            ),
            origin,
        )?;
        targets.insert_method_descriptor(
            CompileMethodDescriptor {
                id: method,
                owner,
                member_name: "apply".to_owned(),
                debug_name: "calls::Receiver::apply".to_owned(),
                class: CompileMethodClass::Script {
                    executable,
                    owner_name: "calls::Receiver".to_owned(),
                    code_symbol: "calls::Receiver::apply".to_owned(),
                },
                signature: CompileSignature {
                    asyncness: vela_common::CallableAsyncness::Sync,
                    parameters: method_parameters,
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileMethodAccess::script(),
            },
            origin,
        )?;
        targets.insert_script_method_target(executable, origin)?;
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::script(
                CompileCalleeTarget::ScriptMethod {
                    target: executable,
                    debug_name: "apply".to_owned(),
                },
                values.clone(),
                vec![
                    CompilePlacedCallArgument::placed(0, 1, values[1]),
                    CompilePlacedCallArgument::placed(1, 0, values[0]),
                ],
            ),
            origin,
        )
    })
    .expect("script method call should lower");

    let dump = program.dump();
    assert!(
        dump.contains("call method#711 function#712 owner#710 name=\"apply\" receiver=t0"),
        "{dump}"
    );
    assert!(dump.contains("(p0=t2, p1=t1)"), "{dump}");
}

#[test]
fn mir_builder_lowers_callable_local_with_callee_first() {
    let source = "fn main(callee, argument) { return callee(argument); }";
    let program = try_build_calls(source, |_graph, body, targets| {
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        let callee = body.params[0].local;
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::positional(CompileCalleeTarget::Local(callee), values),
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression)),
        )
    })
    .expect("local callable should lower");

    let dump = program.dump();
    let callee = dump.find("t0 = l0").expect("captured callee");
    let argument = dump.find("t1 = l1").expect("captured argument");
    let call = dump
        .find("call closure(t0)(t1)")
        .expect("callable value call");
    assert!(callee < argument && argument < call, "{dump}");
}

#[test]
fn mir_builder_stops_after_a_terminating_argument_without_appending_call() {
    let source = r#"
fn main(receiver, value) {
    receiver.invoke({ return value; }, "later");
}
"#;
    let program = try_build_calls(source, |_graph, body, targets| {
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::dynamic(
                CompileCalleeTarget::DynamicMethod(DynamicMethodTarget::method(
                    "invoke",
                    2,
                    Vec::new(),
                )),
                values
                    .into_iter()
                    .map(|value| CompileDynamicCallArgument { name: None, value })
                    .collect(),
            ),
            MirSourceOrigin::expression(body.id, expression, call_origin(body, expression)),
        )
    })
    .expect("a diverging argument should finish the function without a call");

    let dump = program.dump();
    assert!(dump.contains("-> return l1"), "{dump}");
    assert!(!dump.contains("const.materialize \"later\""), "{dump}");
    assert!(!dump.contains("call dynamic-method"), "{dump}");
}

#[test]
fn mir_builder_rejects_missing_call_target_as_input_error() {
    let error = try_build_calls(
        "fn main() { return missing(1); }",
        |_graph, _body, _targets| Ok(()),
    )
    .expect_err("missing compile target must be an internal MIR input error");
    assert!(
        error
            .to_string()
            .contains("call expression has no compile target"),
        "{error:?}"
    );
}

fn required(name: &str) -> CompileParameter {
    CompileParameter {
        name: name.to_owned(),
        contract: None,
        default: CompileParameterDefault::Required,
        origin: None,
    }
}

fn complete_function_dump(program: &MirProgram) -> String {
    let dump = program.dump();
    let start = dump.find("  fn ").expect("defined MIR function");
    dump[start..].to_owned()
}

fn call_origin(body: &HirBody, expression: HirExprId) -> vela_common::Span {
    body.expression(expression)
        .expect("call expression record")
        .origin
        .span
}
