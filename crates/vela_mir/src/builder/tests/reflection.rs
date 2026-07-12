use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{PrimitiveTag, SourceId};
use vela_def::FunctionId;
use vela_hir::body::{HirBody, HirExprKind};
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

use crate::{
    CompileCallTarget, CompileCalleeTarget, CompileDynamicCallArgument, CompileFunctionAccess,
    CompileFunctionClass, CompileFunctionDescriptor, CompileFunctionIdentity, CompileGuardKey,
    CompileGuardTarget, CompileParameter, CompileParameterDefault, CompilePlacedCallArgument,
    CompilePositionalPolicy, CompileReflectionCall, CompileSignature, CompileTargetSnapshot,
    CompileTargetSnapshotBuilder, MirBuildError, MirCall, MirEffect, MirEvaluatedConstant,
    MirGuardLocation, MirLoweringConfig, MirLoweringInput, MirOperand, MirReflectionOperation,
    MirSourceOrigin, MirStatementKind, MirTypeContract, MirValueType,
};

const ROOT_FUNCTION: FunctionId = FunctionId::new(9_950);
const READ_FUNCTION: FunctionId = FunctionId::new(9_951);
const WRITE_FUNCTION: FunctionId = FunctionId::new(9_952);
const CALL_FUNCTION: FunctionId = FunctionId::new(9_953);
const OTHER_FUNCTION: FunctionId = FunctionId::new(9_954);

fn try_build_reflection(
    source: &str,
    configure: impl FnOnce(
        &ModuleGraph,
        &HirBody,
        &mut CompileTargetSnapshotBuilder,
    ) -> Result<(), MirBuildError>,
) -> Result<crate::MirProgram, MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(100),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("reflection_builder"),
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
    .expect("reflection analysis");
    let bindings = graph.bindings_for_body(body.id).expect("main bindings");
    let parameters = body
        .params
        .iter()
        .map(|parameter| CompileParameter {
            name: bindings
                .local(parameter.local)
                .expect("parameter binding")
                .name
                .clone(),
            contract: None,
            default: CompileParameterDefault::Required,
            origin: None,
        })
        .collect();
    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        declaration.id,
        body.id,
        function_descriptor(
            ROOT_FUNCTION,
            CompileFunctionClass::Script,
            "reflection_builder::main",
            parameters,
            CompilePositionalPolicy::ExactOrTrailingDefaults,
            MirEffect::PURE,
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
    positional: CompilePositionalPolicy,
    effect: MirEffect,
) -> CompileFunctionDescriptor {
    CompileFunctionDescriptor {
        id,
        class,
        canonical_symbol: symbol.to_owned(),
        debug_name: symbol.to_owned(),
        signature: CompileSignature {
            parameters,
            positional,
            return_contract: None,
            effect,
        },
        access: CompileFunctionAccess::new(true, true, true),
    }
}

fn required(name: &str) -> CompileParameter {
    CompileParameter {
        name: name.to_owned(),
        contract: None,
        default: CompileParameterDefault::Required,
        origin: None,
    }
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
        panic!("expected one call, got {calls:?}")
    };
    call.clone()
}

fn argument_values(call: &vela_hir::body::HirCall) -> Vec<HirExprId> {
    call.arguments
        .iter()
        .map(|argument| argument.value.expect("call argument"))
        .collect()
}

fn expression_origin(body: &HirBody, expression: HirExprId) -> MirSourceOrigin {
    MirSourceOrigin::expression(
        body.id,
        expression,
        body.expression(expression).expect("expression").origin.span,
    )
}

