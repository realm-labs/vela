use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{PrimitiveTag, SourceId};
use vela_def::{FunctionId, TypeId};
use vela_hir::binding::BindingMap;
use vela_hir::body::{HirBody, HirBodyRoot, HirExprKind, HirStmtKind};
use vela_hir::ids::{HirExprId, HirLocalId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use crate::{
    CompileConstructorTarget, CompileDynamicConstructorField, CompileFunctionAccess,
    CompileFunctionClass, CompileFunctionDescriptor, CompileFunctionIdentity, CompileGuardKey,
    CompileGuardTarget, CompileLambdaParameterTarget, CompileLambdaTarget, CompileParameter,
    CompileParameterDefault, CompilePositionalPolicy, CompileSignature, CompileTargetSnapshot,
    CompileTypeClass, CompileTypeDescriptor, MirEffect, MirGuardLocation, MirLocalKind,
    MirLoweringConfig, MirLoweringInput, MirPlace, MirRvalue, MirSourceNode, MirSourceOrigin,
    MirStatementKind, MirTypeContract,
};

const FUNCTION: FunctionId = FunctionId::new(9_800);
const GUARDED_RECORD: TypeId = TypeId::new(9_801);

#[test]
fn central_guards_cover_bindings_assignment_nested_values_control_flow_and_lambda_once() {
    let source = r#"
fn main(value, target, condition) {
    let typed: i64 = value;
    target = value;
    let nested = Missing { field: ((value,),) };
    let controlled = if condition { value } else { value };
    let closure = |argument| value;
    return controlled;
}
"#;
    let graph = graph(source);
    let body = root_body(&graph);
    let bindings = graph.bindings_for_body(body.id).expect("bindings");
    let typed = let_initializer(body, bindings, "typed");
    let nested = let_initializer(body, bindings, "nested");
    let controlled = let_initializer(body, bindings, "controlled");
    let closure = let_initializer(body, bindings, "closure");
    let assignment_rhs = body
        .expressions
        .values()
        .find_map(|expression| match expression.kind {
            HirExprKind::Assign {
                value: Some(value), ..
            } => Some(value),
            _ => None,
        })
        .expect("assignment RHS");
    let HirExprKind::Record { fields, .. } = &body.expression(nested).expect("record").kind else {
        panic!("nested initializer must be a record")
    };
    let record_field = fields[0].value.expect("record field value");
    let [inner_tuple] = tuple_elements(body, record_field) else {
        panic!("record field must contain one outer tuple element")
    };
    let [nested_value] = tuple_elements(body, *inner_tuple) else {
        panic!("inner tuple must contain one value")
    };
    assert!(matches!(
        body.expression(controlled)
            .expect("control expression")
            .kind,
        HirExprKind::If(_)
    ));
    let HirExprKind::Lambda { body: lambda_body } =
        body.expression(closure).expect("closure expression").kind
    else {
        panic!("closure initializer must be a lambda")
    };

    let mut targets = root_targets(&graph, body, Vec::new());
    let root_origin = MirSourceOrigin::body(body.id, body.origin.span);
    targets
        .insert_type_descriptor(
            CompileTypeDescriptor {
                id: GUARDED_RECORD,
                canonical_name: "guards::Expected".to_owned(),
                runtime_name: "guards::Expected".to_owned(),
                class: CompileTypeClass::OpaqueExternal,
                shape: None,
                fields: Vec::new(),
                variants: Vec::new(),
            },
            root_origin,
        )
        .expect("guard type descriptor");
    targets
        .insert_constructor(
            FUNCTION,
            nested,
            CompileConstructorTarget::DynamicRecord {
                type_name: "Missing".to_owned(),
                fields: vec![CompileDynamicConstructorField {
                    name: fields[0].name.clone(),
                    value: record_field,
                }],
            },
            expression_origin(body, nested),
        )
        .expect("dynamic constructor target");
    let lambda = graph.body(lambda_body).expect("lambda body");
    let lambda_bindings = graph.bindings_for_body(lambda.id).expect("lambda bindings");
    targets
        .insert_lambda(
            FUNCTION,
            CompileLambdaTarget {
                body: lambda.id,
                parent: body.id,
                expression: closure,
                code_symbol: format!("guards::main::<lambda@{}>", lambda.origin.span.start),
                parameters: lambda
                    .params
                    .iter()
                    .map(|parameter| CompileLambdaParameterTarget {
                        parameter: parameter.id,
                        local: parameter.local,
                        name: lambda_bindings
                            .local(parameter.local)
                            .expect("lambda parameter")
                            .name
                            .clone(),
                        contract: None,
                        origin: MirSourceOrigin::body(lambda.id, parameter.origin.span),
                    })
                    .collect(),
                origin: MirSourceOrigin::body(lambda.id, lambda.origin.span),
            },
        )
        .expect("lambda target");

    let guarded = [
        (
            typed,
            MirTypeContract::Primitive(PrimitiveTag::I64),
            "typed",
        ),
        (
            assignment_rhs,
            MirTypeContract::Primitive(PrimitiveTag::I64),
            "assignment",
        ),
        (
            *nested_value,
            MirTypeContract::Primitive(PrimitiveTag::I64),
            "nested-value",
        ),
        (
            *inner_tuple,
            MirTypeContract::Tuple(vec![None]),
            "inner-tuple",
        ),
        (
            record_field,
            MirTypeContract::Tuple(vec![None]),
            "outer-tuple",
        ),
        (
            nested,
            MirTypeContract::Definition(GUARDED_RECORD),
            "constructor",
        ),
        (
            controlled,
            MirTypeContract::Primitive(PrimitiveTag::I64),
            "control",
        ),
        (
            closure,
            MirTypeContract::Callable {
                accepted_kinds: crate::MirCallableKindSet::CLOSURE,
                positional_arity: Some(1),
            },
            "closure",
        ),
    ];
    for (expression, contract, name) in guarded.clone() {
        targets
            .insert_guard(
                CompileGuardKey::Expression {
                    function: FUNCTION,
                    expression,
                },
                CompileGuardTarget::new(contract, MirGuardLocation::Local, name),
                expression_origin(body, expression),
            )
            .expect("expression guard");
    }
    let program = build(&graph, body, targets);
    let function = root_function(&program);
    assert_eq!(function.guards().count(), guarded.len());
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(statement.kind, MirStatementKind::GuardTrap { .. }))
            .count(),
        guarded.len()
    );

    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let guard_position = |expression| {
        statements
            .iter()
            .position(|statement| {
                matches!(statement.kind, MirStatementKind::GuardTrap { .. })
                    && statement.origin.node == MirSourceNode::Expression(expression)
            })
            .unwrap_or_else(|| panic!("guard for {expression:?}"))
    };
    let allocation_position = |expression| {
        statements
            .iter()
            .position(|statement| {
                matches!(statement.kind, MirStatementKind::Allocate(_))
                    && statement.origin.node == MirSourceNode::Expression(expression)
            })
            .unwrap_or_else(|| panic!("allocation for {expression:?}"))
    };
    let typed_local = local_named(bindings, "typed");
    let typed_assignment = statements
        .iter()
        .position(|statement| statement.destination == mir_local(function, typed_local))
        .expect("typed let assignment");
    assert!(guard_position(typed) < typed_assignment);
    let target_local = local_named(bindings, "target");
    let target_assignment = statements
        .iter()
        .position(|statement| statement.destination == mir_local(function, target_local))
        .expect("assignment target write");
    assert!(guard_position(assignment_rhs) < target_assignment);

    assert!(guard_position(*nested_value) < allocation_position(*inner_tuple));
    assert!(allocation_position(*inner_tuple) < guard_position(*inner_tuple));
    assert!(guard_position(*inner_tuple) < allocation_position(record_field));
    assert!(allocation_position(record_field) < guard_position(record_field));
    assert!(guard_position(record_field) < allocation_position(nested));
    assert!(allocation_position(nested) < guard_position(nested));

    let controlled_local = local_named(bindings, "controlled");
    let controlled_assignment = statements
        .iter()
        .position(|statement| statement.destination == mir_local(function, controlled_local))
        .expect("controlled let assignment");
    assert!(guard_position(controlled) < controlled_assignment);
    let closure_allocation = allocation_position(closure);
    assert!(closure_allocation < guard_position(closure));
    let closure_local = local_named(bindings, "closure");
    let closure_assignment = statements
        .iter()
        .position(|statement| statement.destination == mir_local(function, closure_local))
        .expect("closure let assignment");
    assert!(guard_position(closure) < closure_assignment);
}

