use std::collections::BTreeMap;

use vela_analysis::executable::{
    ExecutableAnalysisGeneration, ExecutableAnalysisInput, ExecutableReceiverInput,
};
use vela_analysis::semantic_facts::ScriptTypeTargetFact;
use vela_analysis::type_fact::TypeFact;
use vela_common::{PrimitiveTag, ShapeId, SourceId, Span};
use vela_def::{FunctionId, TypeId};
use vela_hir::body::{HirBody, HirBodyOwner, HirBodyRoot, HirExprKind};
use vela_hir::ids::{HirBodyId, HirExprId, HirLocalId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};
use vela_hir::script_methods::{ScriptMethodCatalog, ScriptMethodCatalogMode};

use super::super::topological_lambdas;
use crate::{
    CompileConstructorTarget, CompileDynamicConstructorField, CompileFunctionAccess,
    CompileFunctionClass, CompileFunctionDescriptor, CompileFunctionIdentity, CompileGuardKey,
    CompileGuardTarget, CompileLambdaParameterTarget, CompileLambdaTarget, CompileMethodAccess,
    CompileMethodClass, CompileMethodDescriptor, CompileParameter, CompileParameterDefault,
    CompilePositionalPolicy, CompileSignature, CompileTargetSnapshot, CompileTypeClass,
    CompileTypeDescriptor, MethodExecutableTarget, MirAggregate, MirEffect, MirFunctionOwner,
    MirGuardAssumption, MirGuardLocation, MirLocalKind, MirLoweringConfig, MirLoweringInput,
    MirOperand, MirParameterKind, MirRvalue, MirSourceOrigin, MirStatementKind, MirTerminatorKind,
    MirTypeContract,
};

const FUNCTION: FunctionId = FunctionId::new(8_800);

#[derive(Clone, Debug)]
struct CaptureExpectation {
    body: HirBodyId,
    captures: Vec<(vela_hir::ids::HirCaptureId, HirLocalId, MirSourceOrigin)>,
}

struct BuiltFixture {
    program: crate::MirProgram,
    root: HirBodyId,
    defaults: Vec<HirBodyId>,
    lambdas: Vec<HirBodyId>,
    captures: Vec<CaptureExpectation>,
}