// Keep every descriptor field visible at call sites: these tests deliberately vary
// each one independently to exercise reflection-target validation.
#[allow(clippy::too_many_arguments)]
fn insert_reflection_descriptor(
    targets: &mut CompileTargetSnapshotBuilder,
    body: &HirBody,
    expression: HirExprId,
    function: FunctionId,
    name: &str,
    parameters: Vec<CompileParameter>,
    positional: CompilePositionalPolicy,
    effect: MirEffect,
) -> Result<(), MirBuildError> {
    targets.insert_function_descriptor(
        function_descriptor(
            function,
            CompileFunctionClass::Native,
            name,
            parameters,
            positional,
            effect,
        ),
        expression_origin(body, expression),
    )
}

#[test]
fn named_reflection_read_evaluates_in_source_order_then_projects_descriptor_order() {
    let source = r#"fn main(value) { return reflect::get(field = "name", target = value); }"#;
    let program = try_build_reflection(source, |_graph, body, targets| {
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        let descriptor_effect = MirEffect {
            reads_time: true,
            ..MirEffect::PURE
        };
        insert_reflection_descriptor(
            targets,
            body,
            expression,
            READ_FUNCTION,
            "reflect::get",
            vec![required("target"), required("field")],
            CompilePositionalPolicy::ExactOrTrailingDefaults,
            descriptor_effect,
        )?;
        targets.insert_guard(
            CompileGuardKey::Expression {
                function: ROOT_FUNCTION,
                expression: values[1],
            },
            CompileGuardTarget::new(
                MirTypeContract::Primitive(PrimitiveTag::I64),
                MirGuardLocation::Parameter { index: 0 },
                "target",
            ),
            expression_origin(body, values[1]),
        )?;
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::external_named(
                CompileCalleeTarget::Reflection {
                    operation: CompileReflectionCall::Read,
                    function: READ_FUNCTION,
                    debug_name: "reflect::get".to_owned(),
                },
                values.clone(),
                vec![
                    CompilePlacedCallArgument::placed(0, 1, values[1]),
                    CompilePlacedCallArgument::placed(1, 0, values[0]),
                ],
            ),
            expression_origin(body, expression),
        )
    })
    .expect("named reflection read");
    let (_, function) = program.functions().next().expect("main function");
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let member_allocation = statements
        .iter()
        .position(|statement| {
            matches!(
                &statement.kind,
                MirStatementKind::MaterializeConstant(MirEvaluatedConstant::String(value))
                    if value == "name"
            )
        })
        .expect("member allocation");
    let target_guard = statements
        .iter()
        .position(|statement| matches!(statement.kind, MirStatementKind::GuardTrap { .. }))
        .expect("target guard");
    let reflection = statements
        .iter()
        .position(|statement| matches!(statement.kind, MirStatementKind::Reflect(_)))
        .expect("reflection read");
    assert!(member_allocation < target_guard && target_guard < reflection);
    let statement = statements[reflection];
    let MirStatementKind::Reflect(MirReflectionOperation::Read {
        function: reflected_function,
        target,
        member,
    }) = &statement.kind
    else {
        panic!("expected reflection read: {statement:?}")
    };
    assert_eq!(*reflected_function, READ_FUNCTION);
    assert_eq!(operand_source(source, function, target), "value");
    assert_eq!(operand_source(source, function, member), "\"name\"");
    assert_eq!(
        statement.effect,
        MirEffect::reflection_read().union(MirEffect {
            reads_time: true,
            ..MirEffect::PURE
        })
    );
    assert!(statement.safepoint.is_some());
    assert_eq!(
        source_text(source, statement.origin),
        "reflect::get(field = \"name\", target = value)"
    );
    let destination = statement.destination.expect("read destination");
    let crate::MirPlace::Temp(destination) = destination else {
        panic!("reflection read destination")
    };
    assert_eq!(
        function.temp(destination).expect("read temp").value_type,
        MirValueType::Dynamic
    );
}

