use std::collections::BTreeMap;

use super::*;

use vela_common::HostTypeId;
use vela_def::{FieldId, TypeId, VariantId};
use vela_hir::body::{HirBody, HirBodyOwner, HirExprKind};
use vela_hir::ids::HirExprId;

use crate::{
    CompileCallTarget, CompileCalleeTarget, CompileFieldAccess, CompileFieldDescriptor,
    CompileHostPathSegment, CompileHostPathTarget, CompileLambdaParameterTarget,
    CompileLambdaTarget, CompileMemberTarget, CompileReflectionCall, CompileTargetSnapshotBuilder,
    CompileTryFamily, CompileTryLayoutTarget, CompileTryTarget, CompileTypeClass,
    CompileTypeDescriptor, CompileVariantDescriptor, HostFieldTarget, MirBuildError,
    MirTypeContract,
};

const SWEEP_SOURCE: SourceId = SourceId::new(112);
const SWEEP_FUNCTION: FunctionId = FunctionId::new(7_600);
const SWEEP_SYMBOL: &str = "verifier_sweep::main";

fn build_configured(
    source: &str,
    configure: impl FnOnce(
        &ModuleGraph,
        &HirBody,
        &mut CompileTargetSnapshotBuilder,
    ) -> Result<(), MirBuildError>,
) -> crate::MirProgram {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SWEEP_SOURCE,
        ModulePath::from_qualified("verifier_sweep"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main body");
    let bindings = graph.bindings_for_body(body.id).expect("root bindings");
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
            default: parameter.default_body.map_or(
                CompileParameterDefault::Required,
                CompileParameterDefault::HirBody,
            ),
            origin: Some(MirSourceOrigin::body(body.id, parameter.origin.span)),
        })
        .collect();
    let root_origin = MirSourceOrigin::body(body.id, body.origin.span);
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(SWEEP_FUNCTION, body.id)],
    )
    .expect("sweep analysis");
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_script_function(
            declaration.id,
            body.id,
            CompileFunctionDescriptor {
                id: SWEEP_FUNCTION,
                class: CompileFunctionClass::Script,
                canonical_symbol: SWEEP_SYMBOL.to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
                    parameters,
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::script(false),
            },
            root_origin,
        )
        .expect("root target");
    configure(&graph, body, &mut targets).expect("sweep target configuration");
    let targets = targets.build().expect("closed sweep targets");
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(SWEEP_FUNCTION),
        body.id,
        analysis.view(SWEEP_FUNCTION).expect("analysis view"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )
    .expect("sweep lowering input");
    crate::build_mir(input).expect("representative builder output")
}

fn expression_origin(body: &HirBody, expression: HirExprId) -> MirSourceOrigin {
    MirSourceOrigin::expression(
        body.id,
        expression,
        body.expression(expression).expect("expression").origin.span,
    )
}

fn add_lambda_targets(
    graph: &ModuleGraph,
    root: &HirBody,
    targets: &mut CompileTargetSnapshotBuilder,
) -> Result<(), MirBuildError> {
    let mut bodies = graph
        .bodies()
        .filter(|body| matches!(body.owner, HirBodyOwner::Lambda { .. }))
        .filter(|body| {
            graph
                .body_and_ancestors(body.id)
                .any(|ancestor| ancestor.id == root.id)
        })
        .collect::<Vec<_>>();
    bodies.sort_unstable_by_key(|body| {
        let depth = graph
            .body_and_ancestors(body.id)
            .filter(|ancestor| matches!(ancestor.owner, HirBodyOwner::Lambda { .. }))
            .count();
        (depth, body.origin.span.start, body.id)
    });
    let mut symbols = BTreeMap::from([(root.id, SWEEP_SYMBOL.to_owned())]);
    for body in bodies {
        let HirBodyOwner::Lambda {
            parent: hir_parent,
            expression,
        } = body.owner
        else {
            unreachable!()
        };
        let parent = graph
            .body_and_ancestors(hir_parent)
            .find(|candidate| {
                candidate.id == root.id || matches!(candidate.owner, HirBodyOwner::Lambda { .. })
            })
            .expect("lambda executable parent")
            .id;
        let code_symbol = format!(
            "{}::<lambda@{}>",
            symbols.get(&parent).expect("parent symbol"),
            body.origin.span.start
        );
        let bindings = graph.bindings_for_body(body.id).expect("lambda bindings");
        let parameters = body
            .params
            .iter()
            .map(|parameter| CompileLambdaParameterTarget {
                parameter: parameter.id,
                local: parameter.local,
                name: bindings
                    .local(parameter.local)
                    .expect("lambda parameter binding")
                    .name
                    .clone(),
                contract: None,
                origin: MirSourceOrigin::body(body.id, parameter.origin.span),
            })
            .collect();
        targets.insert_lambda(
            SWEEP_FUNCTION,
            CompileLambdaTarget {
                body: body.id,
                parent,
                expression,
                code_symbol: code_symbol.clone(),
                parameters,
                origin: MirSourceOrigin::body(body.id, body.origin.span),
            },
        )?;
        symbols.insert(body.id, code_symbol);
    }
    Ok(())
}