#[test]
fn parameter_default_block_tail_guard_precedes_result_and_parameter_assignment() {
    let source = "fn main(value, fallback: i64 = { value }) { return fallback; }";
    let graph = graph(source);
    let body = root_body(&graph);
    let bindings = graph.bindings_for_body(body.id).expect("bindings");
    let default_body = graph
        .body(body.params[1].default_body.expect("fallback default"))
        .expect("default body");
    let HirBodyRoot::Expr(block_expression) = default_body.root else {
        panic!("default must have an expression root")
    };
    let HirExprKind::Block { block } = default_body
        .expression(block_expression)
        .expect("block expression")
        .kind
    else {
        panic!("default must be a block expression")
    };
    let block = default_body.blocks.get(&block).expect("default block");
    let tail = block
        .statements
        .last()
        .and_then(|statement| default_body.statements.get(statement))
        .and_then(|statement| match statement.kind {
            HirStmtKind::Expr {
                expression: Some(expression),
                terminated: false,
            } => Some(expression),
            _ => None,
        })
        .expect("default block tail");
    let parameters = body_parameters(&graph, body, Some(1));
    let mut targets = root_targets(&graph, body, parameters);
    let parameter_contract = MirTypeContract::Primitive(PrimitiveTag::I64);
    targets
        .insert_guard(
            CompileGuardKey::Parameter {
                function: FUNCTION,
                parameter: 1,
            },
            CompileGuardTarget::new(
                parameter_contract.clone(),
                MirGuardLocation::Parameter { index: 1 },
                "fallback",
            ),
            MirSourceOrigin::body(body.id, body.params[1].origin.span),
        )
        .expect("parameter guard metadata");
    targets
        .insert_guard(
            CompileGuardKey::Expression {
                function: FUNCTION,
                expression: tail,
            },
            CompileGuardTarget::new(parameter_contract, MirGuardLocation::Local, "default-tail"),
            expression_origin(default_body, tail),
        )
        .expect("default tail guard");
    let program = build(&graph, body, targets);
    let function = root_function(&program);
    assert_eq!(function.guards().count(), 1);
    let fallback = function.parameters()[1].storage;
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let guard = statements
        .iter()
        .position(|statement| matches!(statement.kind, MirStatementKind::GuardTrap { .. }))
        .expect("tail guard");
    let result_assignment = statements
        .iter()
        .enumerate()
        .find_map(|(index, statement)| {
            let Some(MirPlace::Local(local)) = statement.destination else {
                return None;
            };
            (function
                .local(local)
                .is_some_and(|local| local.kind == MirLocalKind::Synthetic)
                && matches!(statement.kind, MirStatementKind::Assign(MirRvalue::Use(_))))
            .then_some(index)
        })
        .expect("block result assignment");
    let parameter_assignment = statements
        .iter()
        .position(|statement| statement.destination == Some(MirPlace::local(fallback)))
        .expect("default parameter assignment");
    assert!(guard < result_assignment);
    assert!(result_assignment < parameter_assignment);
    assert_eq!(
        local_named(bindings, "fallback"),
        function.parameters()[1].hir_local
    );
}