#[test]
fn reflection_write_guards_each_operand_before_later_effects_and_unions_effects() {
    let source = r#"fn main(target, member) { return reflect::set(target, member, "later"); }"#;
    let program = try_build_reflection(source, |_graph, body, targets| {
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        insert_reflection_descriptor(
            targets,
            body,
            expression,
            WRITE_FUNCTION,
            "reflect::set",
            vec![required("target"), required("field"), required("value")],
            CompilePositionalPolicy::ExactOrTrailingDefaults,
            MirEffect::host_write(),
        )?;
        for (index, value) in values[..2].iter().copied().enumerate() {
            targets.insert_guard(
                CompileGuardKey::Expression {
                    function: ROOT_FUNCTION,
                    expression: value,
                },
                CompileGuardTarget::new(
                    MirTypeContract::Primitive(PrimitiveTag::I64),
                    MirGuardLocation::Parameter {
                        index: u32::try_from(index).expect("guard index"),
                    },
                    format!("argument-{index}"),
                ),
                expression_origin(body, value),
            )?;
        }
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::positional(
                CompileCalleeTarget::Reflection {
                    operation: CompileReflectionCall::Write,
                    function: WRITE_FUNCTION,
                    debug_name: "reflect::set".to_owned(),
                },
                values,
            ),
            expression_origin(body, expression),
        )
    })
    .expect("reflection write");
    let (_, function) = program.functions().next().expect("main function");
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let guards = statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            matches!(statement.kind, MirStatementKind::GuardTrap { .. }).then_some(index)
        })
        .collect::<Vec<_>>();
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
    let reflection = statements
        .iter()
        .position(|statement| matches!(statement.kind, MirStatementKind::Reflect(_)))
        .expect("reflection write");
    assert_eq!(guards.len(), 2);
    assert!(guards[0] < guards[1] && guards[1] < later && later < reflection);
    let statement = statements[reflection];
    let MirStatementKind::Reflect(MirReflectionOperation::Write {
        function: reflected_function,
        target,
        member,
        value,
    }) = &statement.kind
    else {
        panic!("expected reflection write: {statement:?}")
    };
    assert_eq!(*reflected_function, WRITE_FUNCTION);
    assert_eq!(operand_source(source, function, target), "target");
    assert_eq!(operand_source(source, function, member), "member");
    assert_eq!(operand_source(source, function, value), "\"later\"");
    assert_eq!(
        statement.effect,
        MirEffect::reflection_write().union(MirEffect::host_write())
    );
    assert!(statement.safepoint.is_some());
}

#[test]
fn reflection_call_preserves_the_complete_evaluated_tail() {
    let source = r#"fn main(target) { return reflect::call(target, "method", "argument"); }"#;
    let program = try_build_reflection(source, |_graph, body, targets| {
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        let descriptor_effect = MirEffect {
            emits_event: true,
            ..MirEffect::PURE
        };
        insert_reflection_descriptor(
            targets,
            body,
            expression,
            CALL_FUNCTION,
            "reflect::call",
            vec![required("target")],
            CompilePositionalPolicy::RuntimeChecked,
            descriptor_effect,
        )?;
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::positional(
                CompileCalleeTarget::Reflection {
                    operation: CompileReflectionCall::Call,
                    function: CALL_FUNCTION,
                    debug_name: "reflect::call".to_owned(),
                },
                values,
            ),
            expression_origin(body, expression),
        )
    })
    .expect("reflection call tail");
    let (_, function) = program.functions().next().expect("main function");
    let statement = function
        .statements()
        .find_map(|(_, statement)| {
            matches!(statement.kind, MirStatementKind::Reflect(_)).then_some(statement)
        })
        .expect("reflection call");
    let MirStatementKind::Reflect(MirReflectionOperation::Call {
        function: reflected_function,
        target,
        tail,
    }) = &statement.kind
    else {
        panic!("expected reflection call: {statement:?}")
    };
    assert_eq!(*reflected_function, CALL_FUNCTION);
    assert_eq!(operand_source(source, function, target), "target");
    assert_eq!(tail.len(), 2);
    assert_eq!(operand_source(source, function, &tail[0]), "\"method\"");
    assert_eq!(operand_source(source, function, &tail[1]), "\"argument\"");
    assert_eq!(
        statement.effect,
        MirEffect::reflection_call().union(MirEffect {
            emits_event: true,
            ..MirEffect::PURE
        })
    );
    assert!(statement.safepoint.is_some());
    assert_eq!(
        function
            .safepoint(statement.safepoint.expect("call safepoint"))
            .expect("call safepoint record")
            .origin,
        statement.origin
    );
}