fn build_fixture(
    source: &str,
    contracts: &[(&str, MirTypeContract)],
    return_contract: Option<MirTypeContract>,
    guarded_default: Option<usize>,
    dynamic_constructor_default: Option<usize>,
) -> BuiltFixture {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(88),
        ModulePath::from_qualified("closures_defaults"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let root = graph.function_body(declaration.id).expect("main body");
    let bindings = graph.bindings_for_body(root.id).expect("main bindings");
    let contracts = contracts.iter().cloned().collect::<BTreeMap<_, _>>();
    let parameters = root
        .params
        .iter()
        .map(|parameter| {
            let name = bindings
                .local(parameter.local)
                .expect("parameter binding")
                .name
                .clone();
            CompileParameter {
                contract: contracts.get(name.as_str()).cloned(),
                name,
                default: parameter.default_body.map_or(
                    CompileParameterDefault::Required,
                    CompileParameterDefault::HirBody,
                ),
                origin: Some(MirSourceOrigin::body(root.id, parameter.origin.span)),
            }
        })
        .collect::<Vec<_>>();
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(FUNCTION, root.id)],
    )
    .expect("executable analysis");

    let root_origin = MirSourceOrigin::body(root.id, root.origin.span);
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_script_function(
            declaration.id,
            root.id,
            CompileFunctionDescriptor {
                id: FUNCTION,
                class: CompileFunctionClass::Script,
                canonical_symbol: "closures_defaults::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
                    parameters: parameters.clone(),
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: return_contract.clone(),
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::script(false),
            },
            root_origin,
        )
        .expect("root target");
    for (index, parameter) in parameters.iter().enumerate() {
        let Some(contract) = parameter.contract.clone() else {
            continue;
        };
        let index = u32::try_from(index).expect("fixture parameter index");
        targets
            .insert_guard(
                CompileGuardKey::Parameter {
                    function: FUNCTION,
                    parameter: index,
                },
                CompileGuardTarget::new(
                    contract,
                    MirGuardLocation::Parameter { index },
                    parameter.name.clone(),
                ),
                parameter.origin.expect("parameter origin"),
            )
            .expect("parameter guard");
    }
    if let Some(contract) = return_contract {
        targets
            .insert_guard(
                CompileGuardKey::Return(FUNCTION),
                CompileGuardTarget::new(contract, MirGuardLocation::Return, "return"),
                root_origin,
            )
            .expect("return guard");
    }
    if let Some(index) = guarded_default {
        let parameter = &root.params[index];
        let body = graph
            .body(parameter.default_body.expect("guarded default body"))
            .expect("guarded default body");
        let HirBodyRoot::Expr(expression) = body.root else {
            panic!("guarded default must have an expression root")
        };
        let contract = parameters[index]
            .contract
            .clone()
            .expect("guarded default contract");
        let origin = expression_origin(body, expression);
        targets
            .insert_guard(
                CompileGuardKey::Expression {
                    function: FUNCTION,
                    expression,
                },
                CompileGuardTarget::new(
                    contract,
                    MirGuardLocation::Parameter {
                        index: u32::try_from(index).expect("guarded parameter index"),
                    },
                    parameters[index].name.clone(),
                ),
                origin,
            )
            .expect("default expression guard");
    }
    if let Some(index) = dynamic_constructor_default {
        let body = graph
            .body(
                root.params[index]
                    .default_body
                    .expect("constructor default"),
            )
            .expect("constructor default body");
        let HirBodyRoot::Expr(expression) = body.root else {
            panic!("constructor default must have an expression root")
        };
        let HirExprKind::Record { fields, .. } = &body
            .expression(expression)
            .expect("constructor expression")
            .kind
        else {
            panic!("constructor default must be a record expression")
        };
        let fields = fields
            .iter()
            .map(|field| CompileDynamicConstructorField {
                name: field.name.clone(),
                value: field.value.expect("constructor field value"),
            })
            .collect();
        targets
            .insert_constructor(
                FUNCTION,
                expression,
                CompileConstructorTarget::DynamicRecord {
                    type_name: "Missing".to_owned(),
                    fields,
                },
                expression_origin(body, expression),
            )
            .expect("constructor target");
    }

    let mut lambda_bodies = graph
        .bodies()
        .filter(|body| matches!(body.owner, HirBodyOwner::Lambda { .. }))
        .filter(|body| {
            graph
                .body_and_ancestors(body.id)
                .any(|ancestor| ancestor.id == root.id)
        })
        .map(|body| body.id)
        .collect::<Vec<_>>();
    lambda_bodies.sort_unstable_by_key(|body| {
        let body = graph.body(*body).expect("lambda body");
        let depth = graph
            .body_and_ancestors(body.id)
            .filter(|ancestor| matches!(ancestor.owner, HirBodyOwner::Lambda { .. }))
            .count();
        (
            depth,
            body.origin.span.source,
            body.origin.span.start,
            body.origin.span.end,
            body.id,
        )
    });
    let mut symbols = BTreeMap::from([(root.id, "closures_defaults::main".to_owned())]);
    for body_id in &lambda_bodies {
        let body = graph.body(*body_id).expect("lambda body");
        let HirBodyOwner::Lambda { expression, .. } = body.owner else {
            unreachable!()
        };
        let parent = graph
            .body_and_ancestors(direct_parent(body))
            .find(|candidate| {
                candidate.id == root.id || matches!(candidate.owner, HirBodyOwner::Lambda { .. })
            })
            .expect("executable lambda parent")
            .id;
        let symbol = format!(
            "{}::<lambda@{}>",
            symbols.get(&parent).expect("parent symbol"),
            body.origin.span.start
        );
        let lambda_bindings = graph.bindings_for_body(body.id).expect("lambda bindings");
        let parameters = body
            .params
            .iter()
            .map(|parameter| {
                let binding = lambda_bindings
                    .local(parameter.local)
                    .expect("lambda parameter binding");
                CompileLambdaParameterTarget {
                    parameter: parameter.id,
                    local: parameter.local,
                    name: binding.name.clone(),
                    contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                    origin: MirSourceOrigin::body(body.id, parameter.origin.span),
                }
            })
            .collect();
        targets
            .insert_lambda(
                FUNCTION,
                CompileLambdaTarget {
                    body: body.id,
                    parent,
                    expression,
                    code_symbol: symbol.clone(),
                    parameters,
                    origin: MirSourceOrigin::body(body.id, body.origin.span),
                },
            )
            .expect("lambda target");
        symbols.insert(body.id, symbol);
    }

    let captures = lambda_bodies
        .iter()
        .map(|body| {
            let body = graph.body(*body).expect("capture body");
            CaptureExpectation {
                body: body.id,
                captures: body
                    .captures
                    .iter()
                    .map(|capture| {
                        let (use_body, expression) = graph
                            .bodies()
                            .find_map(|body| {
                                body.expression(capture.use_expression)
                                    .map(|expression| (body, expression))
                            })
                            .expect("capture use expression");
                        (
                            capture.id,
                            capture.local,
                            MirSourceOrigin::expression(
                                use_body.id,
                                capture.use_expression,
                                expression.origin.span,
                            ),
                        )
                    })
                    .collect(),
            }
        })
        .collect();
    let defaults = root
        .params
        .iter()
        .filter_map(|parameter| parameter.default_body)
        .collect();
    let targets = targets.build().expect("closed targets");
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(FUNCTION),
        root.id,
        analysis.view(FUNCTION).expect("analysis view"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )
    .expect("MIR input");
    let program = crate::build_mir(input).expect("complete MIR lowering");
    BuiltFixture {
        program,
        root: root.id,
        defaults,
        lambdas: lambda_bodies,
        captures,
    }
}