fn graph(source: &str) -> ModuleGraph {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(98),
        ModulePath::from_qualified("guards"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    graph
}

fn root_body(graph: &ModuleGraph) -> &HirBody {
    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    graph.function_body(declaration.id).expect("main body")
}

fn root_targets(
    graph: &ModuleGraph,
    body: &HirBody,
    parameters: Vec<CompileParameter>,
) -> crate::CompileTargetSnapshotBuilder {
    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let parameters = if parameters.is_empty() {
        body_parameters(graph, body, None)
    } else {
        parameters
    };
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_script_function(
            declaration.id,
            body.id,
            CompileFunctionDescriptor {
                id: FUNCTION,
                class: CompileFunctionClass::Script,
                canonical_symbol: "guards::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
                    parameters,
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::script(false),
            },
            MirSourceOrigin::body(body.id, body.origin.span),
        )
        .expect("root target");
    targets
}

fn body_parameters(
    graph: &ModuleGraph,
    body: &HirBody,
    contracted: Option<usize>,
) -> Vec<CompileParameter> {
    let bindings = graph.bindings_for_body(body.id).expect("bindings");
    body.params
        .iter()
        .enumerate()
        .map(|(index, parameter)| CompileParameter {
            name: bindings
                .local(parameter.local)
                .expect("parameter binding")
                .name
                .clone(),
            contract: (contracted == Some(index))
                .then_some(MirTypeContract::Primitive(PrimitiveTag::I64)),
            default: parameter.default_body.map_or(
                CompileParameterDefault::Required,
                CompileParameterDefault::HirBody,
            ),
            origin: Some(MirSourceOrigin::body(body.id, parameter.origin.span)),
        })
        .collect()
}

fn build(
    graph: &ModuleGraph,
    body: &HirBody,
    targets: crate::CompileTargetSnapshotBuilder,
) -> crate::MirProgram {
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        graph,
        [ExecutableAnalysisInput::new(FUNCTION, body.id)],
    )
    .expect("guard analysis");
    let targets = targets.build().expect("closed guard targets");
    let input = MirLoweringInput::new(
        graph,
        CompileFunctionIdentity::Function(FUNCTION),
        body.id,
        analysis.view(FUNCTION).expect("guard analysis view"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )
    .expect("guard MIR input");
    crate::build_mir(input).expect("guard MIR")
}

fn root_function(program: &crate::MirProgram) -> &crate::MirFunction {
    let function = program.function_by_id(FUNCTION).expect("root function");
    program.function(function).expect("defined root function")
}

fn let_initializer(body: &HirBody, bindings: &BindingMap, name: &str) -> HirExprId {
    body.statements
        .values()
        .find_map(|statement| match &statement.kind {
            HirStmtKind::Let {
                pattern: Some(pattern),
                initializer: Some(initializer),
                ..
            } if body
                .patterns
                .get(pattern)
                .and_then(|pattern| pattern.local())
                .and_then(|local| bindings.local(local))
                .is_some_and(|binding| binding.name == name) =>
            {
                Some(*initializer)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("let initializer {name:?}"))
}

fn tuple_elements(body: &HirBody, expression: HirExprId) -> &[HirExprId] {
    let HirExprKind::Tuple { elements } = &body.expression(expression).expect("tuple").kind else {
        panic!("expression {expression:?} must be a tuple")
    };
    elements
}

fn expression_origin(body: &HirBody, expression: HirExprId) -> MirSourceOrigin {
    MirSourceOrigin::expression(
        body.id,
        expression,
        body.expression(expression).expect("expression").origin.span,
    )
}

fn local_named(bindings: &BindingMap, name: &str) -> HirLocalId {
    let [local] = bindings.locals_named(name) else {
        panic!("one local named {name:?}")
    };
    *local
}

fn mir_local(function: &crate::MirFunction, hir: HirLocalId) -> Option<MirPlace> {
    function.locals().find_map(|(local, data)| {
        (data.kind == MirLocalKind::Script(hir)).then_some(MirPlace::local(local))
    })
}