#[test]
fn malformed_reflection_arity_name_and_placement_fail_without_native_fallback() {
    let wrong_arity = try_build_reflection(
        "fn main(target) { return reflect::get(target); }",
        |_graph, body, targets| {
            let (expression, call) = only_call(body);
            let values = argument_values(&call);
            insert_reflection_descriptor(
                targets,
                body,
                expression,
                READ_FUNCTION,
                "reflect::get",
                vec![required("target")],
                CompilePositionalPolicy::RuntimeChecked,
                MirEffect::PURE,
            )?;
            targets.insert_call(
                ROOT_FUNCTION,
                expression,
                CompileCallTarget::positional(
                    CompileCalleeTarget::Reflection {
                        operation: CompileReflectionCall::Read,
                        function: READ_FUNCTION,
                        debug_name: "reflect::get".to_owned(),
                    },
                    values,
                ),
                expression_origin(body, expression),
            )
        },
    )
    .expect_err("reflection read arity");
    assert!(
        wrong_arity
            .to_string()
            .contains("reflection read requires exactly two"),
        "{wrong_arity:?}"
    );

    let wrong_write_arity = try_build_reflection(
        "fn main(target, field) { return reflect::set(target, field); }",
        |_graph, body, targets| {
            let (expression, call) = only_call(body);
            let values = argument_values(&call);
            insert_reflection_descriptor(
                targets,
                body,
                expression,
                WRITE_FUNCTION,
                "reflect::set",
                vec![required("target"), required("field")],
                CompilePositionalPolicy::RuntimeChecked,
                MirEffect::PURE,
            )?;
            targets.insert_call(
                ROOT_FUNCTION,
                expression,
                CompileCallTarget::positional(
                    CompileCalleeTarget::Reflection {
                        operation: CompileReflectionCall::Write,
                        function: WRITE_FUNCTION,
                        debug_name: "reflect::set".to_owned(),
                    },
                    values,
                ),
                expression_origin(body, expression),
            )
        },
    )
    .expect_err("reflection write arity");
    assert!(
        wrong_write_arity
            .to_string()
            .contains("reflection write requires exactly three"),
        "{wrong_write_arity:?}"
    );

    let empty_call = try_build_reflection(
        "fn main() { return reflect::call(); }",
        |_graph, body, targets| {
            let (expression, call) = only_call(body);
            let values = argument_values(&call);
            insert_reflection_descriptor(
                targets,
                body,
                expression,
                CALL_FUNCTION,
                "reflect::call",
                Vec::new(),
                CompilePositionalPolicy::RuntimeChecked,
                MirEffect::PURE,
            )?;
            targets.insert_call(
                ROOT_FUNCTION,
                expression,
                CompileCallTarget::positional(
                    CompileCalleeTarget::Reflection {
                        operation: CompileReflectionCall::Call,
                        function: CALL_FUNCTION,
                        debug_name: "reflect::call".to_owned(),
                    },
                    values,
                ),
                expression_origin(body, expression),
            )
        },
    )
    .expect_err("empty reflection call");
    assert!(
        empty_call
            .to_string()
            .contains("reflection call requires at least one"),
        "{empty_call:?}"
    );

    let wrong_name = try_build_reflection(
        "fn main(target, field) { return reflect::get(target, field); }",
        |_graph, body, targets| {
            let (expression, call) = only_call(body);
            let values = argument_values(&call);
            insert_reflection_descriptor(
                targets,
                body,
                expression,
                WRITE_FUNCTION,
                "reflect::set",
                vec![required("target"), required("field")],
                CompilePositionalPolicy::RuntimeChecked,
                MirEffect::PURE,
            )?;
            targets.insert_call(
                ROOT_FUNCTION,
                expression,
                CompileCallTarget::positional(
                    CompileCalleeTarget::Reflection {
                        operation: CompileReflectionCall::Read,
                        function: WRITE_FUNCTION,
                        debug_name: "reflect::set".to_owned(),
                    },
                    values,
                ),
                expression_origin(body, expression),
            )
        },
    )
    .expect_err("reflection name mismatch");
    assert!(
        wrong_name
            .to_string()
            .contains("operation or debug name disagrees"),
        "{wrong_name:?}"
    );

    let invalid_placement = try_build_reflection(
        "fn main(target, field) { return reflect::get(target, field); }",
        |_graph, body, targets| {
            let (expression, call) = only_call(body);
            let values = argument_values(&call);
            insert_reflection_descriptor(
                targets,
                body,
                expression,
                READ_FUNCTION,
                "reflect::get",
                vec![required("target"), required("field")],
                CompilePositionalPolicy::RuntimeChecked,
                MirEffect::PURE,
            )?;
            targets.insert_call(
                ROOT_FUNCTION,
                expression,
                CompileCallTarget::dynamic(
                    CompileCalleeTarget::Reflection {
                        operation: CompileReflectionCall::Read,
                        function: READ_FUNCTION,
                        debug_name: "reflect::get".to_owned(),
                    },
                    values
                        .into_iter()
                        .map(|value| CompileDynamicCallArgument { name: None, value })
                        .collect(),
                ),
                expression_origin(body, expression),
            )
        },
    )
    .expect_err("reflection dynamic placement");
    assert!(
        invalid_placement
            .to_string()
            .contains("non-script call target has incompatible argument placement"),
        "{invalid_placement:?}"
    );
}