#[test]
fn mir_verifier_sweeps_closures_defaults_aggregates_and_patterns() {
    let closures = build_configured(
        "fn main(pair, fallback = 7) { let (left, right) = pair; let closure = |value| (value, right, fallback); return closure; }",
        add_lambda_targets,
    );
    assert!(closures.defined_len() > 1);
    verify_mir(&closures).expect("closure/default builder output verifies");

    let aggregates = build(
        "fn main() { let pair = (1, 2); let (left, right) = pair; let items = [left, right]; return right; }",
        &[],
    );
    verify_mir(&aggregates).expect("aggregate/pattern builder output verifies");
}

#[test]
fn mir_verifier_sweeps_registry_calls_and_reflection() {
    const REGISTRY: FunctionId = FunctionId::new(7_610);
    const REFLECT_GET: FunctionId = FunctionId::new(7_611);
    let program = build_configured(
        "fn main(value) { let loaded = registry(value); return reflect::get(loaded, \"name\"); }",
        |_graph, body, targets| {
            let required = |name: &str| CompileParameter {
                name: name.to_owned(),
                contract: None,
                default: CompileParameterDefault::Required,
                origin: None,
            };
            let registry_signature = CompileSignature {
                parameters: vec![required("value")],
                positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                return_contract: None,
                effect: MirEffect::PURE,
            };
            let reflection_signature = CompileSignature {
                parameters: vec![required("target"), required("member")],
                positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                return_contract: None,
                effect: MirEffect::PURE,
            };
            let origin = MirSourceOrigin::body(body.id, body.origin.span);
            for (id, symbol, signature) in [
                (REGISTRY, "registry", registry_signature.clone()),
                (REFLECT_GET, "reflect::get", reflection_signature.clone()),
            ] {
                targets.insert_function_descriptor(
                    CompileFunctionDescriptor {
                        id,
                        class: CompileFunctionClass::Registry,
                        canonical_symbol: symbol.to_owned(),
                        debug_name: symbol.to_owned(),
                        signature,
                        access: CompileFunctionAccess::new(true, true, true),
                    },
                    origin,
                )?;
            }
            for (expression, record) in &body.expressions {
                let HirExprKind::Call(call) = &record.kind else {
                    continue;
                };
                let HirExprKind::Path(path) =
                    &body.expression(call.callee).expect("call callee").kind
                else {
                    continue;
                };
                let name = body.paths.get(path).expect("callee path").path.join("::");
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| argument.value.expect("argument value"))
                    .collect::<Vec<_>>();
                let callee = match name.as_str() {
                    "registry" => CompileCalleeTarget::NativeFunction {
                        function: REGISTRY,
                        debug_name: "registry".to_owned(),
                    },
                    "reflect::get" => CompileCalleeTarget::Reflection {
                        operation: CompileReflectionCall::Read,
                        function: REFLECT_GET,
                        debug_name: "reflect::get".to_owned(),
                    },
                    _ => continue,
                };
                targets.insert_call(
                    SWEEP_FUNCTION,
                    *expression,
                    CompileCallTarget::positional(callee, arguments),
                    expression_origin(body, *expression),
                )?;
            }
            Ok(())
        },
    );
    verify_mir(&program).expect("registry call/reflection builder output verifies");
}

const HOST_TYPE_ID: TypeId = TypeId::new(7_620);
const HOST_RUNTIME_ID: HostTypeId = HostTypeId::new(7_621);
const HOST_FIELD_ID: FieldId = FieldId::new(7_622);
const HOST_TYPE: HostTypeTarget = HostTypeTarget {
    semantic: HOST_TYPE_ID,
    runtime: HOST_RUNTIME_ID,
};

fn host_field() -> HostFieldTarget {
    HostFieldTarget {
        owner: HOST_TYPE,
        semantic: HOST_FIELD_ID,
        runtime: HOST_FIELD_ID,
        access: CompileFieldAccess::new(true, false, true, false, Vec::new()),
    }
}