fn direct_parent(body: &HirBody) -> HirBodyId {
    match body.owner {
        HirBodyOwner::Lambda { parent, .. } | HirBodyOwner::ParameterDefault { parent, .. } => {
            parent
        }
        _ => panic!("fixture body has no parent"),
    }
}

fn expression_origin(body: &HirBody, expression: HirExprId) -> MirSourceOrigin {
    MirSourceOrigin::expression(
        body.id,
        expression,
        body.expression(expression).expect("expression").origin.span,
    )
}

fn function_for_body(
    fixture: &BuiltFixture,
    body: HirBodyId,
) -> (crate::MirFunctionId, &crate::MirFunction) {
    let [function] = fixture.program.functions_for_body(body) else {
        panic!("body {body:?} must have exactly one MIR function")
    };
    (
        *function,
        fixture
            .program
            .function(*function)
            .expect("defined function"),
    )
}

#[test]
fn complete_closure_tree_preserves_topology_parameters_captures_and_origins() {
    let fixture = build_fixture(
        r#"
fn main(base, callback = (|seed: i64| base + seed)) {
    let outer = |amount: i64| {
        return |bonus: i64| base + amount + bonus;
    };
    return outer;
}
"#,
        &[(
            "callback",
            MirTypeContract::Callable {
                kind: crate::MirCallableKind::Closure,
                positional_arity: Some(1),
            },
        )],
        Some(MirTypeContract::Callable {
            kind: crate::MirCallableKind::Closure,
            positional_arity: Some(1),
        }),
        None,
        None,
    );

    assert_eq!(fixture.lambdas.len(), 3);
    assert_eq!(fixture.program.defined_len(), 4);
    assert!(!fixture.program.has_undefined_reservations());
    let (root_id, root) = function_for_body(&fixture, fixture.root);
    assert_eq!(root_id.index(), 0);
    assert!(root.return_contract().is_some());
    assert_eq!(root.parameters()[1].default_body, Some(fixture.defaults[0]));

    for expected in &fixture.captures {
        let (function_id, function) = function_for_body(&fixture, expected.body);
        let actual = function
            .captures()
            .iter()
            .map(|capture| (capture.capture, capture.source_local, capture.origin))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected.captures);
        assert_eq!(
            function
                .debug_locals()
                .filter(|(_, local)| local.kind == crate::DebugLocalKind::Capture)
                .map(|(_, local)| local.origin)
                .collect::<Vec<_>>(),
            expected
                .captures
                .iter()
                .map(|(_, _, origin)| *origin)
                .collect::<Vec<_>>()
        );
        if let MirFunctionOwner::Lambda { parent, .. } = function.owner() {
            assert!(parent.index() < function_id.index());
        } else {
            panic!("nested body must have a lambda owner")
        }
        for parameter in function.parameters() {
            assert!(matches!(parameter.kind, MirParameterKind::Explicit(_)));
            assert_eq!(
                parameter.contract,
                Some(MirTypeContract::Primitive(PrimitiveTag::I64))
            );
            assert_eq!(parameter.origin.body, Some(function.body()));
        }
    }

    for (_, function) in fixture.program.functions() {
        for (_, statement) in function.statements() {
            let MirStatementKind::Allocate(MirAggregate::Closure {
                function: nested,
                captures,
            }) = &statement.kind
            else {
                continue;
            };
            let nested = fixture.program.function(*nested).expect("nested function");
            assert_eq!(captures.len(), nested.captures().len());
            assert!(
                captures
                    .iter()
                    .all(|capture| matches!(capture, MirOperand::Local(_)))
            );
        }
    }
    assert_eq!(
        fixture
            .program
            .functions()
            .flat_map(|(_, function)| function.statements())
            .filter(|(_, statement)| matches!(
                statement.kind,
                MirStatementKind::Allocate(MirAggregate::Closure { .. })
            ))
            .count(),
        3
    );
}