#[test]
fn other_reflection_natives_remain_ordinary_native_calls() {
    let source = "fn main(value) { return reflect::name(value); }";
    let program = try_build_reflection(source, |_graph, body, targets| {
        let (expression, call) = only_call(body);
        let values = argument_values(&call);
        targets.insert_function_descriptor(
            function_descriptor(
                OTHER_FUNCTION,
                CompileFunctionClass::Native,
                "reflect::name",
                vec![required("target")],
                CompilePositionalPolicy::ExactOrTrailingDefaults,
                MirEffect::reflection_read(),
            ),
            expression_origin(body, expression),
        )?;
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::positional(
                CompileCalleeTarget::NativeFunction {
                    function: OTHER_FUNCTION,
                    debug_name: "reflect::name".to_owned(),
                },
                values,
            ),
            expression_origin(body, expression),
        )
    })
    .expect("ordinary reflection native");
    let (_, function) = program.functions().next().expect("main function");
    assert!(function.statements().any(|(_, statement)| matches!(
        statement.kind,
        MirStatementKind::Call(MirCall::NativeFunction {
            function: OTHER_FUNCTION,
            ..
        })
    )));
    assert!(
        !function
            .statements()
            .any(|(_, statement)| matches!(statement.kind, MirStatementKind::Reflect(_)))
    );
}

fn operand_source<'a>(
    source: &'a str,
    function: &crate::MirFunction,
    operand: &MirOperand,
) -> &'a str {
    let origin = match operand {
        MirOperand::Local(local) => function.local(*local).expect("operand local").origin,
        MirOperand::Temp(temp) => function.temp(*temp).expect("operand temp").origin,
        MirOperand::Immediate(_) => panic!("expected sourced operand"),
    };
    source_text(source, origin)
}

fn source_text(source: &str, origin: MirSourceOrigin) -> &str {
    &source[origin.span.start as usize..origin.span.end as usize]
}