fn host_path(body: &HirBody, expression: HirExprId) -> Option<CompileHostPathTarget> {
    match &body.expression(expression)?.kind {
        HirExprKind::Path(path) => {
            let path = body.paths.get(path)?;
            matches!(path.path.as_slice(), [name] if name == "host").then_some(
                CompileHostPathTarget {
                    root: expression,
                    root_type: HOST_TYPE,
                    segments: Vec::new(),
                },
            )
        }
        HirExprKind::Field(field) if field.name == "value" => {
            let mut path = host_path(body, field.receiver)?;
            path.segments
                .push(CompileHostPathSegment::Field(host_field()));
            Some(path)
        }
        _ => None,
    }
}

#[test]
fn mir_verifier_sweeps_hostaccess_builder_output() {
    let program = build_configured(
        "fn main(host) { return host.value; }",
        |_graph, body, targets| {
            let origin = MirSourceOrigin::body(body.id, body.origin.span);
            targets.insert_type_descriptor(
                CompileTypeDescriptor {
                    id: HOST_TYPE_ID,
                    canonical_name: "host::Root".to_owned(),
                    class: CompileTypeClass::Host {
                        runtime: HOST_RUNTIME_ID,
                    },
                    shape: None,
                    fields: vec![HOST_FIELD_ID],
                    variants: Vec::new(),
                },
                origin,
            )?;
            targets.insert_field_descriptor(
                CompileFieldDescriptor {
                    id: HOST_FIELD_ID,
                    owner: HOST_TYPE_ID,
                    variant: None,
                    name: "value".to_owned(),
                    contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                    declaration_order: 0,
                    access: host_field().access,
                    host_runtime: Some(HOST_FIELD_ID),
                },
                origin,
            )?;
            for (expression, record) in &body.expressions {
                let Some(path) = host_path(body, *expression) else {
                    continue;
                };
                let expression_origin = expression_origin(body, *expression);
                targets.insert_host_path(SWEEP_FUNCTION, *expression, path, expression_origin)?;
                if matches!(record.kind, HirExprKind::Field(_)) {
                    targets.insert_member(
                        SWEEP_FUNCTION,
                        *expression,
                        CompileMemberTarget::HostField(host_field()),
                        expression_origin,
                    )?;
                }
            }
            Ok(())
        },
    );
    verify_mir(&program).expect("HostAccess builder output verifies");
}

const OPTION_TYPE: TypeId = TypeId::new(7_630);
const OPTION_SOME: VariantId = VariantId::new(7_631);
const OPTION_NONE: VariantId = VariantId::new(7_632);
const OPTION_VALUE: FieldId = FieldId::new(7_633);
const OPTION_LAYOUT: CompileTryLayoutTarget = CompileTryLayoutTarget {
    family: CompileTryFamily::Option,
    type_id: OPTION_TYPE,
    continue_variant: OPTION_SOME,
    break_variant: OPTION_NONE,
    continue_payload: OPTION_VALUE,
};

#[test]
fn mir_verifier_sweeps_try_builder_output() {
    let program = build_configured(
        "fn main(value) { let inner = value?; return inner; }",
        |_graph, body, targets| {
            let origin = MirSourceOrigin::body(body.id, body.origin.span);
            targets.insert_type_descriptor(
                CompileTypeDescriptor {
                    id: OPTION_TYPE,
                    canonical_name: "std::Option".to_owned(),
                    class: CompileTypeClass::Standard,
                    shape: None,
                    fields: Vec::new(),
                    variants: vec![OPTION_SOME, OPTION_NONE],
                },
                origin,
            )?;
            targets.insert_variant_descriptor(
                CompileVariantDescriptor {
                    id: OPTION_SOME,
                    owner: OPTION_TYPE,
                    name: "Some".to_owned(),
                    fields: vec![OPTION_VALUE],
                    declaration_order: 0,
                },
                origin,
            )?;
            targets.insert_variant_descriptor(
                CompileVariantDescriptor {
                    id: OPTION_NONE,
                    owner: OPTION_TYPE,
                    name: "None".to_owned(),
                    fields: Vec::new(),
                    declaration_order: 1,
                },
                origin,
            )?;
            targets.insert_field_descriptor(
                CompileFieldDescriptor {
                    id: OPTION_VALUE,
                    owner: OPTION_TYPE,
                    variant: Some(OPTION_SOME),
                    name: "0".to_owned(),
                    contract: None,
                    declaration_order: 0,
                    access: CompileFieldAccess::script(),
                    host_runtime: None,
                },
                origin,
            )?;
            for (expression, record) in &body.expressions {
                if matches!(record.kind, HirExprKind::Try { .. }) {
                    targets.insert_try_target(
                        SWEEP_FUNCTION,
                        *expression,
                        CompileTryTarget::Expected(OPTION_LAYOUT),
                        expression_origin(body, *expression),
                    )?;
                }
            }
            Ok(())
        },
    );
    verify_mir(&program).expect("try builder output verifies");
}