#[test]
fn defaults_form_ordered_is_missing_cfg_with_one_authoritative_guard() {
    let fixture = build_fixture(
        "fn main(first, second: i64 = first, third: i64 = { let copy = second; copy }) { return third; }",
        &[
            ("second", MirTypeContract::Primitive(PrimitiveTag::I64)),
            ("third", MirTypeContract::Primitive(PrimitiveTag::I64)),
        ],
        Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
        Some(1),
        None,
    );
    let (_, function) = function_for_body(&fixture, fixture.root);
    assert_eq!(function.parameters().len(), 3);
    assert_eq!(fixture.defaults.len(), 2);
    assert_eq!(
        function
            .guards()
            .map(|(_, guard)| (&guard.assumption, guard.context.as_ref()))
            .collect::<Vec<_>>(),
        vec![(
            &MirGuardAssumption::Type(MirTypeContract::Primitive(PrimitiveTag::I64)),
            Some(&crate::MirGuardContext::new(
                MirGuardLocation::Parameter { index: 1 },
                "second"
            ))
        )]
    );
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(statement.kind, MirStatementKind::GuardTrap { .. }))
            .count(),
        1
    );
    let missing = function
        .statements()
        .filter_map(|(_, statement)| match &statement.kind {
            MirStatementKind::Assign(MirRvalue::IsMissing { value }) => Some(value.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        missing,
        vec![
            MirOperand::Local(function.parameters()[1].storage),
            MirOperand::Local(function.parameters()[2].storage),
        ]
    );
    assert_eq!(
        function
            .blocks()
            .filter_map(|(_, block)| block.terminator())
            .filter(|terminator| matches!(terminator.kind, MirTerminatorKind::Branch { .. }))
            .count(),
        fixture.defaults.len()
    );
    assert!(function.debug_locals().any(|(_, local)| {
        local.name == "copy"
            && local.kind == crate::DebugLocalKind::Local
            && local.origin.body == Some(fixture.defaults[1])
    }));
    assert!(
        function
            .locals()
            .any(|(_, local)| matches!(local.kind, MirLocalKind::Synthetic))
    );
}

#[test]
fn terminating_default_branch_does_not_assign_or_fall_through() {
    let fixture = build_fixture(
        "fn main(first, second = { return first; }) { return second; }",
        &[],
        None,
        None,
        None,
    );
    let (_, function) = function_for_body(&fixture, fixture.root);
    assert_eq!(
        function
            .blocks()
            .filter_map(|(_, block)| block.terminator())
            .filter(|terminator| matches!(terminator.kind, MirTerminatorKind::Return(_)))
            .count(),
        2
    );
    let second = function.parameters()[1].storage;
    assert!(
        !function.statements().any(|(_, statement)| {
            statement.destination == Some(crate::MirPlace::local(second))
        })
    );
}

#[test]
fn guarded_lambda_default_emits_exactly_one_authoritative_guard() {
    let contract = MirTypeContract::Callable {
        kind: crate::MirCallableKind::Closure,
        positional_arity: Some(1),
    };
    let fixture = build_fixture(
        "fn main(base, callback = (|seed: i64| base + seed)) { return callback; }",
        &[("callback", contract)],
        None,
        Some(1),
        None,
    );
    let (_, function) = function_for_body(&fixture, fixture.root);
    assert_eq!(function.guards().count(), 1);
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(statement.kind, MirStatementKind::GuardTrap { .. }))
            .count(),
        1
    );
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(
                statement.kind,
                MirStatementKind::Allocate(MirAggregate::Closure { .. })
            ))
            .count(),
        1
    );
}

#[test]
fn guarded_constructor_default_emits_exactly_one_authoritative_guard() {
    let fixture = build_fixture(
        "fn main(value = Missing { field: 1 }) { return value; }",
        &[("value", MirTypeContract::Primitive(PrimitiveTag::I64))],
        None,
        Some(0),
        Some(0),
    );
    let (_, function) = function_for_body(&fixture, fixture.root);
    assert_eq!(function.guards().count(), 1);
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(statement.kind, MirStatementKind::GuardTrap { .. }))
            .count(),
        1
    );
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(
                statement.kind,
                MirStatementKind::Allocate(MirAggregate::DynamicRecord { .. })
            ))
            .count(),
        1
    );
}

#[test]
fn method_default_prologue_resolves_receiver_and_earlier_parameter_in_order() {
    const METHOD_FUNCTION: FunctionId = FunctionId::new(8_890);
    const OWNER_TYPE: TypeId = TypeId::new(8_891);
    let source = r#"
struct Counter {}
impl Counter {
    fn combine(self, first, second = (self, first)) { return second; }
}
"#;
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(89),
        ModulePath::from_qualified("closures_defaults_method"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let catalog = ScriptMethodCatalog::from_graph(&graph, ScriptMethodCatalogMode::ModuleGraph)
        .expect("script method catalog");
    let method = catalog.methods().next().expect("combine method");
    assert_eq!(catalog.len(), 1);
    let body = graph.body(method.body()).expect("method body");
    let bindings = graph.bindings_for_body(body.id).expect("method bindings");
    let owner_declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "Counter")
        .expect("Counter declaration");
    let executable = MethodExecutableTarget {
        method: method.method_id(),
        function: METHOD_FUNCTION,
        owner: OWNER_TYPE,
        node: method.node(),
    };
    let parameters = body
        .params
        .iter()
        .map(|parameter| CompileParameter {
            name: bindings
                .local(parameter.local)
                .expect("method parameter binding")
                .name
                .clone(),
            contract: None,
            default: parameter.default_body.map_or(
                CompileParameterDefault::Required,
                CompileParameterDefault::HirBody,
            ),
            origin: Some(MirSourceOrigin::body(body.id, parameter.origin.span)),
        })
        .collect::<Vec<_>>();
    let symbol = method.symbol_seed();
    let body_origin = MirSourceOrigin::body(body.id, body.origin.span);
    let owner_name = method.owner().target_type().to_owned();
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [
            ExecutableAnalysisInput::new(METHOD_FUNCTION, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record(owner_name.clone()))
                    .with_script_type(ScriptTypeTargetFact::declaration(owner_declaration.id)),
            ),
        ],
    )
    .expect("method executable analysis");

    let full_signature = CompileSignature {
        parameters: parameters.clone(),
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: None,
        effect: MirEffect::PURE,
    };
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_script_type(
            owner_declaration.id,
            CompileTypeDescriptor {
                id: OWNER_TYPE,
                canonical_name: owner_name.clone(),
                class: CompileTypeClass::ScriptRecord,
                shape: Some(ShapeId::new(8_892)),
                fields: Vec::new(),
                variants: Vec::new(),
            },
            body_origin,
        )
        .expect("method owner type");
    targets
        .insert_function_descriptor(
            CompileFunctionDescriptor {
                id: METHOD_FUNCTION,
                class: CompileFunctionClass::Script,
                canonical_symbol: symbol.clone(),
                debug_name: format!("{owner_name}::combine"),
                signature: full_signature.clone(),
                access: CompileFunctionAccess::script(false),
            },
            body_origin,
        )
        .expect("method function descriptor");
    targets
        .insert_method_descriptor(
            CompileMethodDescriptor {
                id: executable.method,
                owner: OWNER_TYPE,
                member_name: "combine".to_owned(),
                debug_name: format!("{owner_name}::combine"),
                class: CompileMethodClass::Script {
                    executable,
                    owner_name,
                    code_symbol: symbol,
                },
                signature: CompileSignature {
                    parameters: full_signature.parameters.iter().skip(1).cloned().collect(),
                    ..full_signature
                },
                access: CompileMethodAccess::script(),
            },
            body_origin,
        )
        .expect("method descriptor");
    targets
        .insert_script_method(body.id, executable, body_origin)
        .expect("method root");
    let targets = targets.build().expect("closed method targets");
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Method(executable),
        body.id,
        analysis
            .view(METHOD_FUNCTION)
            .expect("method analysis view"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )
    .expect("method MIR input");
    let program = crate::build_mir(input).expect("method MIR");
    let function_id = program
        .function_by_id(METHOD_FUNCTION)
        .expect("method function ID");
    let function = program.function(function_id).expect("method function");

    assert_eq!(function.owner(), &MirFunctionOwner::Method(executable));
    assert_eq!(function.parameters().len(), 3);
    assert_eq!(function.parameters()[0].kind, MirParameterKind::Receiver);
    assert!(matches!(
        function.parameters()[1].kind,
        MirParameterKind::Explicit(_)
    ));
    assert!(matches!(
        function.parameters()[2].kind,
        MirParameterKind::Explicit(_)
    ));
    assert_eq!(
        function.parameters()[2].default_body,
        body.params[2].default_body
    );
    assert_eq!(
        program
            .targets()
            .method(OWNER_TYPE, executable.method)
            .expect("retained method descriptor")
            .signature
            .parameters
            .len(),
        2
    );

    let receiver = function.parameters()[0].storage;
    let first = function.parameters()[1].storage;
    let second = function.parameters()[2].storage;
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let missing = statements
        .iter()
        .position(|statement| {
            matches!(
                &statement.kind,
                MirStatementKind::Assign(MirRvalue::IsMissing {
                    value: MirOperand::Local(local),
                }) if *local == second
            )
        })
        .expect("second missing check");
    let (tuple, tuple_values) = statements
        .iter()
        .enumerate()
        .find_map(|(index, statement)| match &statement.kind {
            MirStatementKind::Allocate(MirAggregate::Tuple(values)) => Some((index, values)),
            _ => None,
        })
        .expect("tuple default allocation");
    let [captured_receiver, MirOperand::Local(actual_first)] = tuple_values.as_slice() else {
        panic!("tuple default must preserve receiver/earlier-parameter order: {tuple_values:?}")
    };
    assert_eq!(*actual_first, first);
    let MirOperand::Temp(receiver_temp) = captured_receiver else {
        panic!("receiver must be stabilized before the later tuple operand")
    };
    let receiver_capture = statements
        .iter()
        .position(|statement| {
            statement.destination == Some(crate::MirPlace::temp(*receiver_temp))
                && matches!(
                    statement.kind,
                    MirStatementKind::Assign(MirRvalue::Use(MirOperand::Local(local)))
                        if local == receiver
                )
        })
        .expect("receiver capture definition");
    let default_assignment = statements
        .iter()
        .position(|statement| statement.destination == Some(crate::MirPlace::local(second)))
        .expect("default parameter assignment");
    assert!(missing < receiver_capture);
    assert!(receiver_capture < tuple);
    assert!(tuple < default_assignment);
}

#[test]
fn lambda_ordering_is_topological_even_when_body_ids_are_not() {
    let root = HirBodyId::new(50);
    let origin = |body: HirBodyId, start| CompileLambdaTarget {
        body,
        parent: root,
        expression: HirExprId::new(start),
        code_symbol: format!("lambda_{start}"),
        parameters: Vec::new(),
        origin: MirSourceOrigin::body(
            body,
            Span::new(SourceId::new(89), start, start.saturating_add(1)),
        ),
    };
    let first = origin(HirBodyId::new(90), 10);
    let second = origin(HirBodyId::new(2), 20);
    let child = CompileLambdaTarget {
        parent: first.body,
        ..origin(HirBodyId::new(1), 5)
    };
    let ordered = topological_lambdas(root, vec![child.clone(), second.clone(), first.clone()])
        .expect("acyclic lambda tree");
    assert_eq!(
        ordered.iter().map(|target| target.body).collect::<Vec<_>>(),
        vec![first.body, second.body, child.body]
    );
}
